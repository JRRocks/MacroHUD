//! The physical-click guard: while a macro uses a mouse button (« Maintenir le clic » holds it,
//! « Multi-clic » clicks it again and again), a real click on that button stops the macro (« un clic
//! physique reprend la main »).
//!
//! It is fed with the button events seen by the low-level mouse hook (the events MacroHUD injects
//! itself never reach it) and decides which ones to swallow:
//!
//! - the click that stops the macro is consumed, press and release, so the application sees one
//!   press and one release, both from the macro, and nothing else;
//! - while a button is held, a click on the HUD (any button, by hand or by another program) is
//!   consumed and stops every hold (D-17): Windows sends all mouse input to the window that received
//!   the held press (it "captures" the mouse), so that click would never reach the HUD, and would
//!   land in the game instead;
//! - while « Multi-clic » runs, a click on the HUD reaches the HUD and acts there at once (D-27): the
//!   automatic clicks wait while the mouse is on the HUD, so nothing holds the mouse then; a real
//!   click of the clicked button anywhere else stops the macro (D-19);
//! - when « Maintenir le clic » starts while the user already holds the button, the user's release is
//!   consumed too, so the macro takes the button over instead of losing it at once;
//! - a release is never swallowed when its press reached the system: no button can get stuck.

use std::time::{Duration, Instant};

use crate::contract::MouseButton;

/// A set of mouse buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ButtonSet(u8);

impl ButtonSet {
    pub const EMPTY: ButtonSet = ButtonSet(0);

    fn bit(button: MouseButton) -> u8 {
        match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            MouseButton::Middle => 4,
        }
    }

    pub fn only(button: MouseButton) -> Self {
        ButtonSet(Self::bit(button))
    }

    pub fn with(self, button: MouseButton) -> Self {
        ButtonSet(self.0 | Self::bit(button))
    }

    /// The buttons of both sets.
    pub fn union(self, other: ButtonSet) -> Self {
        ButtonSet(self.0 | other.0)
    }

    #[cfg(test)]
    pub fn without(self, button: MouseButton) -> Self {
        ButtonSet(self.0 & !Self::bit(button))
    }

    pub fn contains(self, button: MouseButton) -> bool {
        self.0 & Self::bit(button) != 0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = MouseButton> {
        MouseButton::ALL.into_iter().filter(move |b| self.contains(*b))
    }

    /// Raw value, to keep the set in an atomic.
    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn from_bits(bits: u8) -> Self {
        ButtonSet(bits & 7)
    }
}

/// Press or release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Down,
    Up,
}

/// Who made a mouse event (MacroHUD's own events are recognised earlier and never reach the guard).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The user's hand on the mouse.
    Hand,
    /// Another program (a test, a remote control, an accessibility tool...).
    OtherProgram,
}

/// Mouse messages of the low-level hook (values from WinUser.h).
pub mod msg {
    pub const WM_LBUTTONDOWN: u32 = 0x0201;
    pub const WM_LBUTTONUP: u32 = 0x0202;
    pub const WM_RBUTTONDOWN: u32 = 0x0204;
    pub const WM_RBUTTONUP: u32 = 0x0205;
    pub const WM_MBUTTONDOWN: u32 = 0x0207;
    pub const WM_MBUTTONUP: u32 = 0x0208;
    /// `MSLLHOOKSTRUCT.flags`: the event was injected (SendInput, by MacroHUD or another program).
    pub const LLMHF_INJECTED: u32 = 0x01;
    /// `MSLLHOOKSTRUCT.flags`: injected from a process with a lower integrity level.
    pub const LLMHF_LOWER_IL_INJECTED: u32 = 0x02;
    /// `wParam` flags of a mouse message: buttons and keys down.
    pub const MK_LBUTTON: usize = 0x0001;
    pub const MK_RBUTTON: usize = 0x0002;
    pub const MK_SHIFT: usize = 0x0004;
    pub const MK_CONTROL: usize = 0x0008;
    pub const MK_MBUTTON: usize = 0x0010;
}

