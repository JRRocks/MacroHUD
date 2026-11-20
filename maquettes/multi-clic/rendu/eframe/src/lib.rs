//! The HUD's files (`src/hud/`) reach egui as `eframe::egui`; these renders need no window, so
//! this stands for eframe and only passes egui on.

pub use egui;
