//! The physical-click guard: while a macro holds a mouse button, a real click on that button stops
//! the macro (« un clic physique reprend la main »).
//!
//! It is fed with the *physical* button events seen by the low-level mouse hook (the events MacroHUD
//! injects itself are flagged as injected and never reach it) and decides which ones to swallow:
//!
//! - the click that stops the macro is consumed, press and release, so the application sees one
//!   press and one release, both from the macro, and nothing else;
//! - when the macro starts while the user already holds the button, the user's release is consumed
//!   too, so the macro takes the button over instead of losing it at once;
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

/// What to do with one physical button event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// Keep the event from the system and the applications.
    pub swallow: bool,
    /// The user clicked a held button: the macro holding it must stop.
    pub stop: Option<MouseButton>,
}

impl Verdict {
    const PASS: Verdict = Verdict { swallow: false, stop: None };
    const SWALLOW: Verdict = Verdict { swallow: true, stop: None };
}

/// How long a swallowed press waits for its release before the guard forgets it.
pub const PENDING_RELEASE_TIMEOUT: Duration = Duration::from_secs(10);

/// The guard itself.
#[derive(Clone, Debug, Default)]
pub struct MouseGuard {
    /// Buttons held by macros.
    held: ButtonSet,
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
    /// The buttons macros hold from now on.
    pub fn watch(&mut self, held: ButtonSet) {
        self.held = held;
    }

    /// Whether the hook is needed: a button is held, or a swallowed click is not released yet.
    pub fn needs_hook(&self) -> bool {
        !self.held.is_empty() || self.pending_release.iter().any(Option::is_some)
    }

    /// Handles a physical (non-injected) press or release.
    pub fn on_physical(&mut self, button: MouseButton, edge: Edge, now: Instant) -> Verdict {
        let i = index(button);
        match edge {
            Edge::Down if self.held.contains(button) => {
                // The user clicks the held button: the macro stops, and this click is consumed.
                self.pending_release[i] = Some(now);
                Verdict { swallow: true, stop: Some(button) }
            }
            Edge::Down => {
                // This press reaches the system, so its release must reach it too.
                self.pending_release[i] = None;
                Verdict::PASS
            }
            Edge::Up if self.pending_release[i].take().is_some() => Verdict::SWALLOW,
            // The user held the button before the macro started: the macro keeps it held.
            Edge::Up if self.held.contains(button) => Verdict::SWALLOW,
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

    #[test]
    fn without_a_held_button_everything_goes_through() {
        let mut g = MouseGuard::default();
        let now = t0();
        assert!(!g.needs_hook());
        for (b, e) in [(Left, Edge::Down), (Left, Edge::Up), (Right, Edge::Down), (Right, Edge::Up)] {
            assert_eq!(g.on_physical(b, e, now), Verdict::PASS);
        }
    }

    #[test]
    fn a_click_on_the_held_button_stops_the_macro_and_is_consumed() {
        let mut g = MouseGuard::default();
        let now = t0();
        g.watch(ButtonSet::only(Left));
        assert!(g.needs_hook());
        let v = g.on_physical(Left, Edge::Down, now);
        assert_eq!(v, Verdict { swallow: true, stop: Some(Left) });
        // The engine stops the macro: the button is no longer held.
        g.watch(ButtonSet::EMPTY);
        assert!(g.needs_hook(), "the release is still to swallow");
        assert_eq!(g.on_physical(Left, Edge::Up, now), Verdict::SWALLOW);
        assert!(!g.needs_hook());
        // The next click is an ordinary one.
        assert_eq!(g.on_physical(Left, Edge::Down, now), Verdict::PASS);
        assert_eq!(g.on_physical(Left, Edge::Up, now), Verdict::PASS);
    }

    #[test]
    fn the_release_is_swallowed_even_if_it_comes_before_the_engine_reacts() {
        let mut g = MouseGuard::default();
        let now = t0();
        g.watch(ButtonSet::only(Right));
        assert!(g.on_physical(Right, Edge::Down, now).swallow);
        assert_eq!(g.on_physical(Right, Edge::Up, now), Verdict::SWALLOW);
        g.watch(ButtonSet::EMPTY);
        assert!(!g.needs_hook());
    }

    #[test]
    fn other_buttons_are_left_alone() {
        let mut g = MouseGuard::default();
        let now = t0();
        g.watch(ButtonSet::only(Left));
        assert_eq!(g.on_physical(Right, Edge::Down, now), Verdict::PASS);
        assert_eq!(g.on_physical(Right, Edge::Up, now), Verdict::PASS);
        assert_eq!(g.on_physical(Middle, Edge::Down, now), Verdict::PASS);
        assert_eq!(g.on_physical(Middle, Edge::Up, now), Verdict::PASS);
    }

    #[test]
    fn a_button_held_by_hand_when_the_macro_starts_is_taken_over() {
        // Physical press before the macro started: it went through.
        let mut g = MouseGuard::default();
        let now = t0();
        assert_eq!(g.on_physical(Left, Edge::Down, now), Verdict::PASS);
        g.watch(ButtonSet::only(Left));
        // The user lets go: the macro keeps the button down.
        assert_eq!(g.on_physical(Left, Edge::Up, now), Verdict::SWALLOW);
        // A later click stops it as usual.
        assert_eq!(g.on_physical(Left, Edge::Down, now).stop, Some(Left));
    }

    #[test]
    fn a_release_whose_press_went_through_is_never_swallowed() {
        // A swallowed press whose release was missed, then an ordinary click: its release must go
        // through, or the button would stay down.
        let mut g = MouseGuard::default();
        let now = t0();
        g.watch(ButtonSet::only(Left));
        assert!(g.on_physical(Left, Edge::Down, now).swallow);
        g.watch(ButtonSet::EMPTY);
        assert_eq!(g.on_physical(Left, Edge::Down, now), Verdict::PASS);
        assert_eq!(g.on_physical(Left, Edge::Up, now), Verdict::PASS);
        assert!(!g.needs_hook());
    }

    #[test]
    fn missed_releases_expire() {
        let mut g = MouseGuard::default();
        let start = t0();
        g.watch(ButtonSet::only(Middle));
        assert!(g.on_physical(Middle, Edge::Down, start).swallow);
        g.watch(ButtonSet::EMPTY);
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
    fn button_sets() {
        let s = ButtonSet::EMPTY.with(Left).with(Middle);
        assert!(s.contains(Left) && s.contains(Middle) && !s.contains(Right));
        assert_eq!(s.iter().collect::<Vec<_>>(), vec![Left, Middle]);
        assert_eq!(s.without(Left), ButtonSet::only(Middle));
        assert_eq!(ButtonSet::from_bits(s.bits()), s);
        assert!(ButtonSet::EMPTY.is_empty());
    }
}
