//! Stand-ins for systems other than Windows (Linux: tests and development). Nothing touches the real
//! mouse or keyboard: presses are only written to the log, hotkeys are only remembered. The HUD can
//! be run and looked at (`cargo run`), which is all the "visuel" session needs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::BoxKind;
use crate::clock::Stamp;
use crate::contract::MouseButton;
use crate::engine::{
    Backend, Command, Engine, Event, ExternalSetup, HotkeyId, Launch, MacroSetup, RegisterError, Snapshot,
};
use crate::guard::ButtonSet;
use crate::hotkey::{Hotkey, Vk};
use crate::runner::{EngineHandle, Input, Publisher};
use crate::script::{Report, Scripts};

/// Buttons "held" by the stand-in backend.
static HELD: AtomicU8 = AtomicU8::new(0);

/// Releases whatever is held; returns what was.
pub fn emergency_release() -> ButtonSet {
    ButtonSet::from_bits(HELD.swap(0, Ordering::SeqCst))
}

pub fn set_panicking() {}

pub fn install_native_crash_handler() {}

/// Local time is not known here: callers fall back to UTC.
pub fn local_time() -> Option<Stamp> {
    None
}

/// `$XDG_CONFIG_HOME/MacroHUD`, or `~/.config/MacroHUD`.
pub fn settings_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("MacroHUD")
}

/// Consoles need no attaching here.
pub fn attach_console() {}

/// No single-instance check outside Windows.
pub struct SingleInstance;

pub fn single_instance() -> Option<SingleInstance> {
    Some(SingleInstance)
}

/// Prints the message (no message boxes here).
pub fn message_box(title: &str, text: &str, kind: BoxKind) {
    eprintln!("[{title}] ({kind:?}) {text}");
}

/// No Job Object outside Windows: a script is ended with `Child::kill` (itself only, not what it
/// started), which is enough to develop and test.
pub struct Job;

impl Job {
    pub fn for_child(_child: &std::process::Child, _buttons: ButtonSet) -> Result<Job, String> {
        Ok(Job)
    }

    pub fn terminate(&self) {}
}

/// The HUD window: nothing to place outside Windows.
#[derive(Clone, Copy, Debug, Default)]
pub struct HudWindow;

impl HudWindow {
    pub fn from_creation_context(_cc: &eframe::CreationContext<'_>) -> Self {
        HudWindow
    }

    /// Raw handle, for the engine (none here).
    pub fn raw(&self) -> Option<isize> {
        None
    }

    /// Top-left corner in screen pixels (unknown here).
    pub fn position(&self) -> Option<(i32, i32)> {
        None
    }

    pub fn place(&self, _saved: Option<(i32, i32)>) {}

    pub fn keep_on_screen(&self) {}

    /// No keyboard of the HUD's own to listen to here.
    pub fn start_key_capture(&self, _report: Box<dyn Fn(crate::capture::Outcome)>) {}

    pub fn stop_key_capture(&self) {}

    pub fn key_capture_tick(&self) {}
}

/// A backend that pretends: presses go to the log, hotkeys always register (unless used twice). The
/// scripts of external macros run for real.
struct StandIn {
    hotkeys: Vec<(HotkeyId, Hotkey)>,
    next_id: HotkeyId,
    scripts: Scripts,
    report: Report,
}

