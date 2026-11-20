//! Stand-ins for systems other than Windows (Linux: tests and development). Nothing touches the real
//! mouse or keyboard: presses are only written to the log, hotkeys are only remembered. The HUD can
//! be run and looked at (`cargo run`), which is all the "visuel" session needs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::BoxKind;
use crate::clock::Stamp;
use crate::contract::MouseButton;
use crate::engine::{Backend, Command, Engine, HotkeyId, MacroSetup, RegisterError, Snapshot};
use crate::guard::ButtonSet;
use crate::hotkey::{Hotkey, Vk};
use crate::runner::{EngineHandle, Input, Publisher};

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

/// A backend that pretends: presses go to the log, hotkeys always register (unless used twice).
struct StandIn {
    hotkeys: Vec<(HotkeyId, Hotkey)>,
    next_id: HotkeyId,
}

impl Backend for StandIn {
    fn watch(&mut self, _held: ButtonSet) -> Result<(), String> {
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
}

/// Starts the engine thread.
pub fn spawn_engine(
    setup: Vec<MacroSetup>,
    _hud: Option<isize>,
    repaint: Box<dyn Fn() + Send>,
) -> Result<EngineHandle, String> {
    let (sender, receiver) = mpsc::channel::<Input>();
    let shared = Arc::new(Mutex::new(Snapshot::default()));
    let publisher = Publisher::new(shared.clone(), repaint);
    let thread = std::thread::Builder::new()
        .name("macrohud-moteur".into())
        .spawn(move || engine_main(setup, receiver, publisher))
        .map_err(|e| format!("fil du moteur : {e}"))?;
    Ok(EngineHandle::new(sender, Arc::new(|| {}), shared, thread))
}

fn engine_main(setup: Vec<MacroSetup>, inputs: Receiver<Input>, mut publisher: Publisher) {
    let mut engine = Engine::new(StandIn { hotkeys: Vec::new(), next_id: 0 }, &setup);
    engine.start();
    publisher.publish(engine.snapshot());
    loop {
        let wait =
            engine.next_deadline().map_or(Duration::from_secs(3600), |at| at.saturating_duration_since(Instant::now()));
        match inputs.recv_timeout(wait) {
            Ok(Input::Command(command)) => {
                if matches!(command, Command::Stop(_)) {
                    crate::crash::crash_test_point(ButtonSet::from_bits(HELD.load(Ordering::SeqCst)));
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
    engine.shutdown();
    publisher.publish(engine.snapshot());
}
