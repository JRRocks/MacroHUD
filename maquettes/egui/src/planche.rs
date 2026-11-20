//! Comparison sheets: the views of the mock-up drawn by the real HUD code (`src/hud/`), laid out
//! like `maquettes/variante-2.png` on the same scenery, so both images compare side by side.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::epaint::RectShape;
use egui::{
    Color32, Rect, Stroke, StrokeKind, TextureHandle, TextureOptions, UiBuilder, Vec2, pos2, vec2,
};

use crate::hud::demo::{self, Demo};
use crate::hud::render::{self, Image, Renderer};
use crate::hud::text;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

// Page of the mock-ups (maquettes/outils/generer.mjs).
const PAGE: Color32 = rgb(0xe7e5e0);
const SECTION: Color32 = rgb(0xf7f6f3);
const SECTION_EDGE: Color32 = rgb(0xd3d0c9);
const INK: Color32 = rgb(0x1c1d20);
const INK_2: Color32 = rgb(0x43464c);
const INK_3: Color32 = rgb(0x686b72);

const WIDTH: f32 = 1600.0;
const SECTION_LEFT: f32 = 40.0;
const SECTION_WIDTH: f32 = 1520.0;
const PADDING: f32 = 28.0;
const COLUMN: f32 = 474.0;
const GAP: f32 = 20.0;
const CAPTION: f32 = 32.0;
const HUD_GAP: f32 = 18.0;

struct Tile {
    letter: &'static str,
    caption: &'static str,
    scene: &'static str,
    demos: Vec<Demo>,
}

fn scene_label(scene: &str) -> &'static str {
    match scene {
        "bureau" => "fond : bureau clair",
        "nuit" => "fond : jeu de nuit",
        _ => "fond : jeu en plein jour",
    }
}

fn load_scenes(renderer: &Renderer) -> HashMap<&'static str, TextureHandle> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fonds");
    ["bureau", "nuit", "jour"]
        .into_iter()
        .map(|name| {
            let image = render::read_png(&dir.join(format!("{name}.png")));
            (
                name,
                renderer
                    .ctx
                    .load_texture(name, image, TextureOptions::LINEAR),
            )
        })
        .collect()
}

/// UV rectangle showing the middle of the texture at the proportions of `size` (CSS « cover »).
fn cover(texture: &TextureHandle, size: Vec2) -> Rect {
    let [w, h] = texture.size().map(|v| v as f32);
    let scale = (size.x / w).max(size.y / h);
    let (u, v) = (size.x / scale / w, size.y / scale / h);
    Rect::from_min_max(
        pos2((1.0 - u) / 2.0, (1.0 - v) / 2.0),
        pos2((1.0 + u) / 2.0, (1.0 + v) / 2.0),
    )
}

