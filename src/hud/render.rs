//! Software rendering of egui for the tests, without GPU or window: egui's triangles are
//! rasterised the way its GPU back-ends do (premultiplied alpha, colours multiplied and blended
//! in gamma space, see the egui crate documentation), then written as PNG.
//!
//! `cargo test` writes the views of the mock-up, drawn by the real HUD code, into the folder
//! named by `MACROHUD_RENDUS` (default `target/rendus-hud`).

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use egui::epaint::{ImageData, Mesh, Primitive, TextureId, Vertex};
use egui::{
    Color32, ColorImage, Context, Event, PointerButton, Pos2, RawInput, Rect, TextureFilter,
    TexturesDelta, Ui, Vec2, ViewportId, ViewportInfo,
};

use super::demo::{self, Demo};
use super::{Hud, HudAction, HudView, MacroState, MouseButton, install_fonts, style};

// Rasteriser ---------------------------------------------------------------------------------

struct Texture {
    size: [usize; 2],
    pixels: Vec<Color32>,
    linear: bool,
}

impl Texture {
    fn texel(&self, x: i64, y: i64) -> [f32; 4] {
        let [w, h] = self.size;
        let x = x.clamp(0, w as i64 - 1) as usize;
        let y = y.clamp(0, h as i64 - 1) as usize;
        let c = self.pixels[y * w + x];
        [c.r(), c.g(), c.b(), c.a()].map(|v| f32::from(v) / 255.0)
    }

    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let (w, h) = (self.size[0] as f32, self.size[1] as f32);
        if !self.linear {
            return self.texel((u * w).floor() as i64, (v * h).floor() as i64);
        }
        let (x, y) = (u * w - 0.5, v * h - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let [a, b, c, d] = [
            self.texel(x0, y0),
            self.texel(x0 + 1, y0),
            self.texel(x0, y0 + 1),
            self.texel(x0 + 1, y0 + 1),
        ];
        std::array::from_fn(|k| {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bottom = c[k] + (d[k] - c[k]) * fx;
            top + (bottom - top) * fy
        })
    }
}

/// RGBA image, premultiplied alpha, gamma space, 0–1.
pub(crate) struct Image {
    pub width: usize,
    pub height: usize,
    pixels: Vec<[f32; 4]>,
}

fn edge(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Top-left fill rule: a pixel centre exactly on a shared edge is painted once, not twice.
fn covers(w: f32, a: [f32; 2], b: [f32; 2]) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    w > 0.0 || (w == 0.0 && ((dy == 0.0 && dx > 0.0) || dy < 0.0))
}

impl Image {
    pub fn new(width: usize, height: usize, background: Color32) -> Self {
        let fill = [
            background.r(),
            background.g(),
            background.b(),
            background.a(),
        ]
        .map(|v| f32::from(v) / 255.0);
        Self {
            width,
            height,
            pixels: vec![fill; width * height],
        }
    }

    pub fn pixel(&self, x: usize, y: usize) -> Color32 {
        let [r, g, b, a] =
            self.pixels[y * self.width + x].map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
        Color32::from_rgba_premultiplied(r, g, b, a)
    }

