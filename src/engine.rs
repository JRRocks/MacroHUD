//! The macro engine: what each macro does when its hotkey is pressed, when the HUD asks, when the
//! user clicks for real, when MacroHUD quits.
//!
//! It knows nothing of Windows: everything it does to the system goes through [`Backend`], so
//! `cargo test` checks it on Linux with a fake backend. In the application it runs on its own
//! thread (see `platform`), which owns the global hotkeys and injects the mouse buttons.
//!
//! Rules it keeps, whatever happens:
//! - no button is pressed unless the physical-click guard already watches it;
//! - the button released is always the one actually pressed, even if the options changed since;
//! - stopping, changing the options, quitting: whatever is held is released first.

use std::time::{Duration, Instant};

use crate::capture::Outcome;
use crate::contract::{MacroId, MacroOptions, MacroStatus, Message, MessageLevel, MouseButton};
use crate::guard::ButtonSet;
use crate::hotkey::{self, Hotkey, Vk};
use crate::macros::MacroDef;

/// Delay between « Activer » in the HUD and the press: the time to move the mouse away from the
/// HUD, to where the button must act (a press at once would land on the HUD itself).
pub const HUD_START_DELAY: Duration = Duration::from_secs(3);
/// A capture that gets no key for this long is cancelled, so the keyboard is not listened to forever.
pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

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
    /// From now on the physical-click guard watches these buttons (the ones macros hold).
    fn watch(&mut self, held: ButtonSet) -> Result<(), String>;
    /// Whether the button is already down (held by the user's hand, for instance).
    fn is_down(&self, button: MouseButton) -> bool;
    /// Presses the button (injected input), remembered for the emergency release.
    fn press(&mut self, button: MouseButton) -> Result<(), String>;
    /// Takes over a button that is already down: nothing injected, but it will be released like a
    /// pressed one (and by the emergency release).
    fn adopt(&mut self, button: MouseButton);
    /// Releases the button (injected input).
    fn release(&mut self, button: MouseButton) -> Result<(), String>;
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
    /// A click the guard swallowed: on a held button, made by hand (`on_hud` false), or anywhere on
    /// the HUD while buttons were held (`on_hud` true). The macros holding `buttons` must stop.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Run {
    Stopped,
    /// Started from the HUD: the button is pressed at `at`; until then the macro shows as stopped,
    /// and a message counts down the seconds (`shown`: the number it shows).
    Arming {
        button: MouseButton,
        at: Instant,
        shown: u64,
    },
    /// The button is down, held by the macro.
    Holding {
        button: MouseButton,
    },
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

/// The countdown shown between « Activer » and the press.
fn countdown_text(seconds: u64) -> String {
    format!("Activation dans {seconds} s : place la souris là où le clic doit rester enfoncé.")
}

/// Whole seconds left until `at`, rounded up (2.3 s left: « 3 s »).
fn seconds_left(at: Instant, now: Instant) -> u64 {
    let millis = at.saturating_duration_since(now).as_millis();
    u64::try_from(millis.div_ceil(1000)).unwrap_or(u64::MAX)
}