/// The button and edge of a low-level mouse hook message; `None` for moves, wheel, side buttons.
pub fn button_event(message: u32) -> Option<(MouseButton, Edge)> {
    match message {
        msg::WM_LBUTTONDOWN => Some((MouseButton::Left, Edge::Down)),
        msg::WM_LBUTTONUP => Some((MouseButton::Left, Edge::Up)),
        msg::WM_RBUTTONDOWN => Some((MouseButton::Right, Edge::Down)),
        msg::WM_RBUTTONUP => Some((MouseButton::Right, Edge::Up)),
        msg::WM_MBUTTONDOWN => Some((MouseButton::Middle, Edge::Down)),
        msg::WM_MBUTTONUP => Some((MouseButton::Middle, Edge::Up)),
        _ => None,
    }
}

/// Whether a low-level mouse hook event was injected rather than made by the user's hand.
pub fn is_injected(flags: u32) -> bool {
    flags & (msg::LLMHF_INJECTED | msg::LLMHF_LOWER_IL_INJECTED) != 0
}

// --- Delivery of a release ---------------------------------------------------------------------

/// Where things stand when a held button is released, for [`release_reaches`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseScene {
    /// Thread of the window that received the press (0: unknown).
    pub press_thread: u32,
    /// Thread of the window under the pointer now.
    pub pointer_thread: u32,
    /// The press thread holds the mouse capture (`SetCapture`).
    pub press_thread_captures: bool,
    /// The press thread owns the foreground window.
    pub press_thread_foreground: bool,
}

/// Whether the release Windows delivers by itself reaches the application that received the press.
///
/// Windows sends a button message to the window under the pointer, unless a window captured the
/// mouse (`SetCapture`), and a capture only follows the pointer everywhere while its window is in
/// the foreground. An application that did not capture the mouse (Paint does, many do not) thus
/// never sees the release when the pointer is elsewhere at that moment, on the HUD for instance:
/// the release must then be sent to it (P1.8).
pub fn release_reaches(scene: &ReleaseScene) -> bool {
    scene.press_thread == 0
        || scene.pointer_thread == scene.press_thread
        || (scene.press_thread_captures && scene.press_thread_foreground)
}

/// The window message of a button's release.
pub fn release_message(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => msg::WM_LBUTTONUP,
        MouseButton::Right => msg::WM_RBUTTONUP,
        MouseButton::Middle => msg::WM_MBUTTONUP,
    }
}

/// `wParam` of a mouse message: the buttons still down, Shift and Ctrl.
pub fn key_flags(down: ButtonSet, shift: bool, ctrl: bool) -> usize {
    let mut flags = 0;
    for (on, flag) in [
        (down.contains(MouseButton::Left), msg::MK_LBUTTON),
        (down.contains(MouseButton::Right), msg::MK_RBUTTON),
        (down.contains(MouseButton::Middle), msg::MK_MBUTTON),
        (shift, msg::MK_SHIFT),
        (ctrl, msg::MK_CONTROL),
    ] {
        if on {
            flags |= flag;
        }
    }
    flags
}

/// `lParam` of a mouse message: a point in client coordinates, each on 16 signed bits (MAKELPARAM).
pub fn pack_point(x: i32, y: i32) -> isize {
    (((y as u16 as u32) << 16) | (x as u16 as u32)) as isize
}

/// The point of a release sent to the press window: the pointer's (client coordinates), brought
/// back inside the client area of `width` × `height`. Frameworks that look for the element under
/// the point (WPF...) then find one; a drawing application ends its stroke near where the pointer
/// left it.
pub fn point_inside(x: i32, y: i32, width: i32, height: i32) -> (i32, i32) {
    (x.clamp(0, (width - 1).max(0)), y.clamp(0, (height - 1).max(0)))
}

/// What to do with one button event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// Keep the event from the system and the applications.
    pub swallow: bool,
    /// The macros holding these buttons must stop.
    pub stop: ButtonSet,
    /// The stopping click was on the HUD.
    pub on_hud: bool,
}