    fn draw(&mut self, mesh: &Mesh, clip: Rect, ppp: f32, textures: &HashMap<TextureId, Texture>) {
        let Some(texture) = textures.get(&mesh.texture_id) else {
            return;
        };
        let clip = [
            (clip.min.x * ppp).round().max(0.0) as usize,
            (clip.min.y * ppp).round().max(0.0) as usize,
            ((clip.max.x * ppp).round().max(0.0) as usize).min(self.width),
            ((clip.max.y * ppp).round().max(0.0) as usize).min(self.height),
        ];
        if clip[0] >= clip[2] || clip[1] >= clip[3] {
            return;
        }
        for triangle in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [0, 1, 2].map(|k| mesh.vertices[triangle[k] as usize]);
            self.triangle(a, b, c, ppp, texture, clip);
        }
    }

    fn triangle(
        &mut self,
        a: Vertex,
        b: Vertex,
        c: Vertex,
        ppp: f32,
        texture: &Texture,
        clip: [usize; 4],
    ) {
        let at = |v: &Vertex| [v.pos.x * ppp, v.pos.y * ppp];
        let (pa, mut pb, mut pc) = (at(&a), at(&b), at(&c));
        let (mut vb, mut vc) = (b, c);
        let mut area = edge(pa, pb, pc);
        if area == 0.0 {
            return;
        }
        if area < 0.0 {
            std::mem::swap(&mut pb, &mut pc);
            std::mem::swap(&mut vb, &mut vc);
            area = -area;
        }
        let min_x = pa[0].min(pb[0]).min(pc[0]).floor().max(clip[0] as f32) as usize;
        let max_x = (pa[0].max(pb[0]).max(pc[0]).ceil() as usize).min(clip[2]);
        let min_y = pa[1].min(pb[1]).min(pc[1]).floor().max(clip[1] as f32) as usize;
        let max_y = (pa[1].max(pb[1]).max(pc[1]).ceil() as usize).min(clip[3]);
        let colour = |v: &Vertex| {
            [v.color.r(), v.color.g(), v.color.b(), v.color.a()].map(|x| f32::from(x) / 255.0)
        };
        let (ca, cb, cc) = (colour(&a), colour(&vb), colour(&vc));
        for y in min_y..max_y {
            for x in min_x..max_x {
                let q = [x as f32 + 0.5, y as f32 + 0.5];
                let (w0, w1, w2) = (edge(pb, pc, q), edge(pc, pa, q), edge(pa, pb, q));
                if !(covers(w0, pb, pc) && covers(w1, pc, pa) && covers(w2, pa, pb)) {
                    continue;
                }
                let (l0, l1, l2) = (w0 / area, w1 / area, w2 / area);
                let u = a.uv.x * l0 + vb.uv.x * l1 + vc.uv.x * l2;
                let v = a.uv.y * l0 + vb.uv.y * l1 + vc.uv.y * l2;
                let texel = texture.sample(u, v);
                let source: [f32; 4] =
                    std::array::from_fn(|k| (ca[k] * l0 + cb[k] * l1 + cc[k] * l2) * texel[k]);
                let target = &mut self.pixels[y * self.width + x];
                for k in 0..4 {
                    target[k] = source[k] + target[k] * (1.0 - source[3]);
                }
            }
        }
    }

    /// Writes the image as an 8-bit RGBA PNG (straight alpha).
    pub fn save_png(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("dossier des rendus");
        }
        let mut bytes = Vec::with_capacity(self.width * self.height * 4);
        for &[r, g, b, a] in &self.pixels {
            let straight = |v: f32| if a > 0.0 { v / a } else { 0.0 };
            for v in [straight(r), straight(g), straight(b), a] {
                bytes.push((v * 255.0).round().clamp(0.0, 255.0) as u8);
            }
        }
        let file = BufWriter::new(File::create(path).expect("création du PNG"));
        let mut encoder = png::Encoder::new(file, self.width as u32, self.height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("en-tête PNG");
        writer.write_image_data(&bytes).expect("écriture du PNG");
    }
}

