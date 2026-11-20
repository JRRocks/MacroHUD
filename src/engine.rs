//! The macro engine: what each macro does when its hotkey is pressed, when the HUD asks, when the
//! user clicks for real, when MacroHUD quits.
//!
//! It knows nothing of Windows: everything it does to the system goes through [`Backend`], so
//! `cargo test` checks it on Linux with a fake backend. In the application it runs on its own
//! thread (see `platform`), which owns the global hotkeys and injects the mouse buttons, and wakes
//! it at [`Engine::next_deadline`].
//!
//! Two kinds of macros: « Maintenir le clic » keeps a button down; « Multi-clic » clicks it again and
//! again, on a fixed beat (press at `t0`, `t0 + 1/rate`, `t0 + 2/rate`...: a late wake-up never
//! shifts the next clicks, and after a long one the beat starts again instead of sending the missed
//! clicks in a burst).
//!
//! Rules it keeps, whatever happens:
//! - no button is pressed unless the physical-click guard already watches it;
//! - the button released is always the one actually pressed, even if the options changed since;
//! - stopping, changing the options, quitting: whatever is held is released first;
//! - one macro at a time (D-20): starting one stops the other first;
//! - no automatic click lands on the HUD (D-22): « Multi-clic » waits while the mouse is on it.

use std::time::{Duration, Instant};

use crate::capture::Outcome;
use crate::contract::{ClickRate, MacroId, MacroOptions, MacroStatus, Message, MessageLevel, MouseButton};
use crate::guard::ButtonSet;
use crate::hotkey::{self, Hotkey, Vk};
use crate::macros::MacroDef;

/// Delay between « Activer » in the HUD and the macro's first action: the time to move the mouse
/// away from the HUD, to where the button must act (at once, it would land on the HUD itself).
pub const HUD_START_DELAY: Duration = Duration::from_secs(3);
/// A capture that gets no key for this long is cancelled, so the keyboard is not listened to forever.
pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
/// The longest a click of « Multi-clic » keeps its button down (D-23).
pub const LONGEST_PRESS: Duration = Duration::from_millis(40);
/// While « Multi-clic » waits for the mouse to leave the HUD, how often it looks.
pub const HUD_CHECK: Duration = Duration::from_millis(50);

/// Identifier of a registered hotkey (the id given to `RegisterHotKey`).
pub type HotkeyId = i32;

/// Why a hotkey could not be registered.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// Already taken by Windows or by another program.
    Taken,
    /// Any other refusal, described.
    Other(String),
}

/// What the engine needs from the system.
pub trait Backend {
    /// From now on the physical-click guard watches these buttons (the ones macros hold or click).
    fn watch(&mut self, guarded: ButtonSet) -> Result<(), String>;
    /// Whether the button is already down (held by the user's hand, for instance).
    fn is_down(&self, button: MouseButton) -> bool;
    /// Presses the button (injected input), remembered for the emergency release.
    fn press(&mut self, button: MouseButton) -> Result<(), String>;
    /// Takes over a button that is already down: nothing injected, but it will be released like a
    /// pressed one (and by the emergency release).
    fn adopt(&mut self, button: MouseButton);
    /// Releases the button (injected input); its release reaches the window that got the press.
    fn release(&mut self, button: MouseButton) -> Result<(), String>;
    /// The press of one click of « Multi-clic »: as [`Backend::press`], without a line in the run
    /// log for each click.
    fn click_down(&mut self, button: MouseButton) -> Result<(), String>;
    /// Its release: as [`Backend::release`], without a line in the run log. `Ok(true)`: Windows would
    /// not have brought it to the window of the press, which got it sent as well (P1.8).
    fn click_up(&mut self, button: MouseButton) -> Result<bool, String>;
    /// Whether the mouse pointer is over the HUD window.
    fn pointer_on_hud(&self) -> bool;
    /// Registers a global hotkey.
    fn register_hotkey(&mut self, hotkey: Hotkey) -> Result<HotkeyId, RegisterError>;
    fn unregister_hotkey(&mut self, id: HotkeyId);
    /// Starts listening to the keyboard for a new hotkey; the result comes as [`Event::Captured`].
    fn start_capture(&mut self) -> Result<(), String>;
    /// Stops listening (keys already swallowed still get their release swallowed).
    fn stop_capture(&mut self);
    /// The character `Ctrl+Alt+key` (that is AltGr+key) types with the current keyboard layout.
    fn altgr_char(&self, hotkey: Hotkey) -> Option<char>;
    /// The character printed on a layout-dependent key (punctuation), with the current layout.
    fn layout_char(&self, key: Vk) -> Option<char>;
}

/// What the HUD asks, through the app.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Start(MacroId),
    Stop(MacroId),
    SetOptions(MacroId, MacroOptions),
    StartCapture(MacroId),
    CancelCapture,
}

/// Where a captured key came from (written in the run log, to understand a capture afterwards).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    /// The low-level keyboard hook: keys typed in any window.
    GlobalHook,
    /// The keyboard of the HUD's own thread: keys typed while the HUD has the focus.
    HudWindow,
    /// A registered hotkey: Windows handed the key over as a hotkey, not as a key.
    Hotkey,
}

/// What the system reports (on Windows: hotkeys and hooks).
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A registered hotkey was pressed.
    Hotkey(HotkeyId),
    /// A click the guard swallowed: on a button a macro holds or clicks, made by hand (`on_hud`
    /// false), or anywhere on the HUD while a macro ran (`on_hud` true). The macros using `buttons`
    /// must stop.
    Click { buttons: ButtonSet, on_hud: bool },
    /// The capture of a new hotkey ended.
    Captured { outcome: Outcome, source: KeySource },
}

/// One macro as the engine publishes it.
#[derive(Clone, Debug, PartialEq)]
pub struct MacroSnapshot {
    pub id: MacroId,
    pub status: MacroStatus,
    pub hotkey: Hotkey,
    /// The hotkey's text, named with the current keyboard layout.
    pub hotkey_label: String,
    pub options: MacroOptions,
    /// « Multi-clic » runs but waits: the mouse is on the HUD.
    pub suspended: bool,
}

/// The engine's latest word for the HUD's one-off message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// Each notice gets the next number: a new number is something new to show.
    pub number: u64,
    /// The message to show; `None` takes the engine's previous message away (a countdown that ended).
    pub message: Option<Message>,
}

/// What the engine publishes after every change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub macros: Vec<MacroSnapshot>,
    pub capturing: Option<MacroId>,
    pub notice: Option<Notice>,
}

/// What a macro starts with: its definition and its saved settings.
#[derive(Clone, Copy, Debug)]
pub struct MacroSetup {
    pub def: MacroDef,
    pub hotkey: Hotkey,
    pub options: MacroOptions,
}

/// Who asked to start a macro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trigger {
    Hotkey,
    Hud,
}

/// Why a macro stops (for the run log).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cause {
    /// Its hotkey.
    Hotkey,
    /// « Arrêter » in the HUD.
    Hud,
    /// A real click of its button.
    Hand,
    /// A click on the HUD while it ran.
    HudClick,
    /// The macro of this index starts (one at a time, D-20).
    Other(usize),
    /// Its button changed.
    NewButton,
    /// MacroHUD quits.
    Shutdown,
    /// Windows refused a click.
    Failure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Run {
    Stopped,
    /// Started from the HUD: the macro acts at `at`; until then it shows as stopped, and a message
    /// counts down the seconds (`shown`: the number it shows).
    Arming {
        at: Instant,
        shown: u64,
    },
    /// The button is down, held by « Maintenir le clic ».
    Holding {
        button: MouseButton,
    },
    /// « Multi-clic » clicks.
    Clicking(Clicking),
}

/// The clicks of « Multi-clic » under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Clicking {
    button: MouseButton,
    /// The press of the current click ends then (`None`: the button is up).
    release_at: Option<Instant>,
    /// When the next press is due: on the beat, or the next look at the mouse while suspended.
    next_press: Instant,
    /// When the last press went (a new speed counts from it).
    last_press: Option<Instant>,
    /// The mouse is on the HUD: no press until it leaves.
    suspended: bool,
    /// For the run log: presses sent, releases also sent to the window of the press, since when.
    clicks: u64,
    delivered: u64,
    since: Instant,
}

#[derive(Debug)]
struct Slot {
    def: MacroDef,
    hotkey: Hotkey,
    registered: Option<HotkeyId>,
    /// Why the macro cannot run (its hotkey could not be registered at start).
    unavailable: Option<String>,
    options: MacroOptions,
    run: Run,
}

#[derive(Debug)]
struct Capture {
    index: usize,
    deadline: Instant,
}

/// The engine.
pub struct Engine<B: Backend> {
    backend: B,
    slots: Vec<Slot>,
    capture: Option<Capture>,
    notice: Option<Notice>,
}

/// The countdown shown between « Activer » and the macro's first action.
fn countdown_text(options: MacroOptions, seconds: u64) -> String {
    match options {
        MacroOptions::HoldClick { .. } => {
            format!("Activation dans {seconds} s : place la souris là où le clic doit rester enfoncé.")
        }
        MacroOptions::MultiClick { .. } => {
            format!("Activation dans {seconds} s : place la souris là où il faut cliquer.")
        }
    }
}

/// Whole seconds left until `at`, rounded up (2.3 s left: « 3 s »).
fn seconds_left(at: Instant, now: Instant) -> u64 {
    let millis = at.saturating_duration_since(now).as_millis();
    u64::try_from(millis.div_ceil(1000)).unwrap_or(u64::MAX)
}

/// How long each click of « Multi-clic » keeps its button down (D-23): half the time between two
/// presses, at most [`LONGEST_PRESS`]. A game that reads the mouse once a frame, 30 times a second
/// or more, sees both the press and the release.
pub fn press_time(rate: ClickRate) -> Duration {
    (rate.period() / 2).min(LONGEST_PRESS)
}