impl Verdict {
    const PASS: Verdict = Verdict { swallow: false, stop: ButtonSet::EMPTY, on_hud: false };
    const SWALLOW: Verdict = Verdict { swallow: true, stop: ButtonSet::EMPTY, on_hud: false };
}

/// How long a swallowed press waits for its release before the guard forgets it.
pub const PENDING_RELEASE_TIMEOUT: Duration = Duration::from_secs(10);

/// The guard itself.
#[derive(Clone, Debug, Default)]
pub struct MouseGuard {
    /// Buttons held by « Maintenir le clic ».
    held: ButtonSet,
    /// Buttons clicked again and again by « Multi-clic ».
    clicked: ButtonSet,
    /// Per button (left, right, middle): a swallowed press whose release is still to swallow, since when.
    pending_release: [Option<Instant>; 3],
}

fn index(button: MouseButton) -> usize {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
    }
}

impl MouseGuard {
    /// The buttons macros hold (`held`) and click (`clicked`) from now on.
    pub fn watch(&mut self, held: ButtonSet, clicked: ButtonSet) {
        self.held = held;
        self.clicked = clicked;
    }

    /// The buttons macros use, held or clicked.
    pub fn used(&self) -> ButtonSet {
        self.held.union(self.clicked)
    }

    /// Whether the hook is needed: a macro uses a button, or a swallowed click is not released yet.
    pub fn needs_hook(&self) -> bool {
        !self.used().is_empty() || self.pending_release.iter().any(Option::is_some)
    }

    /// Handles a press or a release. `on_hud`: the pointer is over the HUD's window.
    pub fn on_event(&mut self, button: MouseButton, edge: Edge, source: Source, on_hud: bool, now: Instant) -> Verdict {
        let i = index(button);
        let hand = source == Source::Hand;
        match edge {
            Edge::Down if on_hud && !self.held.is_empty() => {
                // A click on the HUD during a hold: every hold stops, and this click is consumed.
                self.pending_release[i] = Some(now);
                Verdict { swallow: true, stop: self.held, on_hud: true }
            }
            Edge::Down if hand && !on_hud && self.used().contains(button) => {
                // The user clicks the button a macro uses: the macro stops, and this click is consumed.
                self.pending_release[i] = Some(now);
                Verdict { swallow: true, stop: ButtonSet::only(button), on_hud: false }
            }
            Edge::Down => {
                // (During « Multi-clic », a click on the HUD comes here: it acts on the HUD, D-27.)
                if hand {
                    // This press reaches the system, so its release must reach it too.
                    self.pending_release[i] = None;
                }
                Verdict::PASS
            }
            Edge::Up if self.pending_release[i].take().is_some() => Verdict::SWALLOW,
            // The user held the button before « Maintenir le clic » started: the macro keeps it held.
            // (Never for « Multi-clic »: its own clicks release the button anyway, and a click on the
            // HUD needs its release.)
            Edge::Up if hand && self.held.contains(button) => Verdict::SWALLOW,
            Edge::Up => Verdict::PASS,
        }
    }