/// Reads an 8-bit RGB or RGBA PNG into an egui image (used for the scenery behind the HUD).
pub(crate) fn read_png(path: &Path) -> ColorImage {
    let file =
        BufReader::new(File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let mut reader = png::Decoder::new(file).read_info().expect("PNG lisible");
    let mut buffer = vec![0; reader.output_buffer_size().expect("taille du PNG")];
    let info = reader.next_frame(&mut buffer).expect("image PNG");
    let bytes = &buffer[..info.buffer_size()];
    let size = [info.width as usize, info.height as usize];
    match info.color_type {
        png::ColorType::Rgba => ColorImage::from_rgba_unmultiplied(size, bytes),
        png::ColorType::Rgb => ColorImage::from_rgb(size, bytes),
        other => panic!(
            "{}: format PNG non pris en charge ({other:?})",
            path.display()
        ),
    }
}

/// egui context with the HUD fonts, plus the textures it uploaded.
pub(crate) struct Renderer {
    pub ctx: Context,
    textures: HashMap<TextureId, Texture>,
}

impl Renderer {
    pub fn new() -> Self {
        let ctx = Context::default();
        install_fonts(&ctx);
        Self {
            ctx,
            textures: HashMap::new(),
        }
    }

    fn input(size: Vec2, pixels_per_point: f32, events: Vec<Event>) -> RawInput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        let info = ViewportInfo {
            native_pixels_per_point: Some(pixels_per_point),
            ..Default::default()
        };
        input.viewports.insert(ViewportId::ROOT, info);
        input
    }

    /// Uploads the texture changes of a pass, then marks them handled (egui checks it).
    fn apply(&mut self, delta: &mut TexturesDelta) {
        for (id, deltas) in &delta.set {
            for change in deltas {
                let ImageData::Color(image) = &change.image;
                let linear = change.options.magnification == TextureFilter::Linear;
                match change.pos {
                    None => {
                        let texture = Texture {
                            size: image.size,
                            pixels: image.pixels.clone(),
                            linear,
                        };
                        self.textures.insert(*id, texture);
                    }
                    Some([x0, y0]) => {
                        let texture = self.textures.get_mut(id).expect("texture connue");
                        let width = texture.size[0];
                        for y in 0..image.size[1] {
                            for x in 0..image.size[0] {
                                texture.pixels[(y0 + y) * width + x0 + x] =
                                    image.pixels[y * image.size[0] + x];
                            }
                        }
                    }
                }
            }
        }
        delta.clear();
    }

    /// One egui pass with `events`; returns nothing but keeps textures up to date.
    pub fn pass(&mut self, size: Vec2, events: Vec<Event>, ui_fn: &mut dyn FnMut(&mut Ui)) {
        let mut output = self
            .ctx
            .run_ui(Self::input(size, 1.0, events), |ui| ui_fn(ui));
        self.apply(&mut output.textures_delta);
    }

    /// Runs `ui_fn` on a screen of `size` points and rasterises it at `pixels_per_point`.
    pub fn render(
        &mut self,
        size: Vec2,
        pixels_per_point: f32,
        background: Color32,
        ui_fn: &mut dyn FnMut(&mut Ui),
    ) -> Image {
        // First pass loads the glyphs; the second one is painted.
        let mut first = self
            .ctx
            .run_ui(Self::input(size, pixels_per_point, Vec::new()), |ui| {
                ui_fn(ui)
            });
        self.apply(&mut first.textures_delta);
        let mut output = self
            .ctx
            .run_ui(Self::input(size, pixels_per_point, Vec::new()), |ui| {
                ui_fn(ui)
            });
        self.apply(&mut output.textures_delta);
        let ppp = output.pixels_per_point;
        let primitives = self.ctx.tessellate(output.shapes, ppp);
        let width = (size.x * ppp).round() as usize;
        let height = (size.y * ppp).round() as usize;
        let mut image = Image::new(width, height, background);
        for primitive in &primitives {
            if let Primitive::Mesh(mesh) = &primitive.primitive {
                image.draw(mesh, primitive.clip_rect, ppp, &self.textures);
            }
        }
        image
    }
}

/// Folder of the renders: `MACROHUD_RENDUS`, else `target/rendus-hud` of the crate.
pub(crate) fn output_dir() -> PathBuf {
    std::env::var_os("MACROHUD_RENDUS").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("rendus-hud")
        },
        PathBuf::from,
    )
}

/// HUD of a demo view, settings unfolded as in the mock-up.
pub(crate) fn demo_hud(demo: &Demo, shadow: bool) -> Hud {
    let mut hud = Hud::new();
    hud.set_open(demo.open);
    hud.shadow = shadow;
    hud
}

/// Renders the HUD alone, transparent around it, with the shadow of the mock-up.
fn render_alone(renderer: &mut Renderer, demo: &Demo, ppp: f32) -> Image {
    let mut hud = demo_hud(demo, true);
    let size = hud.size(&renderer.ctx, &demo.view);
    let [side, top, bottom] = style::SHADOW_MARGIN;
    let canvas = egui::vec2(size.x + 2.0 * side, size.y + top + bottom);
    let view = demo.view.clone();
    renderer.render(canvas, ppp, Color32::TRANSPARENT, &mut |ui| {
        let rect = Rect::from_min_size(egui::pos2(side, top), size);
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            hud.show(ui, &view);
        });
    })
}

