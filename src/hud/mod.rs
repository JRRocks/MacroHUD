//! The HUD. **Provisional debug version**: egui's default widgets only, no colour or layout choice.
//! The "visuel" session replaces it with the validated HUD (P1.5), keeping the items below, which
//! are all the rest of MacroHUD uses:
//!
//! - [`setup`]: called once at start (fonts, theme...);
//! - [`draw`]: draws the HUD from a [`HudState`] and returns the [`HudAction`]s the user asked for;
//! - [`window_size`]: the window's inner size, in points, for a given state (the window follows it);
//! - [`TRANSPARENT`]: whether the window is transparent (for rounded or irregular shapes);
//! - [`MESSAGE_DURATION`]: how long a message stays shown;
//! - [`window_icon`]: the window's icon (taskbar). If `src/hud/icone.ico` exists, it also becomes
//!   the icon of `macrohud.exe` (see `build.rs`).
//!
//! Everything the HUD shows comes from `crate::contract` (docs/CONTRAT-HUD.md); everything it asks
//! goes through [`HudAction`]. Previews of every situation: `crate::contract::samples()`.

use std::time::Duration;

use eframe::egui;

use crate::contract::{HudAction, HudState, MacroOptions, MacroStatus, MacroView, Message, MessageLevel, MouseButton};

/// Whether the window is transparent where the HUD draws nothing.
pub const TRANSPARENT: bool = false;

/// How long a message stays shown.
pub const MESSAGE_DURATION: Duration = Duration::from_secs(6);

/// Called once, before the first frame.
pub fn setup(_ctx: &egui::Context) {}

/// The window's icon; an empty one means « the icon of macrohud.exe ».
pub fn window_icon() -> egui::IconData {
    egui::IconData::default()
}

/// The window's inner size, in points.
pub fn window_size(state: &HudState) -> egui::Vec2 {
    let macros = state.macros.len() as f32;
    if state.collapsed { egui::vec2(380.0, 96.0 + 24.0 * macros) } else { egui::vec2(380.0, 130.0 + 200.0 * macros) }
}

/// Draws the HUD; returns what the user asked for during this frame.
pub fn draw(ui: &mut egui::Ui, state: &HudState) -> Vec<HudAction> {
    let mut actions = Vec::new();
    egui::CentralPanel::default_margins().show(ui, |ui| {
        title_bar(ui, state, &mut actions);
        match &state.message {
            Some(message) => message_line(ui, message),
            None => {
                ui.label("");
            }
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for view in &state.macros {
                if state.collapsed {
                    ui.label(format!("{} : {} ({})", view.name, status_text(&view.status), view.hotkey));
                } else {
                    macro_panel(ui, state, view, &mut actions);
                    ui.separator();
                }
            }
        });
    });
    actions
}

fn title_bar(ui: &mut egui::Ui, state: &HudState, actions: &mut Vec<HudAction>) {
    ui.horizontal_wrapped(|ui| {
        let handle = ui
            .add(egui::Label::new(format!("{} (glisser ici pour déplacer)", state.title())).sense(egui::Sense::drag()));
        if handle.drag_started_by(egui::PointerButton::Primary) {
            actions.push(HudAction::StartDrag);
        }
        let toggle = if state.collapsed { "Déplier" } else { "Replier" };
        if ui.button(toggle).clicked() {
            actions.push(HudAction::SetCollapsed(!state.collapsed));
        }
        if ui.button("Quitter").clicked() {
            actions.push(HudAction::Quit);
        }
    });
}

fn message_line(ui: &mut egui::Ui, message: &Message) {
    let level = match message.level {
        MessageLevel::Info => "Info",
        MessageLevel::Warning => "Attention",
        MessageLevel::Error => "Erreur",
    };
    ui.label(format!("{level} : {}", message.text));
}

fn status_text(status: &MacroStatus) -> String {
    match status {
        MacroStatus::Stopped => "arrêtée".to_string(),
        MacroStatus::Active => "active".to_string(),
        MacroStatus::Unavailable(reason) => format!("indisponible ({reason})"),
    }
}

fn macro_panel(ui: &mut egui::Ui, state: &HudState, view: &MacroView, actions: &mut Vec<HudAction>) {
    ui.add_enabled_ui(view.available, |ui| {
        if view.available {
            ui.strong(view.name);
        } else {
            ui.strong(format!("{} (à venir)", view.name));
        }
        ui.label(view.description);
        ui.label(format!("État : {}", status_text(&view.status)));

        if state.capturing == Some(view.id) {
            ui.label("Raccourci : appuyez sur la touche ou la combinaison voulue (Échap annule)");
            if ui.button("Annuler").clicked() {
                actions.push(HudAction::CancelCapture);
            }
        } else {
            ui.horizontal(|ui| {
                ui.label(format!("Raccourci : {}", view.hotkey));
                if ui.button("Changer").clicked() {
                    actions.push(HudAction::StartCapture(view.id));
                }
            });
        }

        match view.options {
            MacroOptions::HoldClick { button } => {
                ui.horizontal(|ui| {
                    ui.label("Bouton :");
                    for choice in MouseButton::ALL {
                        if ui.radio(button == choice, choice.label()).clicked() && button != choice {
                            actions.push(HudAction::SetOptions(view.id, MacroOptions::HoldClick { button: choice }));
                        }
                    }
                });
            }
        }

        match view.status {
            MacroStatus::Active => {
                if ui.button("Arrêter").clicked() {
                    actions.push(HudAction::Stop(view.id));
                }
            }
            MacroStatus::Stopped => {
                if ui.button("Activer").clicked() {
                    actions.push(HudAction::Start(view.id));
                }
            }
            MacroStatus::Unavailable(_) => {
                ui.add_enabled(false, egui::Button::new("Activer"));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::samples;

    /// Runs one frame of the HUD without a window, at its own window size.
    fn frame(state: &HudState) -> Vec<HudAction> {
        let ctx = egui::Context::default();
        setup(&ctx);
        let size = window_size(state);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        let mut actions = Vec::new();
        let mut output = ctx.run_ui(input, |ui| actions = draw(ui, state));
        // No renderer here: the font textures egui wanted to upload are dropped on purpose.
        output.textures_delta.clear();
        actions
    }

    #[test]
    fn every_sample_state_draws_without_asking_anything() {
        for (name, state) in samples() {
            let size = window_size(&state);
            assert!(size.x > 0.0 && size.y > 0.0, "{name}");
            assert!(frame(&state).is_empty(), "{name}: actions without any input");
        }
    }

    #[test]
    fn the_collapsed_hud_is_not_larger() {
        for (name, state) in samples() {
            let mut collapsed = state.clone();
            collapsed.collapsed = true;
            let mut expanded = state;
            expanded.collapsed = false;
            assert!(window_size(&collapsed).y <= window_size(&expanded).y, "{name}");
        }
    }
}