impl Backend for StandIn {
    fn watch(&mut self, _held: ButtonSet, _clicked: ButtonSet) -> Result<(), String> {
        Ok(())
    }
    fn is_down(&self, button: MouseButton) -> bool {
        ButtonSet::from_bits(HELD.load(Ordering::SeqCst)).contains(button)
    }
    fn press(&mut self, button: MouseButton) -> Result<(), String> {
        HELD.fetch_or(ButtonSet::only(button).bits(), Ordering::SeqCst);
        crate::log::write(format!("(simulé) {} enfoncé", button.phrase()));
        Ok(())
    }
    fn adopt(&mut self, button: MouseButton) {
        HELD.fetch_or(ButtonSet::only(button).bits(), Ordering::SeqCst);
    }
    fn release(&mut self, button: MouseButton) -> Result<(), String> {
        HELD.fetch_and(!ButtonSet::only(button).bits(), Ordering::SeqCst);
        crate::log::write(format!("(simulé) {} relâché", button.phrase()));
        Ok(())
    }
    // The clicks of « Multi-clic »: no line for each, as on Windows.
    fn click_down(&mut self, button: MouseButton) -> Result<(), String> {
        HELD.fetch_or(ButtonSet::only(button).bits(), Ordering::SeqCst);
        Ok(())
    }
    fn click_up(&mut self, button: MouseButton) -> Result<bool, String> {
        HELD.fetch_and(!ButtonSet::only(button).bits(), Ordering::SeqCst);
        Ok(false)
    }
    fn pointer_on_hud(&self) -> bool {
        false
    }
    fn register_hotkey(&mut self, hotkey: Hotkey) -> Result<HotkeyId, RegisterError> {
        if self.hotkeys.iter().any(|(_, h)| *h == hotkey) {
            return Err(RegisterError::Taken);
        }
        self.next_id += 1;
        self.hotkeys.push((self.next_id, hotkey));
        Ok(self.next_id)
    }
    fn unregister_hotkey(&mut self, id: HotkeyId) {
        self.hotkeys.retain(|(i, _)| *i != id);
    }
    fn start_capture(&mut self) -> Result<(), String> {
        // No global keyboard here: the capture ends by Échap... never, so by the HUD or the timeout.
        Ok(())
    }
    fn stop_capture(&mut self) {}
    fn altgr_char(&self, _hotkey: Hotkey) -> Option<char> {
        None
    }
    fn layout_char(&self, _key: Vk) -> Option<char> {
        None
    }
    fn guard_hud(&mut self, _on: bool) -> Result<(), String> {
        Ok(())
    }
    fn launch(&mut self, launch: &Launch) -> Result<u64, String> {
        self.scripts.launch(launch, self.report.clone())
    }
    fn tell(&mut self, run: u64, line: &str) -> Result<(), String> {
        self.scripts.tell(run, line)
    }
    fn script_ended(&mut self, run: u64) -> Option<Option<i32>> {
        self.scripts.ended(run)
    }
    fn kill(&mut self, run: u64) {
        self.scripts.kill(run);
    }
    fn script_done(&mut self, run: u64) {
        self.scripts.done(run);
    }
    fn release_down(&mut self, buttons: ButtonSet) -> ButtonSet {
        // Nothing the scripts press is known here.
        let _ = buttons;
        ButtonSet::EMPTY
    }
}

/// Starts the engine thread.
pub fn spawn_engine(
    setup: Vec<MacroSetup>,
    externals: Vec<ExternalSetup>,
    _hud: Option<isize>,
    repaint: Box<dyn Fn() + Send>,
) -> Result<EngineHandle, String> {
    let (sender, receiver) = mpsc::channel::<Input>();
    let shared = Arc::new(Mutex::new(Snapshot::default()));
    let publisher = Publisher::new(shared.clone(), repaint);
    let for_scripts = sender.clone();
    let thread = std::thread::Builder::new()
        .name("macrohud-moteur".into())
        .spawn(move || engine_main(setup, externals, receiver, for_scripts, publisher))
        .map_err(|e| format!("fil du moteur : {e}"))?;
    Ok(EngineHandle::new(sender, Arc::new(|| {}), shared, thread))
}

fn engine_main(
    setup: Vec<MacroSetup>,
    externals: Vec<ExternalSetup>,
    inputs: Receiver<Input>,
    sender: Sender<Input>,
    mut publisher: Publisher,
) {
    let report: Report = Arc::new(move |run, output| {
        let _ = sender.send(Input::Event(Event::Script { run, output }));
    });
    let backend = StandIn { hotkeys: Vec::new(), next_id: 0, scripts: Scripts::default(), report };
    let mut engine = Engine::new(backend, &setup, externals);
    engine.start();
    publisher.publish(engine.snapshot());
    loop {
        let wait =
            engine.next_deadline().map_or(Duration::from_secs(3600), |at| at.saturating_duration_since(Instant::now()));
        match inputs.recv_timeout(wait) {
            Ok(Input::Command(command)) => {
                if matches!(command, Command::Stop(_)) {
                    crate::crash::crash_test_point(engine.crash_test_buttons());
                }
                engine.command(command, Instant::now());
            }
            Ok(Input::Event(event)) => engine.event(event, Instant::now()),
            Ok(Input::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        engine.tick(Instant::now());
        publisher.publish(engine.snapshot());
    }
    // Quitting: the script of an external macro has a moment to stop, its output still read.
    let mut next = engine.shutdown(Instant::now());
    publisher.publish(engine.snapshot());
    while let Some(at) = next {
        match inputs.recv_timeout(at.saturating_duration_since(Instant::now())) {
            Ok(Input::Event(event)) => engine.event(event, Instant::now()),
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(at.saturating_duration_since(Instant::now())),
        }
        next = engine.shutdown(Instant::now());
        publisher.publish(engine.snapshot());
    }
}
