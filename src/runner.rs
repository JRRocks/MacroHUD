//! The link between the HUD's thread and the engine's thread: commands go one way, snapshots the
//! other. The engine thread itself (its loop) is in `platform`, since on Windows it must also
//! receive the global hotkeys.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::engine::{Command, Event, SCRIPT_STOP_WAIT, Snapshot};

/// What the engine thread receives.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Command(Command),
    Event(Event),
    Shutdown,
}

/// How long quitting waits for the engine to release everything and stop: the script of an external
/// macro has [`SCRIPT_STOP_WAIT`] to quit, then it is stopped by force; one more second for that.
const SHUTDOWN_WAIT: Duration = SCRIPT_STOP_WAIT.saturating_add(Duration::from_secs(1));

/// The app's handle on the engine thread.
pub struct EngineHandle {
    sender: Sender<Input>,
    wake: Arc<dyn Fn() + Send + Sync>,
    shared: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<()>>,
}

impl EngineHandle {
    /// `wake` makes the engine thread look at its inputs (on Windows, a thread message).
    pub fn new(
        sender: Sender<Input>,
        wake: Arc<dyn Fn() + Send + Sync>,
        shared: Arc<Mutex<Snapshot>>,
        thread: JoinHandle<()>,
    ) -> Self {
        EngineHandle { sender, wake, shared, thread: Some(thread) }
    }

    pub fn send(&self, command: Command) {
        if self.sender.send(Input::Command(command)).is_ok() {
            (self.wake)();
        }
    }

    /// Something that reports system events to the engine from another thread (the HUD's own
    /// keyboard, during a capture).
    pub fn reporter(&self) -> impl Fn(Event) + Send + 'static {
        let sender = self.sender.clone();
        let wake = Arc::clone(&self.wake);
        move |event| {
            if sender.send(Input::Event(event)).is_ok() {
                wake();
            }
        }
    }

    /// The latest published snapshot.
    pub fn snapshot(&self) -> Snapshot {
        self.shared.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Asks the engine to release everything and stop, and waits for it (a little).
    pub fn shutdown(&mut self) {
        let Some(thread) = self.thread.take() else { return };
        if self.sender.send(Input::Shutdown).is_ok() {
            (self.wake)();
        }
        let deadline = Instant::now() + SHUTDOWN_WAIT;
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if thread.is_finished() {
            let _ = thread.join();
        } else {
            crate::log::write("Le moteur ne s'est pas arrêté à temps.");
        }
    }
}

/// Publishes the engine's snapshot when it changed, and asks the HUD to redraw.
pub struct Publisher {
    shared: Arc<Mutex<Snapshot>>,
    repaint: Box<dyn Fn() + Send>,
    last: Option<Snapshot>,
}

impl Publisher {
    pub fn new(shared: Arc<Mutex<Snapshot>>, repaint: Box<dyn Fn() + Send>) -> Self {
        Publisher { shared, repaint, last: None }
    }

    pub fn publish(&mut self, snapshot: Snapshot) {
        if self.last.as_ref() == Some(&snapshot) {
            return;
        }
        *self.shared.lock().unwrap_or_else(PoisonError::into_inner) = snapshot.clone();
        self.last = Some(snapshot);
        (self.repaint)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn only_changes_are_published_and_repainted() {
        let shared = Arc::new(Mutex::new(Snapshot::default()));
        let repaints = Arc::new(AtomicUsize::new(0));
        let counter = repaints.clone();
        let mut publisher = Publisher::new(
            shared.clone(),
            Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        );
        publisher.publish(Snapshot::default());
        publisher.publish(Snapshot::default());
        assert_eq!(repaints.load(Ordering::SeqCst), 1);
        let changed = Snapshot { capturing: Some(crate::contract::MacroId::HOLD_CLICK), ..Snapshot::default() };
        publisher.publish(changed.clone());
        assert_eq!(repaints.load(Ordering::SeqCst), 2);
        assert_eq!(*shared.lock().unwrap(), changed);
    }
}
