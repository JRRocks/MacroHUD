//! MacroHUD: keyboard and mouse macros for Windows, driven from a small HUD that stays on top of
//! the other windows. First macro: « Maintenir le clic » (D-04).
//!
//! Modules:
//! - `contract`: what the HUD receives and what it may ask (docs/CONTRAT-HUD.md) as Rust types;
//! - `hud`: the HUD itself, owned by the "visuel" session;
//! - `app`: the window, the glue between the HUD and the engine, the settings;
//! - `engine`: the macros' state machine, independent of Windows (tested on Linux);
//! - `hotkey`, `capture`, `guard`: hotkeys and their rules, capture of a new hotkey, physical-click
//!   guard (pure logic, tested on Linux);
//! - `settings`, `session`, `log`, `crash`, `clock`: settings file, abrupt-end marker, run log,
//!   crash reports, dates;
//! - `runner`, `platform`: the engine's thread and everything that calls Windows.

// No console window when MacroHUD starts (release builds; debug builds keep one for development).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
// The capture and the guard run inside the Windows hooks; elsewhere only their tests use them.
#[cfg_attr(not(windows), allow(dead_code))]
mod capture;
mod clock;
mod contract;
mod crash;
mod engine;
#[cfg_attr(not(windows), allow(dead_code))]
mod guard;
mod hotkey;
mod hud;
mod log;
mod macros;
mod platform;
mod runner;
mod session;
mod settings;

use std::process::ExitCode;

use contract::{Message, MessageLevel};
use platform::BoxKind;

/// Name of the software.
pub const NAME: &str = "MacroHUD";
/// Delivery version: 0.1.<CI run number>, or 0.1.0-dev for a local build (see build.rs).
pub const VERSION: &str = env!("MACROHUD_VERSION");

const USAGE: &str = "Utilisation : macrohud [--version | --aide | --essai-plantage]\n\
    \n\
    Sans option : ouvre le HUD de MacroHUD.\n\
    --version          affiche le numéro de version\n\
    --aide             affiche cette aide\n\
    --essai-plantage   pour les essais : plante exprès à l'arrêt d'un maintien\n\
    \u{20}                  (le bouton doit être relâché et un rapport écrit)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => run(),
        Some("--essai-plantage") => {
            crash::arm_crash_test();
            run()
        }
        Some("--version" | "-V") => {
            platform::attach_console();
            println!("{NAME} {VERSION}");
            ExitCode::SUCCESS
        }
        Some("--aide" | "--help" | "-h" | "/?") => {
            platform::attach_console();
            println!("{NAME} {VERSION}\n\n{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            platform::attach_console();
            eprintln!("Option inconnue : {other}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run() -> ExitCode {
    // First: a second MacroHUD must not touch the first one's files (log, session marker).
    let Some(_instance) = platform::single_instance() else {
        platform::message_box(
            NAME,
            "MacroHUD est déjà ouvert : son HUD est au-dessus des autres fenêtres.\n\nCette seconde copie se ferme.",
            BoxKind::Info,
        );
        return ExitCode::SUCCESS;
    };

    let dir = platform::settings_dir();
    log::init(&dir);
    log::write(format!("{NAME} {VERSION} démarre."));
    crash::install(dir.clone());

    let mut messages = Vec::new();
    if session::begin(&dir) {
        log::write("La session précédente ne s'était pas terminée normalement.");
        messages.push(Message::new(MessageLevel::Warning, session::ABRUPT_END_WARNING));
    }
    let stamp = clock::Stamp::now().file_part();
    let loaded = settings::load(&dir, macros::CATALOG, &stamp);
    if let Some(warning) = loaded.warning {
        log::write(format!("Réglages : {warning}"));
        messages.push(Message::new(MessageLevel::Warning, warning));
    }
    if loaded.rewrite
        && let Err(e) = settings::save(&dir, &loaded.settings, &|h| h.label(&|_| None))
    {
        log::write(format!("Réglages corrigés non enregistrés : {e}"));
    }

    let result = app::run(app::Startup { dir, settings: loaded.settings, messages });

    // Whatever happened, nothing stays pressed.
    let released = platform::emergency_release();
    if !released.is_empty() {
        log::write(format!("Relâché en sortie : {released:?}"));
    }
    session::end();
    match result {
        Ok(()) => {
            log::write("MacroHUD se ferme normalement.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            log::write(format!("Impossible d'ouvrir la fenêtre : {e}"));
            platform::message_box(
                NAME,
                &format!(
                    "MacroHUD ne peut pas ouvrir sa fenêtre.\n\n{e}\n\n\
                     Si l'erreur parle d'OpenGL : MacroHUD a besoin d'une carte graphique \
                     (et de son pilote) compatible OpenGL 2 ou plus récent."
                ),
                BoxKind::Error,
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_version_is_set() {
        assert!(!super::VERSION.is_empty());
        assert!(super::VERSION.starts_with("0.1."), "{}", super::VERSION);
    }
}
