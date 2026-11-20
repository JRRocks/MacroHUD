//! A small run log, `%APPDATA%\MacroHUD\macrohud.log`, to understand afterwards what happened on
//! the PC (hotkeys registered or refused, macros started and stopped, errors). The previous run's log
//! is kept as `macrohud.precedent.log`. Nothing is written until [`init`] is called (tests write nothing).

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use crate::clock::Stamp;

static LOG: Mutex<Option<File>> = Mutex::new(None);

pub const FILE_NAME: &str = "macrohud.log";
const PREVIOUS: &str = "macrohud.precedent.log";

/// Starts the log of this run in `dir`.
pub fn init(dir: &Path) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join(FILE_NAME);
    let _ = std::fs::rename(&path, dir.join(PREVIOUS));
    if let Ok(file) = File::create(&path) {
        *LOG.lock().unwrap_or_else(PoisonError::into_inner) = Some(file);
    }
}

fn line(text: &str) -> String {
    format!("{}  {}\n", Stamp::now().display(), text)
}

/// Writes one line.
pub fn write(text: impl AsRef<str>) {
    let mut guard = LOG.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(file) = guard.as_mut() {
        let _ = file.write_all(line(text.as_ref()).as_bytes());
    }
}

/// Writes one line from the panic hook: never waits for the lock (the panicking thread may hold it).
pub fn write_now(text: impl AsRef<str>) {
    let mut guard = match LOG.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return,
    };
    if let Some(file) = guard.as_mut() {
        let _ = file.write_all(line(text.as_ref()).as_bytes());
        let _ = file.flush();
    }
}