/// When the press after the one due at `due` (and sent at `now`) is due: one period later, on the
/// same beat. After a long delay (the computer was busy), the beat starts again from now, rather
/// than sending the missed clicks all at once.
fn next_beat(due: Instant, period: Duration, now: Instant) -> Instant {
    let next = due + period;
    if next > now { next } else { now + period }
}

/// « 9,8 » (seconds, one decimal, French).
fn seconds_text(duration: Duration) -> String {
    format!("{:.1}", duration.as_secs_f64()).replace('.', ",")
}

impl<B: Backend> Engine<B> {
    pub fn new(backend: B, setup: &[MacroSetup]) -> Self {
        let slots = setup
            .iter()
            .map(|s| Slot {
                def: s.def,
                hotkey: s.hotkey,
                registered: None,
                unavailable: None,
                options: s.options,
                run: Run::Stopped,
            })
            .collect();
        Engine { backend, slots, capture: None, notice: None }
    }

    #[cfg(test)]
    pub fn backend(&self) -> &B {
        &self.backend
    }

    #[cfg(test)]
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Registers the hotkeys. A macro whose hotkey cannot be registered becomes unavailable, with
    /// the reason; choosing another hotkey makes it available again.
    pub fn start(&mut self) {
        for i in 0..self.slots.len() {
            if !self.slots[i].def.available {
                continue;
            }
            let hotkey = self.slots[i].hotkey;
            let label = self.label(hotkey);
            let earlier_owner =
                self.slots[..i].iter().find(|s| s.def.available && s.hotkey == hotkey).map(|s| s.def.name);
            let reason = match earlier_owner {
                Some(other) => Some(format!("{label} est déjà le raccourci de « {other} » : choisis-en un autre.")),
                None => match self.backend.register_hotkey(hotkey) {
                    Ok(id) => {
                        self.slots[i].registered = Some(id);
                        crate::log::write(format!("Raccourci {label} enregistré pour « {} ».", self.slots[i].def.name));
                        None
                    }
                    Err(RegisterError::Taken) => Some(format!(
                        "{label} est déjà pris par Windows ou par un autre programme : choisis un autre raccourci."
                    )),
                    Err(RegisterError::Other(e)) => {
                        Some(format!("Windows refuse le raccourci {label} ({e}) : choisis-en un autre."))
                    }
                },
            };
            if let Some(reason) = reason {
                let name = self.slots[i].def.name;
                self.say(i, MessageLevel::Warning, format!("« {name} » est indisponible : {reason}"));
                self.slots[i].unavailable = Some(reason);
            }
        }
    }

    /// Handles a request of the HUD.
    pub fn command(&mut self, command: Command, now: Instant) {
        match command {
            Command::Start(id) => {
                if let Some(i) = self.find(id) {
                    self.start_macro(i, Trigger::Hud, now);
                }
            }
            Command::Stop(id) => {
                if let Some(i) = self.find(id) {
                    self.stop_macro(i, Cause::Hud, now);
                }
            }
            Command::SetOptions(id, options) => {
                if let Some(i) = self.find(id) {
                    self.set_options(i, options, now);
                }
            }
            Command::StartCapture(id) => {
                if let Some(i) = self.find(id) {
                    self.start_capture(i, now);
                }
            }
            Command::CancelCapture => {
                if let Some(capture) = self.capture.take() {
                    self.backend.stop_capture();
                    let label = self.label(self.slots[capture.index].hotkey);
                    self.say(capture.index, MessageLevel::Info, format!("Raccourci inchangé : {label}."));
                }
            }
        }
    }

    /// Handles a report of the system.
    pub fn event(&mut self, event: Event, now: Instant) {
        match event {
            Event::Hotkey(id) => {
                let Some(i) = self.slots.iter().position(|s| s.registered == Some(id)) else { return };
                if self.capture.is_some() {
                    // During a capture, a registered hotkey is a key the capture did not see: Windows
                    // handed it over as a hotkey. It is the answer, not an order to start or stop.
                    let hotkey = self.slots[i].hotkey;
                    self.captured(Outcome::Chosen(hotkey), KeySource::Hotkey);
                    return;
                }
                match self.slots[i].run {
                    Run::Stopped => self.start_macro(i, Trigger::Hotkey, now),
                    Run::Arming { .. } | Run::Holding { .. } | Run::Clicking(_) => {
                        self.stop_macro(i, Cause::Hotkey, now)
                    }
                }
            }
            Event::Click { buttons, on_hud } => {
                for i in 0..self.slots.len() {
                    let used = match self.slots[i].run {
                        Run::Holding { button } => Some(button),
                        Run::Clicking(c) => Some(c.button),
                        Run::Stopped | Run::Arming { .. } => None,
                    };
                    if !used.is_some_and(|button| buttons.contains(button)) {
                        continue;
                    }
                    let holding = matches!(self.slots[i].run, Run::Holding { .. });
                    self.stop_macro(i, if on_hud { Cause::HudClick } else { Cause::Hand }, now);
                    let name = self.slots[i].def.name;
                    // (« Multi-clic » writes the cause in its own last line.)
                    if on_hud {
                        if holding {
                            crate::log::write(format!("« {name} » arrêtée par un clic sur le HUD."));
                        }
                        self.say(
                            i,
                            MessageLevel::Info,
                            format!(
                                "« {name} » arrêtée par ton clic sur le HUD : clique à nouveau pour agir sur le HUD."
                            ),
                        );
                    } else {
                        if holding {
                            crate::log::write(format!("« {name} » arrêtée par un clic physique."));
                        }
                        self.say(i, MessageLevel::Info, format!("« {name} » arrêtée par ton clic."));
                    }
                }
            }
            Event::Captured { outcome, source } => self.captured(outcome, source),
        }
    }

    /// Does what is due at `now`: countdowns and delayed starts, the clicks of « Multi-clic »,
    /// captures that lasted too long.
    pub fn tick(&mut self, now: Instant) {
        for i in 0..self.slots.len() {
            match self.slots[i].run {
                Run::Arming { at, shown } => self.arming_tick(i, at, shown, now),
                Run::Clicking(_) => self.step_clicks(i, now),
                Run::Stopped | Run::Holding { .. } => {}
            }
        }
        if let Some(capture) = &self.capture
            && now >= capture.deadline
        {
            let index = capture.index;
            self.capture = None;
            self.backend.stop_capture();
            let label = self.label(self.slots[index].hotkey);
            self.say(index, MessageLevel::Info, format!("Aucune touche reçue : le raccourci reste {label}."));
        }
    }

    /// When [`Engine::tick`] has something to do next.
    pub fn next_deadline(&self) -> Option<Instant> {
        let runs = self.slots.iter().filter_map(|s| match s.run {
            // The next second of the countdown, or the start itself.
            Run::Arming { at, shown } if shown > 1 => {
                Some(at.checked_sub(Duration::from_secs(shown - 1)).unwrap_or(at))
            }
            Run::Arming { at, .. } => Some(at),
            // The end of the current press, else the next press.
            Run::Clicking(c) => Some(c.release_at.unwrap_or(c.next_press)),
            Run::Stopped | Run::Holding { .. } => None,
        });
        runs.chain(self.capture.as_ref().map(|c| c.deadline)).min()
    }

    /// Before quitting: releases everything held, stops listening, unregisters the hotkeys.
    pub fn shutdown(&mut self, now: Instant) {
        if self.capture.take().is_some() {
            self.backend.stop_capture();
        }
        for i in 0..self.slots.len() {
            self.stop_macro(i, Cause::Shutdown, now);
        }
        let _ = self.backend.watch(ButtonSet::EMPTY);
        for i in 0..self.slots.len() {
            if let Some(id) = self.slots[i].registered.take() {
                self.backend.unregister_hotkey(id);
            }
        }
    }

    /// The buttons macros hold or click right now: the guard watches them, and a crash test fires
    /// when one of them stops (`--essai-plantage`).
    pub fn guarded_buttons(&self) -> ButtonSet {
        self.slots.iter().fold(ButtonSet::EMPTY, |set, s| match s.run {
            Run::Holding { button } => set.with(button),
            Run::Clicking(c) => set.with(c.button),
            Run::Stopped | Run::Arming { .. } => set,
        })
    }

