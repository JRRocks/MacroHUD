//! Everything that depends on the operating system.
//!
//! On Windows (`windows.rs`): `SendInput`, the low-level mouse and keyboard hooks, `RegisterHotKey`,
//! the single-instance mutex, message boxes, window placement. Elsewhere (`other.rs`): stand-ins that
//! let MacroHUD build, be tested and show its HUD on Linux, without touching the real mouse.
//!
//! Both provide the same items: `spawn_engine`, `HudWindow`, `Job`, `emergency_release`,
//! `set_panicking`, `install_native_crash_handler`, `single_instance`, `message_box`,
//! `attach_console`, `settings_dir`, `local_time`.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
mod other;
#[cfg(not(windows))]
pub use other::*;

/// The icon of a message box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    Info,
    Error,
}
