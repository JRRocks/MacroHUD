//! Renders of the « Multi-clic » preview (task P2.2): the HUD with two macros, in the three ways
//! of setting the speed (A, B, C), three views each, drawn with egui and the HUD's own files
//! (`src/hud/`) on the real contract types (`src/contract.rs`). Mock-ups only: none of this is
//! MacroHUD's code, and nothing here changes it.
//!
//! `cargo run --release --manifest-path maquettes/multi-clic/rendu/Cargo.toml` (from the root of
//! MacroHUD, or plain `cargo run --release` here) writes the PNGs next to `apercu.html`, then
//! rebuilds the page from `apercu.modele.html` with the images inside it (data URIs). The two
//! « Aujourd'hui » images are those of `maquettes/rendu-egui/`, which MacroHUD's own test renders
//! from the current HUD: `MACROHUD_RENDUS=maquettes/rendu-egui cargo test hud::` at the root, first.
//! With `-- --mesures`, it only prints the room the state lines of « Multi-clic » have.

#![allow(dead_code, reason = "the HUD's files bring helpers that these renders do not use")]

#[path = "../../../../src/contract.rs"]
mod contract;
mod hud;
mod page;
mod render;

use std::path::{Path, PathBuf};

use eframe::egui::{Pos2, Rect, UiBuilder, pos2, vec2};

use contract::{MacroStatus, MouseButton, samples};
use hud::{MultiClick, Preview, Speed};

/// Room around the HUD for its drop shadow: left and right, top, bottom (as `src/hud/render.rs`).
const SHADOW_MARGIN: [f32; 3] = [16.0, 10.0, 22.0];
/// Drawn at twice the size (Windows at 200 %), shown at their own size in the page.
const SCALE: f32 = 2.0;

/// The three tracks, by letter.
const TRACKS: [(&str, Speed); 3] = [("a", Speed::Slider), ("b", Speed::Stepper), ("c", Speed::Presets)];

/// The three views of each track: settings of « Multi-clic » open with the macro stopped, then
/// active, then active with the mouse on the HUD (its clicks wait).
const VIEWS: [(&str, bool, bool); 3] =
    [("1-reglages", false, false), ("2-active", true, false), ("3-en-pause", true, true)];

/// « Multi-clic » with its defaults (D-22): left button, 10 clicks per second, from 1 to 15, F9.
fn multi_click(active: bool, suspended: bool) -> MultiClick {
    MultiClick {
        status: if active { MacroStatus::Active } else { MacroStatus::Stopped },
        hotkey: "F9".into(),
        button: MouseButton::Left,
        rate: 10,
        min: 1,
        max: 15,
        suspended,
    }
}

fn preview(speed: Speed, active: bool, suspended: bool) -> Preview {
    // The contract's first situation, « arrêtée »: the title and « Maintenir le clic » on F8.
    let state = samples().remove(0).1;
    Preview { state, multi: multi_click(active, suspended), speed, open: true }
}

/// The HUD alone, transparent around it, with its shadow; the mouse pointer drawn at `spot`.
fn render_alone(renderer: &mut render::Renderer, preview: &Preview, spot: Option<Pos2>) -> render::Image {
    let size = hud::size(preview);
    let [side, top, bottom] = SHADOW_MARGIN;
    let canvas = vec2(size.x + 2.0 * side, size.y + top + bottom);
    let origin = pos2(side, top);
    let pointer = spot.map(|spot| origin + spot.to_vec2());
    renderer.render(canvas, SCALE, pointer, &mut |ui| {
        ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(origin, size)), |ui| hud::paint(ui, preview));
        if let Some(tip) = pointer {
            hud::paint_pointer(ui.painter(), tip);
        }
    })
}

/// Prints, for each state line of « Multi-clic », its width and the room it has.
fn measures() {
    for (active, suspended) in [(false, false), (true, false), (true, true)] {
        let multi = multi_click(active, suspended);
        let words = hud::multi_words(&multi);
        let width = hud::state_width(&words, active);
        let room = hud::state_room(&multi.hotkey, active);
        println!("{words:<34} {width:6.1} / {room:5.1} pt");
    }
}

fn main() {
    if std::env::args().any(|a| a == "--mesures") {
        measures();
        return;
    }
    let folder: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("dossier de l'aperçu").to_path_buf();
    let mut renderer = render::Renderer::new();
    for (letter, speed) in TRACKS {
        for (view, active, suspended) in VIEWS {
            let preview = preview(speed, active, suspended);
            let spot = suspended.then(|| hud::pointer_spot(&preview));
            let image = render_alone(&mut renderer, &preview, spot);
            let path = folder.join(format!("{letter}-{view}@2x.png"));
            image.save_png(&path);
            println!("{} ({} × {})", path.display(), image.width, image.height);
        }
    }
    page::write(&folder);
}

/// Every state line of « Multi-clic » fits its line, for each button and each speed: none is cut.
#[test]
fn state_lines_fit_their_line() {
    for button in MouseButton::ALL {
        for rate in 1..=15 {
            for (active, suspended) in [(false, false), (true, false), (true, true)] {
                let multi = MultiClick { button, rate, ..multi_click(active, suspended) };
                let words = hud::multi_words(&multi);
                let (width, room) = (hud::state_width(&words, active), hud::state_room(&multi.hotkey, active));
                assert!(width <= room, "« {words} » : {width:.1} pt pour {room:.1}");
            }
        }
    }
}