fn sheet(renderer: &mut Renderer, number: &str, title: &str, idea: &str, tiles: &[Tile]) -> Image {
    let scenes = load_scenes(renderer);
    renderer.pass(vec2(WIDTH, 100.0), Vec::new(), &mut |_| {});
    let ctx = renderer.ctx.clone();
    let painter = ctx.layer_painter(egui::LayerId::background());

    // Measures: HUD sizes, stage heights, rows.
    let sizes: Vec<Vec<Vec2>> = tiles
        .iter()
        .map(|t| {
            t.demos
                .iter()
                .map(|d| render::demo_hud(d, true).size(&ctx, &d.view))
                .collect()
        })
        .collect();
    let stage_height = |hud: &[Vec2]| {
        let content: f32 =
            hud.iter().map(|s| s.y).sum::<f32>() + HUD_GAP * (hud.len() as f32 - 1.0);
        (content + 56.0).max(380.0)
    };
    let rows: Vec<f32> = sizes
        .chunks(3)
        .map(|row| row.iter().map(|hud| stage_height(hud)).fold(0.0, f32::max) + CAPTION)
        .collect();
    let text_left = SECTION_LEFT + 1.0 + PADDING + 56.0 + 16.0;
    let idea_width = SECTION_LEFT + SECTION_WIDTH - 1.0 - PADDING - text_left;
    let idea_height = text::paragraph(&painter, idea, 16.0, 400.0, INK_2, idea_width, 23.0)
        .size()
        .y;
    let head = (34.0 + 3.0 + idea_height).max(56.0);
    let section_top = 20.0;
    let grid_top = section_top + 1.0 + 26.0 + head + 20.0;
    let grid_height = rows.iter().sum::<f32>() + GAP * (rows.len() as f32 - 1.0);
    let section_bottom = grid_top + grid_height + 30.0 + 1.0;
    let height = (section_bottom + 20.0).ceil();

    let mut huds: Vec<Vec<crate::hud::Hud>> = tiles
        .iter()
        .map(|t| t.demos.iter().map(|d| render::demo_hud(d, true)).collect())
        .collect();

    renderer.render(vec2(WIDTH, height), 1.0, PAGE, &mut |ui| {
        let p = ui.painter().clone();
        let section = Rect::from_min_max(
            pos2(SECTION_LEFT, section_top),
            pos2(SECTION_LEFT + SECTION_WIDTH, section_bottom),
        );
        p.rect(
            section,
            18,
            SECTION,
            Stroke::new(1.0, SECTION_EDGE),
            StrokeKind::Inside,
        );

        // Heading: number, title, idea.
        let head_top = section_top + 1.0 + 26.0;
        let badge = Rect::from_min_size(
            pos2(SECTION_LEFT + 1.0 + PADDING, head_top + (head - 56.0) / 2.0),
            vec2(56.0, 56.0),
        );
        p.rect_filled(badge, 14, INK);
        let digit = text::line(&p, number, 32.0, 800.0, Color32::WHITE, f32::INFINITY);
        text::put(
            &p,
            &digit,
            badge.center().x - digit.size().x / 2.0,
            badge.top(),
            56.0,
        );
        let text_top = head_top + (head - (34.0 + 3.0 + idea_height)) / 2.0;
        text::draw(&p, title, 28.0, 800.0, INK, text_left, text_top, 34.0);
        let paragraph = text::paragraph(&p, idea, 16.0, 400.0, INK_2, idea_width, 23.0);
        p.galley(pos2(text_left, text_top + 37.0), paragraph, INK_2);

        // Tiles.
        let mut row_top = grid_top;
        for (r, row_height) in rows.iter().enumerate() {
            for c in 0..3 {
                let index = r * 3 + c;
                let Some(tile) = tiles.get(index) else {
                    continue;
                };
                let left = SECTION_LEFT + 1.0 + PADDING + c as f32 * (COLUMN + GAP);
                // Caption: letter, text, scenery.
                let letter = Rect::from_min_size(pos2(left, row_top), vec2(24.0, 24.0));
                p.rect_filled(letter, 6, INK);
                let glyph = text::line(&p, tile.letter, 14.0, 700.0, Color32::WHITE, f32::INFINITY);
                text::put(
                    &p,
                    &glyph,
                    letter.center().x - glyph.size().x / 2.0,
                    letter.top(),
                    24.0,
                );
                text::draw(
                    &p,
                    tile.caption,
                    15.0,
                    600.0,
                    INK,
                    letter.right() + 8.0,
                    row_top,
                    24.0,
                );
                let label = text::line(
                    &p,
                    scene_label(tile.scene),
                    13.0,
                    500.0,
                    INK_3,
                    f32::INFINITY,
                );
                text::put(&p, &label, left + COLUMN - label.size().x, row_top, 24.0);
                // Stage with its scenery, HUDs centred.
                let stage = Rect::from_min_size(
                    pos2(left, row_top + CAPTION),
                    vec2(COLUMN, row_height - CAPTION),
                );
                let texture = &scenes[tile.scene];
                p.add(
                    RectShape::filled(stage, 12, Color32::WHITE)
                        .with_texture(texture.id(), cover(texture, stage.size())),
                );
                let content: f32 = sizes[index].iter().map(|s| s.y).sum::<f32>()
                    + HUD_GAP * (sizes[index].len() as f32 - 1.0);
                let mut y = (stage.center().y - content / 2.0).round();
                for (k, demo) in tile.demos.iter().enumerate() {
                    let size = sizes[index][k];
                    let rect = Rect::from_min_size(
                        pos2((stage.center().x - size.x / 2.0).round(), y),
                        size,
                    );
                    let hud = &mut huds[index][k];
                    ui.scope_builder(UiBuilder::new().max_rect(rect).id_salt((index, k)), |ui| {
                        hud.show(ui, &demo.view);
                    });
                    y += size.y + HUD_GAP;
                }
            }
            row_top += row_height + GAP;
        }
    })
}

fn tile(
    letter: &'static str,
    caption: &'static str,
    scene: &'static str,
    demos: Vec<Demo>,
) -> Tile {
    Tile {
        letter,
        caption,
        scene,
        demos,
    }
}

#[test]
fn comparison_sheets() {
    let mut renderer = Renderer::new();
    let dir = render::output_dir();

    let mut views = demo::mockup_views().into_iter();
    let mut next = || views.next().expect("vue de la maquette");
    let tiles = vec![
        tile("a", "Arrêtée", "bureau", vec![next()]),
        tile("b", "Active, bouton gauche enfoncé", "nuit", vec![next()]),
        tile("c", "Capture d’un nouveau raccourci", "jour", vec![next()]),
        tile("d", "Raccourci refusé", "bureau", vec![next()]),
        tile(
            "e",
            "Trois macros, dont deux « À venir »",
            "nuit",
            vec![next()],
        ),
        tile(
            "f",
            "Repliée : arrêtée, puis active",
            "jour",
            vec![next(), next()],
        ),
    ];
    let image = sheet(
        &mut renderer,
        "2",
        "Variante 2 — Liste compacte : rendu réel egui",
        "Les six vues de la maquette, dessinées par le code du HUD (src/hud, egui 0.37, police Inter) et rendues sans écran, à taille réelle. À comparer avec maquettes/variante-2.png.",
        &tiles,
    );
    image.save_png(&dir.join("variante-2-egui.png"));
    assert_eq!(image.width, 1600);

    let mut extras = demo::extra_views().into_iter();
    let mut next = || extras.next().expect("vue hors maquette");
    let tiles = vec![
        tile("g", "Macro indisponible", "bureau", vec![next()]),
        tile("h", "Message d’information", "nuit", vec![next()]),
        tile("i", "Message d’erreur", "jour", vec![next()]),
    ];
    let image = sheet(
        &mut renderer,
        "+",
        "États sans maquette, à valider",
        "Cas prévus par le contrat que la maquette ne montrait pas, dessinés avec les mêmes éléments : macro indisponible (avec sa raison), message d’information, message d’erreur.",
        &tiles,
    );
    image.save_png(&dir.join("hors-maquette-egui.png"));
}