/// « le bouton gauche » -> « Le bouton gauche ».
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
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
                self.say(MessageLevel::Warning, format!("« {name} » est indisponible : {reason}"));
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
                    self.stop_macro(i);
                }
            }
            Command::SetOptions(id, options) => {
                if let Some(i) = self.find(id) {
                    self.set_options(i, options);
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
                    self.say(MessageLevel::Info, format!("Raccourci inchangé : {label}."));
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
                    Run::Arming { .. } | Run::Holding { .. } => self.stop_macro(i),
                }
            }
            Event::Click { buttons, on_hud } => {
                for i in 0..self.slots.len() {
                    if let Run::Holding { button } = self.slots[i].run
                        && buttons.contains(button)
                    {
                        self.stop_macro(i);
                        let name = self.slots[i].def.name;
                        if on_hud {
                            crate::log::write(format!("« {name} » arrêtée par un clic sur le HUD."));
                            self.say(
                                MessageLevel::Info,
                                format!("« {name} » arrêtée par ton clic sur le HUD : clique à nouveau pour agir sur le HUD."),
                            );
                        } else {
                            crate::log::write(format!("« {name} » arrêtée par un clic physique."));
                            self.say(MessageLevel::Info, format!("« {name} » arrêtée par ton clic."));
                        }
                    }
                }
            }
            Event::Captured { outcome, source } => self.captured(outcome, source),
        }
    }

    /// Does what is due at `now`: countdowns and delayed presses, captures that lasted too long.
    pub fn tick(&mut self, now: Instant) {
        for i in 0..self.slots.len() {
            let Run::Arming { button, at, shown } = self.slots[i].run else { continue };
            if now < at {
                let left = seconds_left(at, now);
                if left < shown {
                    self.slots[i].run = Run::Arming { button, at, shown: left };
                    self.say(MessageLevel::Info, countdown_text(left));
                }
                continue;
            }
            self.slots[i].run = Run::Stopped;
            if self.backend.pointer_on_hud() {
                self.say(
                    MessageLevel::Warning,
                    "La souris était sur le HUD : rien n'a été enfoncé. Recommence en la plaçant ailleurs.",
                );
            } else {
                // The countdown is over: its message goes, the button goes down.
                self.withdraw();
                self.press_now(i, button);
            }
        }
        if let Some(capture) = &self.capture
            && now >= capture.deadline
        {
            let index = capture.index;
            self.capture = None;
            self.backend.stop_capture();
            let label = self.label(self.slots[index].hotkey);
            self.say(MessageLevel::Info, format!("Aucune touche reçue : le raccourci reste {label}."));
        }
    }

    /// When [`Engine::tick`] has something to do next.
    pub fn next_deadline(&self) -> Option<Instant> {
        let arming = self.slots.iter().filter_map(|s| match s.run {
            // The next second of the countdown, or the press itself.
            Run::Arming { at, shown, .. } if shown > 1 => {
                Some(at.checked_sub(Duration::from_secs(shown - 1)).unwrap_or(at))
            }
            Run::Arming { at, .. } => Some(at),
            _ => None,
        });
        arming.chain(self.capture.as_ref().map(|c| c.deadline)).min()
    }

    /// Before quitting: releases everything held, stops listening, unregisters the hotkeys.
    pub fn shutdown(&mut self) {
        if self.capture.take().is_some() {
            self.backend.stop_capture();
        }
        for i in 0..self.slots.len() {
            self.stop_macro(i);
        }
        let _ = self.backend.watch(ButtonSet::EMPTY);
        for i in 0..self.slots.len() {
            if let Some(id) = self.slots[i].registered.take() {
                self.backend.unregister_hotkey(id);
            }
        }
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
                        // Nothing is pressed during the countdown: the macro still shows as stopped.
                        (None, Run::Stopped | Run::Arming { .. }) => MacroStatus::Stopped,
                        (None, Run::Holding { .. }) => MacroStatus::Active,
                    },
                    hotkey: s.hotkey,
                    hotkey_label: self.label(s.hotkey),
                    options: s.options,
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

    fn say(&mut self, level: MessageLevel, text: impl Into<String>) {
        let text = text.into();
        crate::log::write(format!("Message ({level:?}) : {text}"));
        let number = self.next_notice_number();
        self.notice = Some(Notice { number, message: Some(Message::new(level, text)) });
    }

    /// Takes the engine's current message away (the countdown it showed is over).
    fn withdraw(&mut self) {
        if self.notice.as_ref().is_some_and(|n| n.message.is_some()) {
            let number = self.next_notice_number();
            self.notice = Some(Notice { number, message: None });
        }
    }

    /// Buttons held by macros right now.
    fn held_buttons(&self) -> ButtonSet {
        self.slots.iter().fold(ButtonSet::EMPTY, |set, s| match s.run {
            Run::Holding { button } => set.with(button),
            _ => set,
        })
    }

    /// The other macro holding (or about to hold) `button`, if any.
    fn other_user_of(&self, button: MouseButton, except: usize) -> Option<&'static str> {
        self.slots.iter().enumerate().find_map(|(i, s)| match s.run {
            Run::Holding { button: b } | Run::Arming { button: b, .. } if i != except && b == button => {
                Some(s.def.name)
            }
            _ => None,
        })
    }

    fn start_macro(&mut self, i: usize, trigger: Trigger, now: Instant) {
        let slot = &self.slots[i];
        if let Some(reason) = &slot.unavailable {
            let text = format!("« {} » est indisponible : {reason}", slot.def.name);
            self.say(MessageLevel::Warning, text);
            return;
        }
        if slot.run != Run::Stopped {
            return;
        }
        let MacroOptions::HoldClick { button } = slot.options;
        if let Some(other) = self.other_user_of(button, i) {
            let text = format!("{} est déjà maintenu par « {other} ».", capitalized(button.phrase()));
            self.say(MessageLevel::Warning, text);
            return;
        }
        match trigger {
            Trigger::Hud => {
                let seconds = HUD_START_DELAY.as_secs();
                self.slots[i].run = Run::Arming { button, at: now + HUD_START_DELAY, shown: seconds };
                self.say(MessageLevel::Info, countdown_text(seconds));
            }
            Trigger::Hotkey => {
                if self.backend.pointer_on_hud() {
                    let label = self.label(self.slots[i].hotkey);
                    self.say(MessageLevel::Warning, format!("Sors la souris du HUD avant d'appuyer sur {label}."));
                    return;
                }
                self.press_now(i, button);
            }
        }
    }

    fn press_now(&mut self, i: usize, button: MouseButton) {
        self.slots[i].run = Run::Stopped;
        if let Some(other) = self.other_user_of(button, i) {
            let text = format!("{} est déjà maintenu par « {other} ».", capitalized(button.phrase()));
            self.say(MessageLevel::Warning, text);
            return;
        }
        // The guard first: no button is ever down without a way for the user to take it back.
        if let Err(e) = self.backend.watch(self.held_buttons().with(button)) {
            self.say(MessageLevel::Error, format!("Impossible de surveiller la souris ({e}) : rien n'a été enfoncé."));
            let _ = self.backend.watch(self.held_buttons());
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
                let _ = self.backend.watch(self.held_buttons());
                self.say(MessageLevel::Error, format!("Windows a refusé d'enfoncer {} ({e}).", button.phrase()));
            }
        }
    }

    fn stop_macro(&mut self, i: usize) {
        match self.slots[i].run {
            Run::Stopped => {}
            Run::Arming { .. } => {
                // Nothing was pressed yet: the countdown just stops, and its message goes.
                self.slots[i].run = Run::Stopped;
                self.withdraw();
            }
            Run::Holding { button } => {
                // Release first: nothing may delay it.
                let released = self.backend.release(button);
                self.slots[i].run = Run::Stopped;
                let _ = self.backend.watch(self.held_buttons());
                match released {
                    Ok(()) => {
                        crate::log::write(format!("« {} » : {} relâché.", self.slots[i].def.name, button.phrase()))
                    }
                    Err(e) => self.say(
                        MessageLevel::Error,
                        format!(
                            "Windows a refusé de relâcher {} ({e}) : clique-le une fois pour le libérer.",
                            button.phrase()
                        ),
                    ),
                }
            }
        }
    }

    fn set_options(&mut self, i: usize, options: MacroOptions) {
        if self.slots[i].options == options {
            return;
        }
        match self.slots[i].run {
            Run::Stopped => self.slots[i].options = options,
            Run::Arming { at, shown, .. } => {
                // Nothing is pressed yet: the countdown goes on with the new button, if it is free.
                self.slots[i].options = options;
                let MacroOptions::HoldClick { button } = options;
                if let Some(other) = self.other_user_of(button, i) {
                    self.slots[i].run = Run::Stopped;
                    let text = format!("{} est déjà maintenu par « {other} ».", capitalized(button.phrase()));
                    self.say(MessageLevel::Warning, text);
                } else {
                    self.slots[i].run = Run::Arming { button, at, shown };
                }
            }
            Run::Holding { .. } => {
                // The held button is released before anything changes.
                self.stop_macro(i);
                self.slots[i].options = options;
                let name = self.slots[i].def.name;
                self.say(MessageLevel::Info, format!("Bouton changé : « {name} » est arrêtée."));
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
                self.say(MessageLevel::Info, format!("Raccourci inchangé : {label}."));
            }
            Outcome::Chosen(hotkey) => self.change_hotkey(capture.index, hotkey),
        }
    }

    /// Replaces the hotkey of macro `i`. A refused hotkey changes nothing: the old one stays active.
    fn change_hotkey(&mut self, i: usize, hotkey: Hotkey) {
        let label = self.label(hotkey);
        let current = self.label(self.slots[i].hotkey);
        let active = self.slots[i].registered.is_some();
        let keep = if active { format!(" Le raccourci reste {current}.") } else { String::new() };

        if hotkey == self.slots[i].hotkey && active {
            self.say(MessageLevel::Info, format!("Raccourci inchangé : {label}."));
            return;
        }
        let owner = self
            .slots
            .iter()
            .enumerate()
            .find(|(j, s)| *j != i && s.def.available && s.hotkey == hotkey)
            .map(|(_, s)| s.def.name);
        if let Some(other) = owner {
            self.say(MessageLevel::Error, format!("{label} est déjà le raccourci de « {other} ».{keep}"));
            return;
        }
        if let Some(why) = hotkey::refusal(&hotkey, &label) {
            self.say(MessageLevel::Error, format!("{why}{keep}"));
            return;
        }
        if let Some(c) = self.backend.altgr_char(hotkey) {
            self.say(
                MessageLevel::Error,
                format!("{label} sert à écrire « {c} » (AltGr) sur ce clavier : choisis-en un autre.{keep}"),
            );
            return;
        }
        match self.backend.register_hotkey(hotkey) {
            Err(RegisterError::Taken) => {
                self.say(
                    MessageLevel::Error,
                    format!("{label} est déjà pris par Windows ou par un autre programme.{keep}"),
                );
            }
            Err(RegisterError::Other(e)) => {
                self.say(MessageLevel::Error, format!("Windows refuse le raccourci {label} ({e}).{keep}"));
            }
            Ok(id) => {
                if let Some(old) = self.slots[i].registered.replace(id) {
                    self.backend.unregister_hotkey(old);
                }
                self.slots[i].hotkey = hotkey;
                self.slots[i].unavailable = None;
                let name = self.slots[i].def.name;
                self.say(MessageLevel::Info, format!("Nouveau raccourci de « {name} » : {label}."));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::{Modifier, Modifiers, vk};
    use crate::macros::HOLD_CLICK;
    use MouseButton::*;

    #[derive(Clone, Debug, PartialEq)]
    enum Call {
        Watch(ButtonSet),
        Press(MouseButton),
        Adopt(MouseButton),
        Release(MouseButton),
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
    fn a_click_on_the_hud_during_a_hold_stops_every_hold() {
        let mut second = HOLD_CLICK;
        second.id = MacroId("deuxieme");
        second.name = "Deuxième";
        second.default_hotkey = Hotkey::new(Modifiers::NONE, vk::F9);
        second.default_options = MacroOptions::HoldClick { button: Middle };
        let mut e = Engine::new(Fake::default(), &[setup(HOLD_CLICK), setup(second)]);
        e.start();
        let now = Instant::now();
        e.command(Command::SetOptions(ID, MacroOptions::HoldClick { button: Right }), now);
        press_hotkey(&mut e, now);
        let id2 = hotkey_id(&e, 1);
        e.event(Event::Hotkey(id2), now);
        calls(&mut e);
        // A left click on the HUD (it cannot reach the HUD while the other window holds the mouse).
        let all = ButtonSet::only(Right).with(Middle);
        e.event(Event::Click { buttons: all, on_hud: true }, now);
        let made = calls(&mut e);
        assert!(made.contains(&Call::Release(Right)) && made.contains(&Call::Release(Middle)), "{made:?}");
        assert_eq!(made.last(), Some(&Call::Watch(ButtonSet::EMPTY)));
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Stopped);
        assert_eq!(e.snapshot().macros[1].status, MacroStatus::Stopped);
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
        let m = message(&e);
        assert_eq!(m.level, MessageLevel::Error);
        assert_eq!(m.text, "Ctrl+Alt+M est déjà pris par Windows ou par un autre programme. Le raccourci reste F8.");
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
        assert_eq!(message(&e).text, "F9 est déjà le raccourci de « Deuxième ». Le raccourci reste F8.");
        // Each macro toggles with its own hotkey; both can run at once on different buttons.
        press_hotkey(&mut e, now);
        let id2 = hotkey_id(&e, 1);
        e.event(Event::Hotkey(id2), now);
        assert_eq!(
            calls(&mut e),
            vec![
                Call::Watch(ButtonSet::only(Left)),
                Call::Press(Left),
                Call::Watch(ButtonSet::only(Left).with(Right)),
                Call::Press(Right),
            ]
        );
        e.event(click(Left), now);
        assert_eq!(calls(&mut e), vec![Call::Release(Left), Call::Watch(ButtonSet::only(Right))]);
        assert_eq!(e.snapshot().macros[1].status, MacroStatus::Active);
    }

    #[test]
    fn two_macros_cannot_hold_the_same_button() {
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
        assert!(calls(&mut e).is_empty());
        assert_eq!(message(&e).text, "Le bouton gauche est déjà maintenu par « Maintenir le clic ».");
        assert_eq!(e.snapshot().macros[1].status, MacroStatus::Stopped);
    }

    #[test]
    fn hotkeys_breaking_typing_or_windows_are_refused() {
        let mut e = engine();
        let now = Instant::now();
        for refused in [
            Hotkey::new(Modifiers::NONE, vk::KEY_M),
            Hotkey::new(Modifiers::NONE.with(Modifier::Alt), vk::F4),
            Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl), vk::KEY_C),
        ] {
            e.command(Command::StartCapture(ID), now);
            e.event(captured(Outcome::Chosen(refused)), now);
            assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::Register(_))));
            let m = message(&e);
            assert_eq!(m.level, MessageLevel::Error);
            assert!(m.text.ends_with("Le raccourci reste F8."), "{}", m.text);
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
        assert_eq!(message(&e).text, "F8 est déjà pris par Windows ou par un autre programme.");
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
        e.shutdown();
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
    fn messages_are_numbered() {
        let mut e = engine_with(Fake { pointer_on_hud: true, ..Fake::default() });
        let now = Instant::now();
        press_hotkey(&mut e, now);
        let first = e.snapshot().notice.unwrap().number;
        press_hotkey(&mut e, now);
        let second = e.snapshot().notice.unwrap().number;
        assert_eq!(second, first + 1);
    }
}