    /// What the HUD needs to know.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            macros: self
                .slots
                .iter()
                .map(|s| MacroSnapshot {
                    id: s.def.id,
                    status: match (&s.unavailable, s.run) {
                        (Some(reason), _) => MacroStatus::Unavailable(reason.clone()),
                        // Nothing happens during the countdown: the macro still shows as stopped.
                        (None, Run::Stopped | Run::Arming { .. }) => MacroStatus::Stopped,
                        (None, Run::Holding { .. } | Run::Clicking(_)) => MacroStatus::Active,
                    },
                    hotkey: s.hotkey,
                    hotkey_label: self.label(s.hotkey),
                    options: s.options,
                    suspended: matches!(s.run, Run::Clicking(c) if c.suspended),
                })
                .collect(),
            capturing: self.capture.as_ref().map(|c| self.slots[c.index].def.id),
            notice: self.notice.clone(),
        }
    }

    // --- Internals -------------------------------------------------------------------------------

    fn find(&self, id: MacroId) -> Option<usize> {
        self.slots.iter().position(|s| s.def.id == id && s.def.available)
    }

    fn label(&self, hotkey: Hotkey) -> String {
        hotkey.label(&|key| self.backend.layout_char(key))
    }

    fn next_notice_number(&self) -> u64 {
        self.notice.as_ref().map_or(1, |n| n.number + 1)
    }

    /// Shows a message about macro `i`.
    fn say(&mut self, i: usize, level: MessageLevel, text: impl Into<String>) {
        let message = Message::new(level, text).concerning(self.slots[i].def.id);
        self.show(message);
    }

    /// « Raccourci refusé » for macro `i`: `label` was refused, `text` says why and what stays.
    fn refuse(&mut self, i: usize, label: &str, text: String) {
        let message = Message::hotkey_refused(self.slots[i].def.id, label, text);
        self.show(message);
    }

    fn show(&mut self, message: Message) {
        crate::log::write(format!("Message ({:?}) : {}", message.level, message.text));
        let number = self.next_notice_number();
        self.notice = Some(Notice { number, message: Some(message) });
    }

    /// Takes the engine's current message away (the countdown it showed is over).
    fn withdraw(&mut self) {
        if self.notice.as_ref().is_some_and(|n| n.message.is_some()) {
            let number = self.next_notice_number();
            self.notice = Some(Notice { number, message: None });
        }
    }

    fn start_macro(&mut self, i: usize, trigger: Trigger, now: Instant) {
        let slot = &self.slots[i];
        if let Some(reason) = &slot.unavailable {
            let text = format!("« {} » est indisponible : {reason}", slot.def.name);
            self.say(i, MessageLevel::Warning, text);
            return;
        }
        if slot.run != Run::Stopped {
            return;
        }
        if trigger == Trigger::Hotkey && self.backend.pointer_on_hud() {
            // Refused as it is: whatever runs goes on.
            let label = self.label(self.slots[i].hotkey);
            self.say(i, MessageLevel::Warning, format!("Sors la souris du HUD avant d'appuyer sur {label}."));
            return;
        }
        // One macro at a time (D-20): the one that runs stops first, its button released.
        for j in 0..self.slots.len() {
            if j != i && self.slots[j].run != Run::Stopped {
                let (stopped, started) = (self.slots[j].def.name, self.slots[i].def.name);
                crate::log::write(format!("« {stopped} » s'arrête : « {started} » démarre (une macro à la fois)."));
                self.stop_macro(j, Cause::Other(i), now);
            }
        }
        match trigger {
            Trigger::Hud => {
                let seconds = HUD_START_DELAY.as_secs();
                self.slots[i].run = Run::Arming { at: now + HUD_START_DELAY, shown: seconds };
                self.say(i, MessageLevel::Info, countdown_text(self.slots[i].options, seconds));
            }
            Trigger::Hotkey => self.begin(i, trigger, now),
        }
    }

    /// The countdown of a macro started from the HUD: a message per second, then the start.
    fn arming_tick(&mut self, i: usize, at: Instant, shown: u64, now: Instant) {
        if now < at {
            let left = seconds_left(at, now);
            if left < shown {
                self.slots[i].run = Run::Arming { at, shown: left };
                self.say(i, MessageLevel::Info, countdown_text(self.slots[i].options, left));
            }
            return;
        }
        self.slots[i].run = Run::Stopped;
        if self.backend.pointer_on_hud() {
            let nothing = match self.slots[i].options {
                MacroOptions::HoldClick { .. } => "rien n'a été enfoncé",
                MacroOptions::MultiClick { .. } => "aucun clic n'est parti",
            };
            self.say(
                i,
                MessageLevel::Warning,
                format!("La souris était sur le HUD : {nothing}. Recommence en la plaçant ailleurs."),
            );
        } else {
            // The countdown is over: its message goes, the macro acts.
            self.withdraw();
            self.begin(i, Trigger::Hud, now);
        }
    }

    /// The macro's first action: the press, or the first click.
    fn begin(&mut self, i: usize, trigger: Trigger, now: Instant) {
        match self.slots[i].options {
            MacroOptions::HoldClick { button } => self.press_now(i, button),
            MacroOptions::MultiClick { button, rate } => self.start_clicking(i, button, rate, trigger, now),
        }
    }

    fn press_now(&mut self, i: usize, button: MouseButton) {
        self.slots[i].run = Run::Stopped;
        // The guard first: no button is ever down without a way for the user to take it back.
        if let Err(e) = self.backend.watch(self.guarded_buttons().with(button)) {
            self.say(
                i,
                MessageLevel::Error,
                format!("Impossible de surveiller la souris ({e}) : rien n'a été enfoncé."),
            );
            let _ = self.backend.watch(self.guarded_buttons());
            return;
        }
        let pressed = if self.backend.is_down(button) {
            self.backend.adopt(button);
            Ok(())
        } else {
            self.backend.press(button)
        };
        match pressed {
            Ok(()) => {
                self.slots[i].run = Run::Holding { button };
                crate::log::write(format!("« {} » : {} maintenu.", self.slots[i].def.name, button.phrase()));
            }
            Err(e) => {
                let _ = self.backend.watch(self.guarded_buttons());
                self.say(i, MessageLevel::Error, format!("Windows a refusé d'enfoncer {} ({e}).", button.phrase()));
            }
        }
    }

    fn start_clicking(&mut self, i: usize, button: MouseButton, rate: ClickRate, trigger: Trigger, now: Instant) {
        self.slots[i].run = Run::Stopped;
        // The guard first: no click without a way for the user to take the mouse back.
        if let Err(e) = self.backend.watch(self.guarded_buttons().with(button)) {
            self.say(
                i,
                MessageLevel::Error,
                format!("Impossible de surveiller la souris ({e}) : aucun clic n'est parti."),
            );
            let _ = self.backend.watch(self.guarded_buttons());
            return;
        }
        let way = match trigger {
            Trigger::Hotkey => format!("par {}", self.label(self.slots[i].hotkey)),
            Trigger::Hud => "par le HUD, après le décompte".to_string(),
        };
        crate::log::write(format!(
            "« {} » démarre : {}, {}, {way}.",
            self.slots[i].def.name,
            button.phrase(),
            rate.label()
        ));
        self.slots[i].run = Run::Clicking(Clicking {
            button,
            release_at: None,
            next_press: now,
            last_press: None,
            suspended: false,
            clicks: 0,
            delivered: 0,
            since: now,
        });
        self.step_clicks(i, now);
    }

    /// The clicks of « Multi-clic » that are due at `now`: the end of the current press, the next
    /// press (unless the mouse is on the HUD).
    fn step_clicks(&mut self, i: usize, now: Instant) {
        let Run::Clicking(mut c) = self.slots[i].run else { return };
        let MacroOptions::MultiClick { rate, .. } = self.slots[i].options else { return };
        let name = self.slots[i].def.name;
        if let Some(at) = c.release_at
            && now >= at
        {
            match self.backend.click_up(c.button) {
                Ok(delivered) => {
                    c.release_at = None;
                    c.delivered += u64::from(delivered);
                }
                Err(e) => {
                    // Still down: no more clicks, the user is told how to free it.
                    c.release_at = None;
                    self.slots[i].run = Run::Clicking(c);
                    self.stop_macro(i, Cause::Failure, now);
                    self.say(
                        i,
                        MessageLevel::Error,
                        format!(
                            "Windows a refusé de relâcher {} ({e}) : clique-le une fois pour le libérer.",
                            c.button.phrase()
                        ),
                    );
                    return;
                }
            }
        }
        if c.release_at.is_none() && now >= c.next_press {
            if self.backend.pointer_on_hud() {
                // No automatic click on the HUD (D-22): it waits for the mouse to leave.
                if !c.suspended {
                    c.suspended = true;
                    crate::log::write(format!("« {name} » suspendue : la souris est sur le HUD."));
                }
                c.next_press = now + HUD_CHECK;
            } else {
                if c.suspended {
                    c.suspended = false;
                    crate::log::write(format!("« {name} » reprend : la souris a quitté le HUD."));
                }
                match self.backend.click_down(c.button) {
                    Ok(()) => {
                        c.clicks += 1;
                        c.last_press = Some(now);
                        c.release_at = Some(now + press_time(rate));
                        c.next_press = next_beat(c.next_press, rate.period(), now);
                    }
                    Err(e) => {
                        self.slots[i].run = Run::Clicking(c);
                        self.stop_macro(i, Cause::Failure, now);
                        let text = format!(
                            "Windows a refusé de cliquer {} ({e}) : « {name} » est arrêtée.",
                            c.button.phrase()
                        );
                        self.say(i, MessageLevel::Error, text);
                        return;
                    }
                }
            }
        }
        self.slots[i].run = Run::Clicking(c);
    }

    fn stop_macro(&mut self, i: usize, cause: Cause, now: Instant) {
        match self.slots[i].run {
            Run::Stopped => {}
            Run::Arming { .. } => {
                // Nothing happened yet: the countdown just stops, and its message goes.
                self.slots[i].run = Run::Stopped;
                self.withdraw();
            }
            Run::Holding { button } => {
                // Release first: nothing may delay it.
                let released = self.backend.release(button);
                self.slots[i].run = Run::Stopped;
                let _ = self.backend.watch(self.guarded_buttons());
                match released {
                    Ok(()) => {
                        crate::log::write(format!("« {} » : {} relâché.", self.slots[i].def.name, button.phrase()))
                    }
                    Err(e) => self.say(
                        i,
                        MessageLevel::Error,
                        format!(
                            "Windows a refusé de relâcher {} ({e}) : clique-le une fois pour le libérer.",
                            button.phrase()
                        ),
                    ),
                }
            }
            Run::Clicking(c) => {
                // A press under way is released first; its release reaches the window of the press.
                let released = c.release_at.map(|_| self.backend.release(c.button));
                self.slots[i].run = Run::Stopped;
                let _ = self.backend.watch(self.guarded_buttons());
                let why = match cause {
                    Cause::Hotkey => format!("par {}", self.label(self.slots[i].hotkey)),
                    Cause::Hud => "par le HUD".to_string(),
                    Cause::Hand => "par un vrai clic".to_string(),
                    Cause::HudClick => "par un clic sur le HUD".to_string(),
                    Cause::Other(j) => format!("pour « {} »", self.slots[j].def.name),
                    Cause::NewButton => "bouton changé".to_string(),
                    Cause::Shutdown => "à la fermeture".to_string(),
                    Cause::Failure => "refus de Windows".to_string(),
                };
                let delivered = match c.delivered {
                    0 => String::new(),
                    n => format!(" ; relâchements remis aussi à la fenêtre de l'appui : {n}"),
                };
                crate::log::write(format!(
                    "« {} » arrêtée ({why}) : {} clics envoyés en {} s{delivered}.",
                    self.slots[i].def.name,
                    c.clicks,
                    seconds_text(now.saturating_duration_since(c.since)),
                ));
                if let Some(Err(e)) = released {
                    let text = format!(
                        "Windows a refusé de relâcher {} ({e}) : clique-le une fois pour le libérer.",
                        c.button.phrase()
                    );
                    self.say(i, MessageLevel::Error, text);
                }
            }
        }
    }

    fn set_options(&mut self, i: usize, options: MacroOptions, now: Instant) {
        let current = self.slots[i].options;
        // Options of another kind of macro are a mistake: nothing changes.
        if current == options || std::mem::discriminant(&current) != std::mem::discriminant(&options) {
            return;
        }
        match self.slots[i].run {
            // Nothing acts yet: the countdown goes on with the new options.
            Run::Stopped | Run::Arming { .. } => self.slots[i].options = options,
            Run::Clicking(mut c) if options.button() == c.button => {
                // A new speed: it counts from the next click.
                self.slots[i].options = options;
                let rate = match options {
                    MacroOptions::MultiClick { rate, .. } => rate,
                    MacroOptions::HoldClick { .. } => return,
                };
                if !c.suspended
                    && let Some(last) = c.last_press
                {
                    c.next_press = (last + rate.period()).max(now);
                }
                self.slots[i].run = Run::Clicking(c);
                crate::log::write(format!(
                    "« {} » : rapidité {}, dès le clic suivant.",
                    self.slots[i].def.name,
                    rate.label()
                ));
            }
            Run::Holding { .. } | Run::Clicking(_) => {
                // A new button: what the macro holds is released before anything changes.
                self.stop_macro(i, Cause::NewButton, now);
                self.slots[i].options = options;
                let name = self.slots[i].def.name;
                self.say(i, MessageLevel::Info, format!("Bouton changé : « {name} » est arrêtée."));
            }
        }
    }

    fn start_capture(&mut self, i: usize, now: Instant) {
        if let Some(capture) = &self.capture {
            if capture.index == i {
                return;
            }
            self.capture = None;
            self.backend.stop_capture();
        }
        match self.backend.start_capture() {
            Ok(()) => self.capture = Some(Capture { index: i, deadline: now + CAPTURE_TIMEOUT }),
            Err(e) => {
                let label = self.label(self.slots[i].hotkey);
                self.say(
                    i,
                    MessageLevel::Error,
                    format!("Impossible d'écouter le clavier ({e}) : le raccourci reste {label}."),
                );
            }
        }
    }

    fn captured(&mut self, outcome: Outcome, source: KeySource) {
        // A result that comes after a cancellation (or after another one) is ignored.
        let Some(capture) = self.capture.take() else { return };
        self.backend.stop_capture();
        crate::log::write(format!(
            "Capture : {outcome:?}, reçue par {}.",
            match source {
                KeySource::GlobalHook => "le crochet clavier global",
                KeySource::HudWindow => "la fenêtre du HUD",
                KeySource::Hotkey => "un raccourci déjà enregistré",
            }
        ));
        match outcome {
            Outcome::Cancelled => {
                let label = self.label(self.slots[capture.index].hotkey);
                self.say(capture.index, MessageLevel::Info, format!("Raccourci inchangé : {label}."));
            }
            Outcome::Chosen(hotkey) => self.change_hotkey(capture.index, hotkey),
        }
    }

    /// Replaces the hotkey of macro `i`. A refused hotkey changes nothing: the old one stays active.
    fn change_hotkey(&mut self, i: usize, hotkey: Hotkey) {
        let label = self.label(hotkey);
        let current = self.label(self.slots[i].hotkey);
        let active = self.slots[i].registered.is_some();
        let keep = if active { format!(" {current} reste actif.") } else { String::new() };

        if hotkey == self.slots[i].hotkey && active {
            self.say(i, MessageLevel::Info, format!("Raccourci inchangé : {label}."));
            return;
        }
        let owner = self
            .slots
            .iter()
            .enumerate()
            .find(|(j, s)| *j != i && s.def.available && s.hotkey == hotkey)
            .map(|(_, s)| s.def.name);
        if let Some(other) = owner {
            self.refuse(i, &label, format!("{label} est déjà le raccourci de « {other} ».{keep}"));
            return;
        }
        if let Some(why) = hotkey::refusal(&hotkey, &label) {
            self.refuse(i, &label, format!("{why}{keep}"));
            return;
        }
        if let Some(c) = self.backend.altgr_char(hotkey) {
            self.refuse(i, &label, format!("{label} sert à écrire « {c} ».{keep}"));
            return;
        }
        match self.backend.register_hotkey(hotkey) {
            Err(RegisterError::Taken) => {
                self.refuse(i, &label, format!("{label} est déjà pris par Windows ou un autre programme.{keep}"));
            }
            Err(RegisterError::Other(e)) => {
                self.refuse(i, &label, format!("Windows refuse {label} ({e}).{keep}"));
            }
            Ok(id) => {
                if let Some(old) = self.slots[i].registered.replace(id) {
                    self.backend.unregister_hotkey(old);
                }
                self.slots[i].hotkey = hotkey;
                self.slots[i].unavailable = None;
                let name = self.slots[i].def.name;
                self.say(i, MessageLevel::Info, format!("Nouveau raccourci de « {name} » : {label}."));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::MessageKind;
    use crate::hotkey::{Modifier, Modifiers, vk};
    use crate::macros::{HOLD_CLICK, MULTI_CLICK};
    use MouseButton::*;

    #[derive(Clone, Debug, PartialEq)]
    enum Call {
        Watch(ButtonSet),
        Press(MouseButton),
        Adopt(MouseButton),
        Release(MouseButton),
        ClickDown(MouseButton),
        ClickUp(MouseButton),
        Register(Hotkey),
        Unregister(HotkeyId),
        StartCapture,
        StopCapture,
    }

    /// A pretend system: records what the engine does to it.
    #[derive(Default)]
    struct Fake {
        calls: Vec<Call>,
        down: ButtonSet,
        pointer_on_hud: bool,
        taken_elsewhere: Vec<Hotkey>,
        registered: Vec<(HotkeyId, Hotkey)>,
        next_id: HotkeyId,
        fail_watch: bool,
        fail_press: bool,
        fail_click_down: bool,
        fail_click_up: bool,
        /// What `click_up` answers: the release was also sent to the window of the press.
        delivered: bool,
        altgr: Option<(Hotkey, char)>,
    }

    impl Backend for Fake {
        fn watch(&mut self, held: ButtonSet) -> Result<(), String> {
            if self.fail_watch && !held.is_empty() {
                return Err("crochet refusé".into());
            }
            self.calls.push(Call::Watch(held));
            Ok(())
        }
        fn is_down(&self, button: MouseButton) -> bool {
            self.down.contains(button)
        }
        fn press(&mut self, button: MouseButton) -> Result<(), String> {
            if self.fail_press {
                return Err("SendInput : erreur 5".into());
            }
            self.calls.push(Call::Press(button));
            self.down = self.down.with(button);
            Ok(())
        }
        fn adopt(&mut self, button: MouseButton) {
            self.calls.push(Call::Adopt(button));
        }
        fn release(&mut self, button: MouseButton) -> Result<(), String> {
            self.calls.push(Call::Release(button));
            self.down = self.down.without(button);
            Ok(())
        }
        fn click_down(&mut self, button: MouseButton) -> Result<(), String> {
            if self.fail_click_down {
                return Err("SendInput : erreur 5".into());
            }
            self.calls.push(Call::ClickDown(button));
            self.down = self.down.with(button);
            Ok(())
        }
        fn click_up(&mut self, button: MouseButton) -> Result<bool, String> {
            if self.fail_click_up {
                return Err("SendInput : erreur 5".into());
            }
            self.calls.push(Call::ClickUp(button));
            self.down = self.down.without(button);
            Ok(self.delivered)
        }
        fn pointer_on_hud(&self) -> bool {
            self.pointer_on_hud
        }
        fn register_hotkey(&mut self, hotkey: Hotkey) -> Result<HotkeyId, RegisterError> {
            if self.taken_elsewhere.contains(&hotkey) || self.registered.iter().any(|(_, h)| *h == hotkey) {
                return Err(RegisterError::Taken);
            }
            self.calls.push(Call::Register(hotkey));
            self.next_id += 1;
            self.registered.push((self.next_id, hotkey));
            Ok(self.next_id)
        }
        fn unregister_hotkey(&mut self, id: HotkeyId) {
            self.calls.push(Call::Unregister(id));
            self.registered.retain(|(i, _)| *i != id);
        }
        fn start_capture(&mut self) -> Result<(), String> {
            self.calls.push(Call::StartCapture);
            Ok(())
        }
        fn stop_capture(&mut self) {
            self.calls.push(Call::StopCapture);
        }
        fn altgr_char(&self, hotkey: Hotkey) -> Option<char> {
            self.altgr.filter(|(h, _)| *h == hotkey).map(|(_, c)| c)
        }
        fn layout_char(&self, _key: Vk) -> Option<char> {
            None
        }
    }

    const ID: MacroId = MacroId::HOLD_CLICK;

    fn setup(def: MacroDef) -> MacroSetup {
        MacroSetup { def, hotkey: def.default_hotkey, options: def.default_options }
    }

    fn engine_with(fake: Fake) -> Engine<Fake> {
        let mut engine = Engine::new(fake, &[setup(HOLD_CLICK)]);
        engine.start();
        engine.backend_mut().calls.clear();
        engine
    }

    fn engine() -> Engine<Fake> {
        engine_with(Fake::default())
    }

    fn hotkey_id(engine: &Engine<Fake>, index: usize) -> HotkeyId {
        engine.slots[index].registered.expect("registered hotkey")
    }

    fn press_hotkey(engine: &mut Engine<Fake>, now: Instant) {
        let id = hotkey_id(engine, 0);
        engine.event(Event::Hotkey(id), now);
    }

    fn status(engine: &Engine<Fake>) -> MacroStatus {
        engine.snapshot().macros[0].status.clone()
    }

    fn message(engine: &Engine<Fake>) -> Message {
        engine.snapshot().notice.expect("a notice").message.expect("a message, not a withdrawal")
    }

    /// The engine's current word: `None` before any message, `Some(None)` after a withdrawal.
    fn notice_message(engine: &Engine<Fake>) -> Option<Option<Message>> {
        engine.snapshot().notice.map(|n| n.message)
    }

    fn click(button: MouseButton) -> Event {
        Event::Click { buttons: ButtonSet::only(button), on_hud: false }
    }

    fn captured(outcome: Outcome) -> Event {
        Event::Captured { outcome, source: KeySource::GlobalHook }
    }

    fn calls(engine: &mut Engine<Fake>) -> Vec<Call> {
        std::mem::take(&mut engine.backend_mut().calls)
    }

    fn ctrl_alt(key: Vk) -> Hotkey {
        Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Alt), key)
    }

    #[test]
    fn startup_registers_f8() {
        let mut e = Engine::new(Fake::default(), &[setup(HOLD_CLICK)]);
        e.start();
        assert_eq!(e.backend().calls, vec![Call::Register(Hotkey::F8)]);
        let s = e.snapshot();
        assert_eq!(s.macros[0].hotkey_label, "F8");
        assert_eq!(s.macros[0].status, MacroStatus::Stopped);
        assert_eq!(s.notice, None);
    }

    #[test]
    fn the_hotkey_presses_then_releases_with_the_guard_around() {
        let mut e = engine();
        let now = Instant::now();
        press_hotkey(&mut e, now);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::Press(Left)]);
        assert_eq!(status(&e), MacroStatus::Active);
        press_hotkey(&mut e, now);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(status(&e), MacroStatus::Stopped);
    }

    #[test]
    fn a_real_click_on_the_held_button_stops_the_macro() {
        let mut e = engine();
        let now = Instant::now();
        press_hotkey(&mut e, now);
        calls(&mut e);
        e.event(click(Right), now);
        assert!(calls(&mut e).is_empty(), "another button changes nothing");
        assert_eq!(status(&e), MacroStatus::Active);
        e.event(click(Left), now);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(message(&e).level, MessageLevel::Info);
        // A late duplicate does nothing.
        e.event(click(Left), now);
        assert!(calls(&mut e).is_empty());
    }

    #[test]
    fn changing_the_button_while_active_releases_the_old_one_and_presses_nothing() {
        let mut e = engine();
        let now = Instant::now();
        press_hotkey(&mut e, now);
        calls(&mut e);
        e.command(Command::SetOptions(ID, MacroOptions::HoldClick { button: Right }), now);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(e.snapshot().macros[0].options, MacroOptions::HoldClick { button: Right });
        assert!(e.backend().down.is_empty(), "no button left down");
        press_hotkey(&mut e, now);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Right)), Call::Press(Right)]);
    }

    #[test]
    fn changing_the_button_while_stopped_just_changes_it() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::SetOptions(ID, MacroOptions::HoldClick { button: Middle }), now);
        assert!(calls(&mut e).is_empty());
        assert_eq!(e.snapshot().notice, None);
        e.command(Command::SetOptions(ID, MacroOptions::HoldClick { button: Middle }), now);
        assert!(calls(&mut e).is_empty());
    }

    #[test]
    fn a_button_already_held_by_hand_is_taken_over_not_pressed_again() {
        let mut e = engine_with(Fake { down: ButtonSet::only(Left), ..Fake::default() });
        let now = Instant::now();
        press_hotkey(&mut e, now);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::Adopt(Left)]);
        assert_eq!(status(&e), MacroStatus::Active);
        press_hotkey(&mut e, now);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
    }

    #[test]
    fn started_from_the_hud_it_counts_down_then_presses() {
        let mut e = engine();
        let now = Instant::now();
        let s = Duration::from_secs(1);
        e.command(Command::Start(ID), now);
        assert!(calls(&mut e).is_empty());
        // Nothing is pressed yet: the macro shows as stopped, a message counts down.
        assert_eq!(status(&e), MacroStatus::Stopped);
        let first = message(&e);
        assert_eq!(first.level, MessageLevel::Info);
        assert_eq!(first.text, "Activation dans 3 s : place la souris là où le clic doit rester enfoncé.");
        assert_eq!(e.next_deadline(), Some(now + s));
        e.tick(now + s / 2);
        assert_eq!(message(&e), first, "no new message before the next second");
        e.tick(now + s);
        assert_eq!(message(&e).text, "Activation dans 2 s : place la souris là où le clic doit rester enfoncé.");
        assert_eq!(e.next_deadline(), Some(now + 2 * s));
        e.tick(now + 2 * s);
        assert_eq!(message(&e).text, "Activation dans 1 s : place la souris là où le clic doit rester enfoncé.");
        assert_eq!(e.next_deadline(), Some(now + HUD_START_DELAY));
        assert!(calls(&mut e).is_empty());
        assert_eq!(status(&e), MacroStatus::Stopped);
        e.tick(now + HUD_START_DELAY);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::Press(Left)]);
        assert_eq!(status(&e), MacroStatus::Active);
        assert_eq!(notice_message(&e), Some(None), "the countdown is taken away at the press");
        assert_eq!(e.next_deadline(), None);
        e.command(Command::Stop(ID), now);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
    }

    #[test]
    fn a_late_tick_skips_to_the_right_second() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::Start(ID), now);
        e.tick(now + Duration::from_millis(2_300));
        assert!(message(&e).text.starts_with("Activation dans 1 s"), "{}", message(&e).text);
        assert!(calls(&mut e).is_empty());
    }

    #[test]
    fn cancelling_the_countdown_takes_its_message_away() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::Start(ID), now);
        press_hotkey(&mut e, now + Duration::from_secs(1));
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(notice_message(&e), Some(None));
        e.tick(now + HUD_START_DELAY);
        assert!(calls(&mut e).is_empty(), "nothing pressed after a cancel");
        // A second « Activer » during the countdown changes nothing.
        e.command(Command::Start(ID), now);
        e.command(Command::Start(ID), now + Duration::from_millis(500));
        assert!(message(&e).text.starts_with("Activation dans 3 s"));
        e.command(Command::Stop(ID), now);
        assert_eq!(notice_message(&e), Some(None));
    }

    #[test]
    fn changing_the_button_during_the_countdown_keeps_it_going_with_the_new_button() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::Start(ID), now);
        e.command(Command::SetOptions(ID, MacroOptions::HoldClick { button: Middle }), now);
        assert_eq!(e.snapshot().macros[0].options, MacroOptions::HoldClick { button: Middle });
        assert!(message(&e).text.starts_with("Activation dans 3 s"), "no « arrêtée » message");
        e.tick(now + HUD_START_DELAY);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Middle)), Call::Press(Middle)]);
    }

    #[test]
    fn a_delayed_press_is_dropped_if_the_pointer_is_still_on_the_hud() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::Start(ID), now);
        e.backend_mut().pointer_on_hud = true;
        e.tick(now + HUD_START_DELAY);
        assert!(calls(&mut e).is_empty());
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(message(&e).level, MessageLevel::Warning);
    }

    #[test]
    fn the_hotkey_or_stop_cancels_a_delayed_press() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::Start(ID), now);
        press_hotkey(&mut e, now);
        assert_eq!(status(&e), MacroStatus::Stopped);
        e.tick(now + HUD_START_DELAY);
        assert!(calls(&mut e).is_empty());
        e.command(Command::Start(ID), now);
        e.command(Command::Stop(ID), now);
        e.tick(now + HUD_START_DELAY);
        assert!(calls(&mut e).is_empty());
        assert_eq!(status(&e), MacroStatus::Stopped);
    }

    #[test]
    fn a_click_on_the_hud_during_a_hold_stops_it() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::SetOptions(ID, MacroOptions::HoldClick { button: Right }), now);
        press_hotkey(&mut e, now);
        calls(&mut e);
        // A left click on the HUD (it cannot reach the HUD while the other window holds the mouse).
        e.event(Event::Click { buttons: ButtonSet::only(Right), on_hud: true }, now);
        assert_eq!(calls(&mut e), vec![Call::Release(Right), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(message(&e).level, MessageLevel::Info);
        assert!(message(&e).text.contains("clique à nouveau pour agir sur le HUD"), "{}", message(&e).text);
    }

    #[test]
    fn during_a_capture_a_registered_hotkey_is_the_captured_key() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::StartCapture(ID), now);
        calls(&mut e);
        // The HUD had the focus: F8 reached Windows' hotkeys instead of the capture.
        press_hotkey(&mut e, now);
        let made = calls(&mut e);
        assert!(!made.iter().any(|c| matches!(c, Call::Press(_) | Call::Watch(_))), "no press: {made:?}");
        assert_eq!(made, vec![Call::StopCapture]);
        assert_eq!(e.snapshot().capturing, None);
        assert_eq!(message(&e).text, "Raccourci inchangé : F8.");
        assert_eq!(status(&e), MacroStatus::Stopped);
        // Outside a capture, F8 toggles again.
        press_hotkey(&mut e, now);
        assert_eq!(status(&e), MacroStatus::Active);
    }

    #[test]
    fn the_first_capture_result_wins() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::StartCapture(ID), now);
        let f9 = Hotkey::new(Modifiers::NONE, vk::F9);
        e.event(Event::Captured { outcome: Outcome::Chosen(f9), source: KeySource::HudWindow }, now);
        e.event(captured(Outcome::Chosen(ctrl_alt(vk::KEY_M))), now);
        assert_eq!(e.snapshot().macros[0].hotkey, f9);
    }

    #[test]
    fn the_hotkey_with_the_pointer_on_the_hud_presses_nothing() {
        let mut e = engine_with(Fake { pointer_on_hud: true, ..Fake::default() });
        press_hotkey(&mut e, Instant::now());
        assert!(calls(&mut e).is_empty());
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(message(&e).text, "Sors la souris du HUD avant d'appuyer sur F8.");
    }

    #[test]
    fn nothing_is_pressed_without_the_guard() {
        let mut e = engine_with(Fake { fail_watch: true, ..Fake::default() });
        press_hotkey(&mut e, Instant::now());
        assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::Press(_) | Call::Adopt(_))));
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(message(&e).level, MessageLevel::Error);
    }

    #[test]
    fn a_refused_press_drops_the_guard_again() {
        let mut e = engine_with(Fake { fail_press: true, ..Fake::default() });
        press_hotkey(&mut e, Instant::now());
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(message(&e).level, MessageLevel::Error);
    }

    #[test]
    fn a_new_hotkey_replaces_the_old_one() {
        let mut e = engine();
        let now = Instant::now();
        let old = hotkey_id(&e, 0);
        e.command(Command::StartCapture(ID), now);
        assert_eq!(e.snapshot().capturing, Some(ID));
        let new = ctrl_alt(vk::KEY_M);
        e.event(captured(Outcome::Chosen(new)), now);
        assert_eq!(
            calls(&mut e),
            vec![Call::StartCapture, Call::StopCapture, Call::Register(new), Call::Unregister(old)]
        );
        let s = e.snapshot();
        assert_eq!(s.capturing, None);
        assert_eq!(s.macros[0].hotkey, new);
        assert_eq!(s.macros[0].hotkey_label, "Ctrl+Alt+M");
        assert_eq!(message(&e).text, "Nouveau raccourci de « Maintenir le clic » : Ctrl+Alt+M.");
        // The old hotkey no longer does anything, the new one does.
        e.event(Event::Hotkey(old), now);
        assert!(calls(&mut e).is_empty());
        press_hotkey(&mut e, now);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::Press(Left)]);
    }

    #[test]
    fn a_hotkey_taken_elsewhere_is_refused_and_the_old_one_stays() {
        let new = ctrl_alt(vk::KEY_M);
        let mut e = engine_with(Fake { taken_elsewhere: vec![new], ..Fake::default() });
        let now = Instant::now();
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Chosen(new)), now);
        let made = calls(&mut e);
        assert!(!made.iter().any(|c| matches!(c, Call::Unregister(_) | Call::Register(_))), "{made:?}");
        assert_eq!(e.snapshot().macros[0].hotkey, Hotkey::F8);
        // « Raccourci refusé » (D-16): a warning about this macro, naming the refused key.
        let m = message(&e);
        assert_eq!(
            m,
            Message::hotkey_refused(
                ID,
                "Ctrl+Alt+M",
                "Ctrl+Alt+M est déjà pris par Windows ou un autre programme. F8 reste actif."
            )
        );
        assert_eq!(m.level, MessageLevel::Warning);
        press_hotkey(&mut e, now);
        assert_eq!(status(&e), MacroStatus::Active, "F8 still works");
    }

    #[test]
    fn a_hotkey_of_another_macro_is_refused() {
        let mut second = HOLD_CLICK;
        second.id = MacroId("deuxieme");
        second.name = "Deuxième";
        second.default_hotkey = Hotkey::new(Modifiers::NONE, vk::F9);
        second.default_options = MacroOptions::HoldClick { button: Right };
        let mut e = Engine::new(Fake::default(), &[setup(HOLD_CLICK), setup(second)]);
        e.start();
        calls(&mut e);
        let now = Instant::now();
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Chosen(second.default_hotkey)), now);
        assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::Register(_))));
        assert_eq!(message(&e).text, "F9 est déjà le raccourci de « Deuxième ». F8 reste actif.");
        assert_eq!(message(&e).kind, MessageKind::HotkeyRefused { key: "F9".into() });
        // Each macro toggles with its own hotkey, one at a time (D-20).
        press_hotkey(&mut e, now);
        let id2 = hotkey_id(&e, 1);
        e.event(Event::Hotkey(id2), now);
        assert_eq!(
            calls(&mut e),
            vec![
                Call::Watch(ButtonSet::only(Left)),
                Call::Press(Left),
                Call::Release(Left),
                Call::Watch(ButtonSet::EMPTY),
                Call::Watch(ButtonSet::only(Right)),
                Call::Press(Right),
            ]
        );
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Stopped);
        assert_eq!(e.snapshot().macros[1].status, MacroStatus::Active);
        e.event(click(Left), now);
        assert!(calls(&mut e).is_empty(), "the first macro no longer holds anything");
    }

    #[test]
    fn starting_a_macro_stops_the_other_first_even_on_the_same_button() {
        let mut second = HOLD_CLICK;
        second.id = MacroId("deuxieme");
        second.name = "Deuxième";
        second.default_hotkey = Hotkey::new(Modifiers::NONE, vk::F9);
        let mut e = Engine::new(Fake::default(), &[setup(HOLD_CLICK), setup(second)]);
        e.start();
        calls(&mut e);
        let now = Instant::now();
        press_hotkey(&mut e, now);
        calls(&mut e);
        let id2 = hotkey_id(&e, 1);
        e.event(Event::Hotkey(id2), now);
        assert_eq!(
            calls(&mut e),
            vec![
                Call::Release(Left),
                Call::Watch(ButtonSet::EMPTY),
                Call::Watch(ButtonSet::only(Left)),
                Call::Press(Left)
            ]
        );
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Stopped);
        assert_eq!(e.snapshot().macros[1].status, MacroStatus::Active);
        // From the HUD: the countdown of the first stops the second at once, nothing is down after.
        e.command(Command::Start(ID), now);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
        assert!(e.backend().down.is_empty());
        e.tick(now + HUD_START_DELAY);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::Press(Left)]);
        assert_eq!(e.snapshot().macros[1].status, MacroStatus::Stopped);
    }

    #[test]
    fn hotkeys_breaking_typing_or_windows_are_refused() {
        let mut e = engine();
        let now = Instant::now();
        for (refused, text) in [
            (
                Hotkey::new(Modifiers::NONE, vk::KEY_M),
                "M empêcherait d'écrire (ajoute Ctrl, Alt ou Win). F8 reste actif.",
            ),
            (
                Hotkey::new(Modifiers::NONE.with(Modifier::Alt), vk::TAB),
                "Alt+Tab est déjà pris par Windows. F8 reste actif.",
            ),
            (Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl), vk::KEY_C), "Ctrl+C sert à copier. F8 reste actif."),
        ] {
            e.command(Command::StartCapture(ID), now);
            e.event(captured(Outcome::Chosen(refused)), now);
            assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::Register(_))));
            let m = message(&e);
            assert_eq!(m.level, MessageLevel::Warning);
            assert_eq!(m.text, text);
            assert_eq!(m.about, Some(ID));
            assert_eq!(m.kind, MessageKind::HotkeyRefused { key: refused.label(&|_| None) });
        }
    }

    #[test]
    fn altgr_characters_are_refused() {
        let euro = ctrl_alt(vk::KEY_E);
        let mut e = engine_with(Fake { altgr: Some((euro, '€')), ..Fake::default() });
        let now = Instant::now();
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Chosen(euro)), now);
        assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::Register(_))));
        assert!(message(&e).text.contains("« € »"), "{}", message(&e).text);
    }

    #[test]
    fn choosing_the_same_hotkey_or_escape_changes_nothing() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Chosen(Hotkey::F8)), now);
        assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::Register(_) | Call::Unregister(_))));
        assert_eq!(message(&e).text, "Raccourci inchangé : F8.");
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Cancelled), now);
        assert_eq!(message(&e).text, "Raccourci inchangé : F8.");
        assert_eq!(e.snapshot().capturing, None);
    }

    #[test]
    fn a_capture_ends_by_itself_or_from_the_hud() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::StartCapture(ID), now);
        assert_eq!(e.next_deadline(), Some(now + CAPTURE_TIMEOUT));
        e.tick(now + CAPTURE_TIMEOUT);
        assert_eq!(calls(&mut e), vec![Call::StartCapture, Call::StopCapture]);
        assert_eq!(e.snapshot().capturing, None);
        assert_eq!(message(&e).text, "Aucune touche reçue : le raccourci reste F8.");

        e.command(Command::StartCapture(ID), now);
        e.command(Command::CancelCapture, now);
        assert_eq!(calls(&mut e), vec![Call::StartCapture, Call::StopCapture]);
        assert_eq!(e.snapshot().capturing, None);
        // A result arriving after the cancellation is ignored.
        e.event(captured(Outcome::Chosen(ctrl_alt(vk::KEY_M))), now);
        assert!(calls(&mut e).is_empty());
        assert_eq!(e.snapshot().macros[0].hotkey, Hotkey::F8);
    }

    #[test]
    fn a_hotkey_taken_at_startup_makes_the_macro_unavailable_until_another_is_chosen() {
        let mut e = Engine::new(Fake { taken_elsewhere: vec![Hotkey::F8], ..Fake::default() }, &[setup(HOLD_CLICK)]);
        e.start();
        let reason = "F8 est déjà pris par Windows ou par un autre programme : choisis un autre raccourci.";
        assert_eq!(status(&e), MacroStatus::Unavailable(reason.into()));
        assert_eq!(message(&e).level, MessageLevel::Warning);
        let now = Instant::now();
        e.command(Command::Start(ID), now);
        e.tick(now + HUD_START_DELAY);
        assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::Press(_))));
        assert_eq!(status(&e), MacroStatus::Unavailable(reason.into()));
        // F8 is still taken: refused, without "le raccourci reste" since F8 does not work.
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Chosen(Hotkey::F8)), now);
        assert_eq!(message(&e).text, "F8 est déjà pris par Windows ou un autre programme.");
        // Another key: the macro becomes available.
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Chosen(Hotkey::new(Modifiers::NONE, vk::F9))), now);
        assert_eq!(status(&e), MacroStatus::Stopped);
        press_hotkey(&mut e, now);
        assert_eq!(status(&e), MacroStatus::Active);
    }

    #[test]
    fn the_same_hotkey_can_be_chosen_again_once_it_is_free() {
        let mut e = Engine::new(Fake { taken_elsewhere: vec![Hotkey::F8], ..Fake::default() }, &[setup(HOLD_CLICK)]);
        e.start();
        e.backend_mut().taken_elsewhere.clear();
        let now = Instant::now();
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Chosen(Hotkey::F8)), now);
        assert_eq!(status(&e), MacroStatus::Stopped);
        assert_eq!(message(&e).text, "Nouveau raccourci de « Maintenir le clic » : F8.");
    }

    #[test]
    fn shutdown_releases_and_unregisters_everything() {
        let mut e = engine();
        let now = Instant::now();
        let id = hotkey_id(&e, 0);
        press_hotkey(&mut e, now);
        e.command(Command::StartCapture(ID), now);
        calls(&mut e);
        e.shutdown(now);
        assert_eq!(
            calls(&mut e),
            vec![
                Call::StopCapture,
                Call::Release(Left),
                Call::Watch(ButtonSet::EMPTY),
                Call::Watch(ButtonSet::EMPTY),
                Call::Unregister(id),
            ]
        );
        assert!(e.backend().down.is_empty());
        assert_eq!(status(&e), MacroStatus::Stopped);
    }

    #[test]
    fn a_macro_to_come_does_nothing() {
        let mut later = HOLD_CLICK;
        later.id = MacroId("a-venir");
        later.available = false;
        later.default_hotkey = Hotkey::new(Modifiers::NONE, vk::F9);
        let mut e = Engine::new(Fake::default(), &[setup(HOLD_CLICK), setup(later)]);
        e.start();
        assert_eq!(e.backend().calls, vec![Call::Register(Hotkey::F8)]);
        calls(&mut e);
        let now = Instant::now();
        for command in [
            Command::Start(later.id),
            Command::StartCapture(later.id),
            Command::SetOptions(later.id, MacroOptions::HoldClick { button: Right }),
        ] {
            e.command(command, now);
        }
        e.tick(now + HUD_START_DELAY);
        assert!(calls(&mut e).is_empty());
        assert_eq!(e.snapshot().macros[1].status, MacroStatus::Stopped);
        assert_eq!(e.snapshot().capturing, None);
    }

    #[test]
    fn every_message_of_a_macro_says_so_and_only_refusals_are_refused() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::Start(ID), now);
        assert_eq!(message(&e).about, Some(ID), "countdown");
        assert_eq!(message(&e).kind, MessageKind::General);
        e.command(Command::StartCapture(ID), now);
        e.event(captured(Outcome::Chosen(Hotkey::new(Modifiers::NONE, vk::F9))), now);
        let saved = message(&e);
        assert_eq!((saved.level, saved.about, saved.kind), (MessageLevel::Info, Some(ID), MessageKind::General));
    }

    #[test]
    fn messages_are_numbered() {
        let mut e = engine_with(Fake { pointer_on_hud: true, ..Fake::default() });
        let now = Instant::now();
        press_hotkey(&mut e, now);
        let first = e.snapshot().notice.unwrap().number;
        press_hotkey(&mut e, now);
        let second = e.snapshot().notice.unwrap().number;
        assert_eq!(second, first + 1);
    }

    // --- « Multi-clic » ----------------------------------------------------------------------------

    const MULTI: MacroId = MacroId::MULTI_CLICK;
    const MS: Duration = Duration::from_millis(1);

    fn engine_both_with(fake: Fake) -> Engine<Fake> {
        let mut engine = Engine::new(fake, &[setup(HOLD_CLICK), setup(MULTI_CLICK)]);
        engine.start();
        engine.backend_mut().calls.clear();
        engine
    }

    fn engine_both() -> Engine<Fake> {
        engine_both_with(Fake::default())
    }

    fn press_f9(engine: &mut Engine<Fake>, now: Instant) {
        let id = hotkey_id(engine, 1);
        engine.event(Event::Hotkey(id), now);
    }

    fn multi(button: MouseButton, rate: u32) -> MacroOptions {
        MacroOptions::MultiClick { button, rate: ClickRate::new(rate) }
    }

    fn multi_view(engine: &Engine<Fake>) -> MacroSnapshot {
        engine.snapshot().macros[1].clone()
    }

    /// Wakes the engine at each of its deadlines, `late(n)` after it (a busy computer), until `end`,
    /// as its thread does. Returns when each press went, and every call made.
    fn run_until(
        engine: &mut Engine<Fake>,
        end: Instant,
        late: impl Fn(usize) -> Duration,
    ) -> (Vec<Instant>, Vec<Call>) {
        let (mut presses, mut made) = (Vec::new(), Vec::new());
        let mut wakes = 0;
        while let Some(at) = engine.next_deadline() {
            let now = at + late(wakes);
            wakes += 1;
            if now > end {
                break;
            }
            engine.tick(now);
            for call in calls(engine) {
                if matches!(call, Call::ClickDown(_)) {
                    presses.push(now);
                }
                made.push(call);
            }
        }
        (presses, made)
    }

    #[test]
    fn ten_clicks_a_second_for_ten_seconds_make_a_hundred_on_the_beat() {
        let mut e = engine_both();
        let t0 = Instant::now();
        press_f9(&mut e, t0);
        // The guard first, then the first click at once.
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::ClickDown(Left)]);
        assert_eq!(multi_view(&e).status, MacroStatus::Active);
        let (presses, made) = run_until(&mut e, t0 + Duration::from_secs(10) - MS, |_| Duration::ZERO);
        assert_eq!(presses.len() + 1, 100);
        for (k, at) in presses.iter().enumerate() {
            assert_eq!(*at, t0 + 100 * MS * (k as u32 + 1), "press {}", k + 2);
        }
        // Press, release, press, release...: never two presses without a release in between.
        let expected: Vec<Call> =
            (0..100).flat_map(|_| [Call::ClickUp(Left), Call::ClickDown(Left)]).take(199).collect();
        assert_eq!(made, expected);
        assert_eq!(e.guarded_buttons(), ButtonSet::only(Left), "the guard watches the button between clicks too");
    }

    #[test]
    fn late_wake_ups_do_not_shift_the_beat() {
        let mut e = engine_both();
        let t0 = Instant::now();
        press_f9(&mut e, t0);
        calls(&mut e);
        // Each wake-up comes 0 to 15 ms late, as with Windows' ordinary clock.
        let (presses, _) = run_until(&mut e, t0 + Duration::from_secs(10) - MS, |n| (n % 16) as u32 * MS);
        assert_eq!(presses.len() + 1, 100);
        for (k, at) in presses.iter().enumerate() {
            let due = t0 + 100 * MS * (k as u32 + 1);
            assert!(*at >= due && *at <= due + 15 * MS, "press {} at +{:?}", k + 2, *at - t0);
        }
    }

    #[test]
    fn after_a_long_delay_the_beat_starts_again_without_a_burst() {
        let mut e = engine_both();
        let t0 = Instant::now();
        press_f9(&mut e, t0);
        calls(&mut e);
        // The computer was busy for a whole second: ten presses were due.
        let late = t0 + Duration::from_secs(1);
        e.tick(late);
        assert_eq!(calls(&mut e), vec![Call::ClickUp(Left), Call::ClickDown(Left)], "one click, not ten");
        assert_eq!(e.next_deadline(), Some(late + 40 * MS));
        e.tick(late + 40 * MS);
        assert_eq!(e.next_deadline(), Some(late + 100 * MS));
    }

    #[test]
    fn a_click_keeps_its_button_down_for_a_frame_or_two() {
        assert_eq!(press_time(ClickRate::MAX), Duration::from_secs(1) / 30);
        assert_eq!(press_time(ClickRate::DEFAULT), 40 * MS);
        assert_eq!(press_time(ClickRate::MIN), LONGEST_PRESS);
        for rate in 1..=15 {
            let rate = ClickRate::new(rate);
            let press = press_time(rate);
            // Released long before the next press, and down for at least a frame at 30 images/s.
            assert!(press * 2 <= rate.period(), "{rate:?}");
            assert!(press >= Duration::from_secs(1) / 30, "{rate:?}");
        }
    }

    #[test]
    fn a_new_speed_counts_from_the_next_click() {
        let mut e = engine_both();
        let t0 = Instant::now();
        press_f9(&mut e, t0);
        run_until(&mut e, t0 + 150 * MS, |_| Duration::ZERO);
        // Last press at +100 ms; at 2 clicks/s the next one comes 500 ms after it.
        e.command(Command::SetOptions(MULTI, multi(Left, 2)), t0 + 150 * MS);
        assert_eq!(multi_view(&e).options, multi(Left, 2));
        assert_eq!(e.next_deadline(), Some(t0 + 600 * MS));
        assert!(calls(&mut e).is_empty(), "the clicks go on: nothing released, nothing stopped");
        // At 15 clicks/s, the next press is already due: it goes at once.
        e.command(Command::SetOptions(MULTI, multi(Left, 15)), t0 + 200 * MS);
        assert_eq!(e.next_deadline(), Some(t0 + 200 * MS));
        e.tick(t0 + 200 * MS);
        assert_eq!(calls(&mut e), vec![Call::ClickDown(Left)]);
        assert_eq!(e.next_deadline(), Some(t0 + 200 * MS + press_time(ClickRate::MAX)));
        assert_eq!(multi_view(&e).status, MacroStatus::Active);
    }

    #[test]
    fn its_hotkey_stops_it_and_releases_a_press_under_way() {
        let mut e = engine_both();
        let t0 = Instant::now();
        press_f9(&mut e, t0);
        calls(&mut e);
        // In the middle of a press.
        press_f9(&mut e, t0 + 10 * MS);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(e.next_deadline(), None);
        assert_eq!(e.guarded_buttons(), ButtonSet::EMPTY);
        e.tick(t0 + Duration::from_secs(1));
        assert!(calls(&mut e).is_empty(), "no click after the stop");
        // Between two clicks: nothing to release.
        press_f9(&mut e, t0 + Duration::from_secs(2));
        e.tick(t0 + Duration::from_secs(2) + 50 * MS);
        calls(&mut e);
        press_f9(&mut e, t0 + Duration::from_secs(2) + 60 * MS);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::EMPTY)]);
        assert!(e.backend().down.is_empty());
    }

    #[test]
    fn a_real_click_of_its_button_stops_it_other_buttons_pass() {
        let mut e = engine_both();
        let now = Instant::now();
        press_f9(&mut e, now);
        calls(&mut e);
        e.event(click(Right), now);
        assert!(calls(&mut e).is_empty());
        assert_eq!(multi_view(&e).status, MacroStatus::Active);
        e.event(click(Left), now + 5 * MS);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(message(&e).text, "« Multi-clic » arrêtée par ton clic.");
        assert_eq!(message(&e).about, Some(MULTI));
    }

    #[test]
    fn a_click_on_the_hud_stops_it_first() {
        let mut e = engine_both();
        let now = Instant::now();
        press_f9(&mut e, now);
        e.tick(now + 40 * MS);
        calls(&mut e);
        e.event(Event::Click { buttons: ButtonSet::only(Left), on_hud: true }, now + 50 * MS);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(
            message(&e).text,
            "« Multi-clic » arrêtée par ton clic sur le HUD : clique à nouveau pour agir sur le HUD."
        );
    }

    #[test]
    fn no_click_lands_on_the_hud_clicks_wait_then_go_on() {
        let mut e = engine_both();
        let t0 = Instant::now();
        press_f9(&mut e, t0);
        calls(&mut e);
        e.backend_mut().pointer_on_hud = true;
        let (presses, made) = run_until(&mut e, t0 + Duration::from_secs(1), |_| Duration::ZERO);
        assert!(presses.is_empty(), "no press while the mouse is on the HUD");
        assert_eq!(made, vec![Call::ClickUp(Left)], "the press under way still ends");
        let view = multi_view(&e);
        assert_eq!((view.status, view.suspended), (MacroStatus::Active, true));
        assert!(e.next_deadline().unwrap() <= t0 + Duration::from_secs(1) + HUD_CHECK, "it keeps looking");
        // The mouse leaves the HUD: the clicks go on at once.
        e.backend_mut().pointer_on_hud = false;
        let at = e.next_deadline().unwrap();
        e.tick(at);
        assert_eq!(calls(&mut e), vec![Call::ClickDown(Left)]);
        assert!(!multi_view(&e).suspended);
        assert_eq!(e.next_deadline(), Some(at + 40 * MS));
    }

    #[test]
    fn its_hotkey_with_the_mouse_on_the_hud_is_refused_and_changes_nothing() {
        let mut e = engine_both();
        let now = Instant::now();
        press_hotkey(&mut e, now);
        calls(&mut e);
        e.backend_mut().pointer_on_hud = true;
        press_f9(&mut e, now);
        assert!(calls(&mut e).is_empty());
        assert_eq!(message(&e).text, "Sors la souris du HUD avant d'appuyer sur F9.");
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Active, "the hold goes on");
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
    }

    #[test]
    fn started_from_the_hud_it_counts_down_then_clicks() {
        let mut e = engine_both();
        let now = Instant::now();
        e.command(Command::Start(MULTI), now);
        assert!(calls(&mut e).is_empty());
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(message(&e).text, "Activation dans 3 s : place la souris là où il faut cliquer.");
        assert_eq!(message(&e).about, Some(MULTI));
        e.tick(now + Duration::from_secs(1));
        assert_eq!(message(&e).text, "Activation dans 2 s : place la souris là où il faut cliquer.");
        e.tick(now + HUD_START_DELAY);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::ClickDown(Left)]);
        assert_eq!(multi_view(&e).status, MacroStatus::Active);
        assert_eq!(notice_message(&e), Some(None), "the countdown goes at the first click");
        // Still on the HUD when the countdown ends: nothing, as for the hold.
        let later = now + Duration::from_secs(10);
        press_f9(&mut e, later);
        calls(&mut e);
        e.command(Command::Start(MULTI), later);
        e.backend_mut().pointer_on_hud = true;
        e.tick(later + HUD_START_DELAY);
        assert!(calls(&mut e).is_empty());
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(
            message(&e).text,
            "La souris était sur le HUD : aucun clic n'est parti. Recommence en la plaçant ailleurs."
        );
    }

    #[test]
    fn one_macro_at_a_time_between_the_hold_and_the_clicks() {
        let mut e = engine_both();
        let now = Instant::now();
        press_f9(&mut e, now);
        calls(&mut e);
        // F8 in the middle of a click: the click ends, then the hold presses.
        press_hotkey(&mut e, now + 10 * MS);
        assert_eq!(
            calls(&mut e),
            vec![
                Call::Release(Left),
                Call::Watch(ButtonSet::EMPTY),
                Call::Watch(ButtonSet::only(Left)),
                Call::Press(Left)
            ]
        );
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Active);
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(e.next_deadline(), None, "no click left to come");
        // F9 during the hold: the held button goes up, then the clicks start.
        press_f9(&mut e, now + Duration::from_secs(1));
        assert_eq!(
            calls(&mut e),
            vec![
                Call::Release(Left),
                Call::Watch(ButtonSet::EMPTY),
                Call::Watch(ButtonSet::only(Left)),
                Call::ClickDown(Left)
            ]
        );
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Stopped);
        assert_eq!(multi_view(&e).status, MacroStatus::Active);
    }

    #[test]
    fn a_new_button_while_clicking_stops_it_options_of_another_kind_change_nothing() {
        let mut e = engine_both();
        let now = Instant::now();
        press_f9(&mut e, now);
        calls(&mut e);
        e.command(Command::SetOptions(MULTI, MacroOptions::HoldClick { button: Right }), now);
        assert!(calls(&mut e).is_empty());
        assert_eq!(multi_view(&e).options, multi(Left, 10));
        e.command(Command::SetOptions(MULTI, multi(Right, 10)), now + 10 * MS);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(multi_view(&e).options, multi(Right, 10));
        assert_eq!(message(&e).text, "Bouton changé : « Multi-clic » est arrêtée.");
        press_f9(&mut e, now + Duration::from_secs(1));
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Right)), Call::ClickDown(Right)]);
    }

    #[test]
    fn shutdown_stops_the_clicks_and_releases_a_press_under_way() {
        let mut e = engine_both();
        let now = Instant::now();
        let ids = (hotkey_id(&e, 0), hotkey_id(&e, 1));
        press_f9(&mut e, now);
        calls(&mut e);
        e.shutdown(now + 10 * MS);
        assert_eq!(
            calls(&mut e),
            vec![
                Call::Release(Left),
                Call::Watch(ButtonSet::EMPTY),
                Call::Watch(ButtonSet::EMPTY),
                Call::Unregister(ids.0),
                Call::Unregister(ids.1)
            ]
        );
        assert!(e.backend().down.is_empty());
        assert_eq!(e.next_deadline(), None);
    }

    #[test]
    fn refused_clicks_stop_the_macro_and_drop_the_guard() {
        let mut e = engine_both_with(Fake { fail_click_down: true, ..Fake::default() });
        let now = Instant::now();
        press_f9(&mut e, now);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::only(Left)), Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(message(&e).level, MessageLevel::Error);
        assert!(message(&e).text.starts_with("Windows a refusé de cliquer le bouton gauche"), "{}", message(&e).text);
        // A refused release: no more clicks, and the user is told how to free the button.
        let mut e = engine_both();
        press_f9(&mut e, now);
        calls(&mut e);
        e.backend_mut().fail_click_up = true;
        e.tick(now + 40 * MS);
        assert_eq!(calls(&mut e), vec![Call::Watch(ButtonSet::EMPTY)]);
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert!(message(&e).text.ends_with("clique-le une fois pour le libérer."), "{}", message(&e).text);
        assert_eq!(e.next_deadline(), None);
    }

    #[test]
    fn nothing_is_clicked_without_the_guard() {
        let mut e = engine_both_with(Fake { fail_watch: true, ..Fake::default() });
        press_f9(&mut e, Instant::now());
        assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::ClickDown(_))));
        assert_eq!(multi_view(&e).status, MacroStatus::Stopped);
        assert_eq!(message(&e).text, "Impossible de surveiller la souris (crochet refusé) : aucun clic n'est parti.");
    }
}
