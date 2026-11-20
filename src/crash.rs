//! Readable crash reports. When MacroHUD panics, in any thread:
//! 1. the mouse button it holds is released, before anything else;
//! 2. a dated text report is written in `%APPDATA%\MacroHUD\plantages\`;
//! 3. a message box says so, in French;
//! 4. MacroHUD exits.
//!
//! A crash outside Rust (access violation in a driver...) only gets step 1, from the crash filter
//! installed by [`install`]. A brutal end of the process (Task Manager, power cut) gets nothing: see
//! `session.rs`.

use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};

use crate::clock::Stamp;
use crate::contract::MouseButton;
use crate::guard::ButtonSet;
use crate::platform::{self, BoxKind};

/// Folder of the reports, inside the settings folder.
pub const FOLDER: &str = "plantages";

/// Installs the panic hook and the crash filter.
pub fn install(settings_dir: PathBuf) {
    platform::install_native_crash_handler();
    std::panic::set_hook(Box::new(move |info| on_panic(&settings_dir, info)));
}

fn on_panic(settings_dir: &Path, info: &PanicHookInfo<'_>) {
    // 1. Nothing stays pressed, whatever happens next.
    let released = platform::emergency_release();
    platform::set_panicking();

    let stamp = Stamp::now();
    let message = panic_message(info);
    let location = info.location().map(|l| format!("{}:{}", l.file(), l.line()));
    let thread = std::thread::current().name().unwrap_or("sans nom").to_string();
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let details = Details { message: &message, location: location.as_deref(), thread: &thread, released };
    let report = report_text(&stamp, crate::VERSION, &details, &backtrace);
    let written = write_report(&settings_dir.join(FOLDER), &stamp, &report);

    crate::log::write_now(format!("PLANTAGE dans le fil « {thread} » : {message}"));
    crate::session::end();
    let text = box_text(released, written.as_deref().map_err(|e| e.to_string()));
    platform::message_box("MacroHUD : plantage", &text, BoxKind::Error);
    std::process::exit(101);
}

fn panic_message(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "erreur sans description".to_string())
}

/// What the report says about the crash.
struct Details<'a> {
    message: &'a str,
    location: Option<&'a str>,
    thread: &'a str,
    released: ButtonSet,
}

fn released_text(released: ButtonSet) -> String {
    let names: Vec<&str> = released.iter().map(MouseButton::phrase).collect();
    match names.as_slice() {
        [] => "Aucun bouton de la souris n'était maintenu par MacroHUD.".to_string(),
        [one] => format!("{} maintenu par MacroHUD a été relâché.", capitalized(one)),
        many => format!("Boutons relâchés : {}.", many.join(", ")),
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn report_text(stamp: &Stamp, version: &str, details: &Details<'_>, backtrace: &str) -> String {
    format!(
        "Rapport de plantage de MacroHUD\n\
         ===============================\n\
         \n\
         Date : {date}\n\
         Version : {version}\n\
         Fil : {thread}\n\
         Erreur : {message}\n\
         Endroit dans le code : {location}\n\
         \n\
         Ce qui a été fait : {released} MacroHUD s'est ensuite fermé.\n\
         Pour aider à corriger ce plantage, transmettez ce fichier.\n\
         \n\
         Pile d'appels (technique)\n\
         -------------------------\n\
         {backtrace}\n",
        date = stamp.display(),
        thread = details.thread,
        message = details.message,
        location = details.location.unwrap_or("inconnu"),
        released = released_text(details.released),
    )
}

/// Writes the report; returns its path.
fn write_report(folder: &Path, stamp: &Stamp, report: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(folder)?;
    let base = format!("plantage-{}", stamp.file_part());
    let mut path = folder.join(format!("{base}.txt"));
    let mut n = 2;
    while path.exists() {
        path = folder.join(format!("{base}-{n}.txt"));
        n += 1;
    }
    std::fs::write(&path, report)?;
    Ok(path)
}

fn box_text(released: ButtonSet, written: Result<&Path, String>) -> String {
    let report = match written {
        Ok(path) => format!("Un rapport a été enregistré ici :\n{}", path.display()),
        Err(e) => format!("Le rapport n'a pas pu être enregistré ({e})."),
    };
    format!("MacroHUD a rencontré une erreur interne et va se fermer.\n\n{}\n\n{report}", released_text(released))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp() -> Stamp {
        Stamp { year: 2026, month: 11, day: 20, hour: 14, minute: 35, second: 12 }
    }

    #[test]
    fn the_report_is_readable_and_complete() {
        let details = Details {
            message: "index out of bounds",
            location: Some("src/engine.rs:42"),
            thread: "macrohud-moteur",
            released: ButtonSet::only(MouseButton::Left),
        };
        let text = report_text(&stamp(), "0.1.7", &details, "0: main");
        for expected in [
            "Date : 2026-11-20 14:35:12",
            "Version : 0.1.7",
            "Fil : macrohud-moteur",
            "Erreur : index out of bounds",
            "Endroit dans le code : src/engine.rs:42",
            "Le bouton gauche maintenu par MacroHUD a été relâché.",
            "0: main",
        ] {
            assert!(text.contains(expected), "missing « {expected} » in:\n{text}");
        }
    }

    #[test]
    fn released_buttons_are_described() {
        assert_eq!(released_text(ButtonSet::EMPTY), "Aucun bouton de la souris n'était maintenu par MacroHUD.");
        let two = ButtonSet::only(MouseButton::Left).with(MouseButton::Middle);
        assert_eq!(released_text(two), "Boutons relâchés : le bouton gauche, le bouton du milieu.");
    }

    #[test]
    fn reports_are_dated_and_never_overwritten() {
        let folder = std::env::temp_dir().join(format!("macrohud-tests-{}-plantages", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let first = write_report(&folder, &stamp(), "un").unwrap();
        let second = write_report(&folder, &stamp(), "deux").unwrap();
        assert!(first.ends_with("plantage-2026-11-20_14-35-12.txt"));
        assert!(second.ends_with("plantage-2026-11-20_14-35-12-2.txt"));
        assert_eq!(std::fs::read_to_string(first).unwrap(), "un");
    }

    #[test]
    fn the_message_box_says_where_the_report_is() {
        let text = box_text(ButtonSet::only(MouseButton::Right), Ok(Path::new("C:/x/plantage.txt")));
        assert!(text.contains("Le bouton droit maintenu par MacroHUD a été relâché."));
        assert!(text.contains("C:/x/plantage.txt"));
        let text = box_text(ButtonSet::EMPTY, Err("disque plein".into()));
        assert!(text.contains("n'a pas pu être enregistré (disque plein)"));
    }
}