// Tests ---------------------------------------------------------------------------------------

fn size_of(demo: &Demo) -> Vec2 {
    let mut renderer = Renderer::new();
    // Fonts are loaded lazily: run one empty pass before measuring.
    renderer.pass(egui::vec2(400.0, 400.0), Vec::new(), &mut |_| {});
    demo_hud(demo, false).size(&renderer.ctx, &demo.view)
}

/// Same sizes as the mock-up (measured in `maquettes/index.html`), plus the 1 px black ring.
#[test]
fn sizes_match_the_mockup() {
    let expected = [
        ("a", 290.0, 190.0),
        ("b", 290.0, 86.0),
        ("c", 290.0, 242.0),
        ("d", 290.0, 311.0),
        ("e", 290.0, 186.0),
        ("f1", 160.0, 36.0),
        ("f2", 153.0, 36.0),
    ];
    for (demo, (key, width, height)) in demo::mockup_views().iter().zip(expected) {
        assert_eq!(demo.key, key);
        let size = size_of(demo);
        assert_eq!(size.y, height, "hauteur de la vue {key}");
        assert!(
            (size.x - width).abs() <= 2.0,
            "largeur de la vue {key} : {}",
            size.x
        );
    }
}

/// Writes every view as PNG and checks a few pixels of the real rendering.
#[test]
fn renders_every_view() {
    let mut renderer = Renderer::new();
    let dir = output_dir();
    for demo in demo::mockup_views().iter().chain(&demo::extra_views()) {
        let image = render_alone(&mut renderer, demo, 1.0);
        image.save_png(&dir.join(format!("vue-{}.png", demo.key)));
        let large = render_alone(&mut renderer, demo, 2.0);
        large.save_png(&dir.join(format!("vue-{}@2x.png", demo.key)));
        let [side, top, _] = style::SHADOW_MARGIN.map(|v| v as usize);
        // Black ring, then the grey border, at the middle of the top edge.
        assert_eq!(
            image.pixel(side + 60, top),
            Color32::BLACK,
            "anneau, vue {}",
            demo.key
        );
        assert_eq!(
            image.pixel(side + 60, top + 1),
            style::EDGE,
            "bord, vue {}",
            demo.key
        );
        if demo.key == "b" {
            // The whole line of the active macro is green (above the text, left of the key).
            assert_eq!(image.pixel(side + 200, top + 2 + 32 + 3), style::ON);
        }
        if demo.key == "a" {
            assert_eq!(image.pixel(side + 200, top + 2 + 32 + 3), style::ROW);
        }
    }
}

// Interaction tests: real pointer events through egui.