    /// Forgets swallowed presses whose release never came (the hook missed it), so the hook can go.
    pub fn expire(&mut self, now: Instant) {
        for (i, pending) in self.pending_release.iter_mut().enumerate() {
            let button = MouseButton::ALL[i];
            if let Some(since) = *pending
                && !self.held.contains(button)
                && now.saturating_duration_since(since) >= PENDING_RELEASE_TIMEOUT
            {
                *pending = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MouseButton::*;

    fn t0() -> Instant {
        Instant::now()
    }

    fn hand(g: &mut MouseGuard, button: MouseButton, edge: Edge, now: Instant) -> Verdict {
        g.on_event(button, edge, Source::Hand, false, now)
    }

    #[test]
    fn a_click_on_the_hud_during_a_hold_stops_every_hold_and_is_consumed() {
        let now = t0();
        for source in [Source::Hand, Source::OtherProgram] {
            let mut g = MouseGuard::default();
            g.watch(ButtonSet::only(Right).with(Middle), ButtonSet::EMPTY);
            // A left click on the HUD while the right and middle buttons are held.
            let v = g.on_event(Left, Edge::Down, source, true, now);
            assert_eq!(v, Verdict { swallow: true, stop: ButtonSet::only(Right).with(Middle), on_hud: true });
            g.watch(ButtonSet::EMPTY, ButtonSet::EMPTY);
            assert_eq!(g.on_event(Left, Edge::Up, source, true, now), Verdict::SWALLOW, "{source:?}");
            assert!(!g.needs_hook());
            // Afterwards, clicks on the HUD reach it normally.
            assert_eq!(g.on_event(Left, Edge::Down, source, true, now), Verdict::PASS);
            assert_eq!(g.on_event(Left, Edge::Up, source, true, now), Verdict::PASS);
        }
    }

    #[test]
    fn during_the_clicks_a_click_on_the_hud_reaches_it() {
        let now = t0();
        for source in [Source::Hand, Source::OtherProgram] {
            let mut g = MouseGuard::default();
            g.watch(ButtonSet::EMPTY, ButtonSet::only(Left));
            assert!(g.needs_hook());
            // The clicked button itself, or another one: the HUD gets the whole click (D-27).
            for button in [Left, Right] {
                assert_eq!(g.on_event(button, Edge::Down, source, true, now), Verdict::PASS, "{source:?}");
                assert_eq!(g.on_event(button, Edge::Up, source, true, now), Verdict::PASS, "{source:?}");
            }
        }
    }

    #[test]
    fn during_the_clicks_a_real_click_of_their_button_elsewhere_stops_them() {
        let now = t0();
        let mut g = MouseGuard::default();
        g.watch(ButtonSet::EMPTY, ButtonSet::only(Left));
        // Another button, or another program: nothing to do.
        assert_eq!(hand(&mut g, Right, Edge::Down, now), Verdict::PASS);
        assert_eq!(hand(&mut g, Right, Edge::Up, now), Verdict::PASS);
        assert_eq!(g.on_event(Left, Edge::Down, Source::OtherProgram, false, now), Verdict::PASS);
        assert_eq!(g.on_event(Left, Edge::Up, Source::OtherProgram, false, now), Verdict::PASS);
        // The user's hand on the clicked button: the macro stops, the click is consumed.
        let v = hand(&mut g, Left, Edge::Down, now);
        assert_eq!(v, Verdict { swallow: true, stop: ButtonSet::only(Left), on_hud: false });
        g.watch(ButtonSet::EMPTY, ButtonSet::EMPTY);
        assert_eq!(hand(&mut g, Left, Edge::Up, now), Verdict::SWALLOW);
        assert!(!g.needs_hook());
    }

    #[test]
    fn during_the_clicks_the_users_release_always_goes_through() {
        let now = t0();
        let mut g = MouseGuard::default();
        // The user held the button when the clicks started: unlike a hold, nothing keeps it.
        g.watch(ButtonSet::EMPTY, ButtonSet::only(Left));
        assert_eq!(hand(&mut g, Left, Edge::Up, now), Verdict::PASS);
        // Whereas a hold takes it over.
        g.watch(ButtonSet::only(Left), ButtonSet::EMPTY);
        assert_eq!(hand(&mut g, Left, Edge::Up, now), Verdict::SWALLOW);
    }

    #[test]
    fn clicks_elsewhere_by_another_program_are_left_alone() {
        let now = t0();
        let mut g = MouseGuard::default();
        g.watch(ButtonSet::only(Left), ButtonSet::EMPTY);
        // Another program clicks the held button outside the HUD: not the user's hand, no stop.
        assert_eq!(g.on_event(Left, Edge::Down, Source::OtherProgram, false, now), Verdict::PASS);
        assert_eq!(g.on_event(Left, Edge::Up, Source::OtherProgram, false, now), Verdict::PASS);
        assert_eq!(g.on_event(Right, Edge::Down, Source::OtherProgram, false, now), Verdict::PASS);
        // Nor does a click on the HUD when nothing is held.
        let mut idle = MouseGuard::default();
        assert_eq!(idle.on_event(Left, Edge::Down, Source::Hand, true, now), Verdict::PASS);
    }

    #[test]
    fn without_a_held_button_everything_goes_through() {
        let mut g = MouseGuard::default();
        let now = t0();
        assert!(!g.needs_hook());
        for (b, e) in [(Left, Edge::Down), (Left, Edge::Up), (Right, Edge::Down), (Right, Edge::Up)] {
            assert_eq!(hand(&mut g, b, e, now), Verdict::PASS);
        }
    }

    #[test]
    fn a_click_on_the_held_button_stops_the_macro_and_is_consumed() {
        let mut g = MouseGuard::default();
        let now = t0();
        g.watch(ButtonSet::only(Left), ButtonSet::EMPTY);
        assert!(g.needs_hook());
        let v = hand(&mut g, Left, Edge::Down, now);
        assert_eq!(v, Verdict { swallow: true, stop: ButtonSet::only(Left), on_hud: false });
        // The engine stops the macro: the button is no longer held.
        g.watch(ButtonSet::EMPTY, ButtonSet::EMPTY);
        assert!(g.needs_hook(), "the release is still to swallow");
        assert_eq!(hand(&mut g, Left, Edge::Up, now), Verdict::SWALLOW);
        assert!(!g.needs_hook());
        // The next click is an ordinary one.
        assert_eq!(hand(&mut g, Left, Edge::Down, now), Verdict::PASS);
        assert_eq!(hand(&mut g, Left, Edge::Up, now), Verdict::PASS);
    }

    #[test]
    fn the_release_is_swallowed_even_if_it_comes_before_the_engine_reacts() {
        let mut g = MouseGuard::default();
        let now = t0();
        g.watch(ButtonSet::only(Right), ButtonSet::EMPTY);
        assert!(hand(&mut g, Right, Edge::Down, now).swallow);
        assert_eq!(hand(&mut g, Right, Edge::Up, now), Verdict::SWALLOW);
        g.watch(ButtonSet::EMPTY, ButtonSet::EMPTY);
        assert!(!g.needs_hook());
    }

    #[test]
    fn other_buttons_are_left_alone() {
        let mut g = MouseGuard::default();
        let now = t0();
        g.watch(ButtonSet::only(Left), ButtonSet::EMPTY);
        assert_eq!(hand(&mut g, Right, Edge::Down, now), Verdict::PASS);
        assert_eq!(hand(&mut g, Right, Edge::Up, now), Verdict::PASS);
        assert_eq!(hand(&mut g, Middle, Edge::Down, now), Verdict::PASS);
        assert_eq!(hand(&mut g, Middle, Edge::Up, now), Verdict::PASS);
    }

    #[test]
    fn a_button_held_by_hand_when_the_macro_starts_is_taken_over() {
        // Physical press before the macro started: it went through.
        let mut g = MouseGuard::default();
        let now = t0();
        assert_eq!(hand(&mut g, Left, Edge::Down, now), Verdict::PASS);
        g.watch(ButtonSet::only(Left), ButtonSet::EMPTY);
        // The user lets go: the macro keeps the button down.
        assert_eq!(hand(&mut g, Left, Edge::Up, now), Verdict::SWALLOW);
        // A later click stops it as usual.
        assert_eq!(hand(&mut g, Left, Edge::Down, now).stop, ButtonSet::only(Left));
    }

    #[test]
    fn a_release_whose_press_went_through_is_never_swallowed() {
        // A swallowed press whose release was missed, then an ordinary click: its release must go
        // through, or the button would stay down.
        let mut g = MouseGuard::default();
        let now = t0();
        g.watch(ButtonSet::only(Left), ButtonSet::EMPTY);
        assert!(hand(&mut g, Left, Edge::Down, now).swallow);
        g.watch(ButtonSet::EMPTY, ButtonSet::EMPTY);
        assert_eq!(hand(&mut g, Left, Edge::Down, now), Verdict::PASS);
        assert_eq!(hand(&mut g, Left, Edge::Up, now), Verdict::PASS);
        assert!(!g.needs_hook());
    }

    #[test]
    fn missed_releases_expire() {
        let mut g = MouseGuard::default();
        let start = t0();
        g.watch(ButtonSet::only(Middle), ButtonSet::EMPTY);
        assert!(hand(&mut g, Middle, Edge::Down, start).swallow);
        g.watch(ButtonSet::EMPTY, ButtonSet::EMPTY);
        g.expire(start + Duration::from_secs(1));
        assert!(g.needs_hook());
        g.expire(start + PENDING_RELEASE_TIMEOUT);
        assert!(!g.needs_hook());
    }

    #[test]
    fn hook_messages_and_flags_are_decoded() {
        assert_eq!(button_event(msg::WM_LBUTTONDOWN), Some((Left, Edge::Down)));
        assert_eq!(button_event(msg::WM_RBUTTONUP), Some((Right, Edge::Up)));
        assert_eq!(button_event(msg::WM_MBUTTONDOWN), Some((Middle, Edge::Down)));
        assert_eq!(button_event(0x0200), None, "WM_MOUSEMOVE");
        assert_eq!(button_event(0x020A), None, "WM_MOUSEWHEEL");
        assert!(is_injected(msg::LLMHF_INJECTED));
        assert!(is_injected(msg::LLMHF_INJECTED | msg::LLMHF_LOWER_IL_INJECTED));
        assert!(!is_injected(0));
    }

    #[test]
    fn a_release_reaches_the_press_window_only_under_the_pointer_or_by_capture() {
        let app = 10;
        let hud = 20;
        let scene = |pointer_thread, captures, foreground| ReleaseScene {
            press_thread: app,
            pointer_thread,
            press_thread_captures: captures,
            press_thread_foreground: foreground,
        };
        // F8 with the pointer still on the window that got the press.
        assert!(release_reaches(&scene(app, false, false)));
        // Pointer on the HUD: only a capture in the foreground brings the release (Paint)...
        assert!(release_reaches(&scene(hud, true, true)));
        // ...not a window without capture (the PC test's window), nor a capture in the background.
        assert!(!release_reaches(&scene(hud, false, true)));
        assert!(!release_reaches(&scene(hud, true, false)));
        assert!(!release_reaches(&scene(hud, false, false)));
        // Press window unknown: nothing to send.
        assert!(release_reaches(&ReleaseScene { press_thread: 0, ..scene(hud, false, false) }));
    }

    #[test]
    fn release_messages_are_built_like_windows_ones() {
        assert_eq!(release_message(Left), msg::WM_LBUTTONUP);
        assert_eq!(release_message(Right), msg::WM_RBUTTONUP);
        assert_eq!(release_message(Middle), msg::WM_MBUTTONUP);
        assert_eq!(key_flags(ButtonSet::EMPTY, false, false), 0);
        assert_eq!(key_flags(ButtonSet::only(Left), true, false), msg::MK_LBUTTON | msg::MK_SHIFT);
        assert_eq!(key_flags(ButtonSet::only(Middle).with(Right), false, true), 0x0010 | 0x0002 | 0x0008);
        // MAKELPARAM: x in the low word, y in the high word, negative values on 16 bits.
        assert_eq!(pack_point(10, 20), 0x0014_000A);
        assert_eq!(pack_point(-1, 5), 0x0005_FFFF);
        assert_eq!(pack_point(3, -2), 0xFFFE_0003_u32 as isize);
        // The point stays where it is inside the window, and comes back to its edge from outside.
        assert_eq!(point_inside(40, 30, 800, 600), (40, 30));
        assert_eq!(point_inside(-250, 30, 800, 600), (0, 30));
        assert_eq!(point_inside(900, 700, 800, 600), (799, 599));
        assert_eq!(point_inside(5, 5, 0, 0), (0, 0));
    }

    #[test]
    fn button_sets() {
        let s = ButtonSet::EMPTY.with(Left).with(Middle);
        assert!(s.contains(Left) && s.contains(Middle) && !s.contains(Right));
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![Left, Middle]);
        assert_eq!(s.without(Left), ButtonSet::only(Middle));
        assert_eq!(ButtonSet::from_bits(s.bits()), s);
        assert!(ButtonSet::EMPTY.is_empty());
    }
}
