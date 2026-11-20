//! Remembers whether MacroHUD was closed normally last time.
//!
//! A brutal stop of the process (Task Manager « Fin de tâche » on the process, power cut) cannot be
//! caught: if a button was held then, it stays down until the user clicks it once. A marker file
//! written at start and removed at a normal exit (or after a handled crash) lets the next start say
//! so in the HUD.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static MARKER: OnceLock<PathBuf> = OnceLock::new();

const FILE_NAME: &str = "session-en-cours";

/// Starts the session; returns whether the previous one ended without a normal exit.
pub fn begin(dir: &Path) -> bool {
    let path = dir.join(FILE_NAME);
    let previous_ended_abruptly = path.exists();
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(&path, format!("MacroHUD {} en cours\n", crate::VERSION));
    let _ = MARKER.set(path);
    previous_ended_abruptly
}

/// The session ended normally (or its crash was handled).
pub fn end() {
    if let Some(path) = MARKER.get() {
        let _ = std::fs::remove_file(path);
    }
}

/// The warning shown at start after an abrupt end.
pub const ABRUPT_END_WARNING: &str = "MacroHUD ne s'était pas fermé normalement la dernière fois. \
     Si un bouton de la souris semble bloqué, cliquez-le une fois.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_left_behind_means_an_abrupt_end() {
        let dir = std::env::temp_dir().join(format!("macrohud-tests-{}-session", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!begin(&dir), "first start");
        assert!(dir.join(FILE_NAME).exists());
        // No end(): the next start notices.
        assert!(begin(&dir));
        // `end` removes the marker registered by the first `begin` (same path here).
        end();
        assert!(!dir.join(FILE_NAME).exists());
    }
}