fn click_at(view: &HudView, open: Option<usize>, point: Pos2) -> (Vec<HudAction>, Hud) {
    let mut renderer = Renderer::new();
    let mut hud = Hud::new();
    hud.set_open(open);
    let size = egui::vec2(400.0, 400.0);
    let mut actions = Vec::new();
    let steps = [
        vec![Event::PointerMoved(point)],
        vec![Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
        vec![Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
        vec![],
    ];
    for events in steps {
        renderer.pass(size, events, &mut |ui| actions.extend(hud.show(ui, view)));
    }
    (actions, hud)
}

fn one_macro(state: MacroState) -> HudView {
    let demos = demo::mockup_views();
    let mut view = demos[0].view.clone();
    view.macros[0].state = state;
    view
}

#[test]
fn clicks_ask_for_the_contract_actions() {
    let stopped = one_macro(MacroState::Stopped);
    // ▶ of the first line: ring 1 + border 1 + 14 px, line under the 32 px bar.
    let toggle = Pos2::new(2.0 + 14.0 + 16.0, 2.0 + 32.0 + 24.0);
    assert_eq!(
        click_at(&stopped, None, toggle).0,
        vec![HudAction::Toggle(0)]
    );
    let active = one_macro(MacroState::Active);
    assert_eq!(
        click_at(&active, None, toggle).0,
        vec![HudAction::Toggle(0)]
    );

    // « Quitter » and « Replier » in the title bar.
    let quit = Pos2::new(2.0 + 286.0 - 4.0 - 20.0, 17.0);
    assert_eq!(click_at(&stopped, None, quit).0, vec![HudAction::Quit]);
    let fold = Pos2::new(2.0 + 286.0 - 4.0 - 64.0 - 6.0 - 30.0, 17.0);
    assert_eq!(
        click_at(&stopped, None, fold).0,
        vec![HudAction::SetCollapsed(true)]
    );

    // Settings: « Droit » in the selector, « Changer » on the shortcut line.
    let settings_top = 2.0 + 32.0 + 50.0;
    let right = Pos2::new(2.0 + 12.0 + 133.0, settings_top + 8.0 + 15.0 + 4.0 + 15.0);
    assert_eq!(
        click_at(&stopped, Some(0), right).0,
        vec![HudAction::SetButton(0, MouseButton::Right)]
    );
    let change = Pos2::new(2.0 + 12.0 + 266.0 - 20.0, settings_top + 67.0 + 13.0);
    assert_eq!(
        click_at(&stopped, Some(0), change).0,
        vec![HudAction::StartCapture(0)]
    );

    // The arrow unfolds the settings: no action, the HUD grows.
    let arrow = Pos2::new(2.0 + 286.0 - 6.0 - 12.0, 2.0 + 32.0 + 24.0);
    let (actions, hud) = click_at(&stopped, None, arrow);
    assert!(actions.is_empty());
    let mut renderer = Renderer::new();
    renderer.pass(egui::vec2(400.0, 400.0), Vec::new(), &mut |_| {});
    assert_eq!(hud.size(&renderer.ctx, &stopped).y, 190.0);

    // « Annuler » while a shortcut is awaited.
    let mut capture = stopped.clone();
    capture.capture = Some(0);
    let cancel = Pos2::new(
        2.0 + 12.0 + 266.0 - 11.0 - 20.0,
        settings_top + 67.0 + 11.0 + 24.0 + 6.0 + 13.0,
    );
    assert_eq!(
        click_at(&capture, None, cancel).0,
        vec![HudAction::CancelCapture]
    );

    // The folded pill unfolds.
    let mut folded = stopped.clone();
    folded.collapsed = true;
    let unfold = Pos2::new(160.0 - 2.0 - 4.0 - 12.0, 18.0);
    assert_eq!(
        click_at(&folded, None, unfold).0,
        vec![HudAction::SetCollapsed(false)]
    );
}

#[test]
fn coming_macros_cannot_be_clicked() {
    let demos = demo::mockup_views();
    let view = &demos[4].view;
    for line in [1.0, 2.0] {
        let y = 2.0 + 32.0 + 50.0 * line + 24.0;
        for x in [32.0, 200.0, 270.0] {
            assert!(
                click_at(view, None, Pos2::new(x, y)).0.is_empty(),
                "ligne {line}, x {x}"
            );
        }
    }
}

#[test]
fn window_is_resized_once_per_change() {
    let ctx = Context::default();
    let size = egui::vec2(290.0, 86.0);
    let mut commands = Vec::new();
    for wanted in [size, size, egui::vec2(290.0, 190.0)] {
        let mut output = ctx.run_ui(
            Renderer::input(egui::vec2(300.0, 300.0), 1.0, Vec::new()),
            |ui| {
                super::fit_window(ui.ctx(), wanted);
            },
        );
        output.textures_delta.clear();
        commands.extend(
            output
                .viewport_output
                .remove(&ViewportId::ROOT)
                .map(|v| v.commands)
                .unwrap_or_default(),
        );
    }
    let resizes: Vec<_> = commands
        .into_iter()
        .filter(|c| matches!(c, egui::ViewportCommand::InnerSize(_)))
        .collect();
    assert_eq!(resizes.len(), 2, "une commande par changement de taille");
}
