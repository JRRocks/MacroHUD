//! Render tests of the HUD, without GPU or window: egui's triangles are rasterised in software the
//! way its GPU back-ends do (premultiplied alpha, colours multiplied and blended in gamma space,
//! see the egui crate documentation), then written as PNG.
//!
//! `cargo test` writes, into the folder named by `MACROHUD_RENDUS` (default `target/rendus-hud`),
//! the six views of the mock-up and every situation of `contract::samples()`, drawn by the real
//! HUD code. With `MACROHUD_RENDUS` set, it also lays out two sheets like the mock-up's
//! (`maquettes/variante-2.png`), on the same scenery (`maquettes/fonds/`):
//! `variante-2-egui.png` and `situations-egui.png`.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

use eframe::egui::epaint::{ImageData, Mesh, Primitive, RectShape, TextureId, Vertex};
use eframe::egui::{
    self, Color32, ColorImage, Context, Event, PointerButton, Pos2, RawInput, Rect, TextureFilter, TextureHandle,
    TextureOptions, TexturesDelta, Ui, UiBuilder, Vec2, ViewportId, ViewportInfo, pos2, vec2,
};

use super::{paint, set_open_macro, style, text, window_size};
use crate::contract::{
    HudAction, HudState, MacroId, MacroOptions, MacroStatus, MacroView, Message, MessageLevel, MouseButton, samples,
};

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
        let [a, b, c, d] =
            [self.texel(x0, y0), self.texel(x0 + 1, y0), self.texel(x0, y0 + 1), self.texel(x0 + 1, y0 + 1)];
        std::array::from_fn(|k| {
            let top = a[k] + (b[k] - a[k]) * fx;
            let bottom = c[k] + (d[k] - c[k]) * fx;
            top + (bottom - top) * fy
        })
    }
}

/// RGBA image, premultiplied alpha, gamma space, 0–1.
struct Image {
    width: usize,
    height: usize,
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
    fn new(width: usize, height: usize, background: Color32) -> Self {
        let fill = [background.r(), background.g(), background.b(), background.a()].map(|v| f32::from(v) / 255.0);
        Image { width, height, pixels: vec![fill; width * height] }
    }

    fn pixel(&self, x: usize, y: usize) -> Color32 {
        let [r, g, b, a] = self.pixels[y * self.width + x].map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
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
            self.triangle([a, b, c], ppp, texture, clip);
        }
    }

    fn triangle(&mut self, [a, b, c]: [Vertex; 3], ppp: f32, texture: &Texture, clip: [usize; 4]) {
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
        let colour = |v: &Vertex| [v.color.r(), v.color.g(), v.color.b(), v.color.a()].map(|x| f32::from(x) / 255.0);
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
                let source: [f32; 4] = std::array::from_fn(|k| (ca[k] * l0 + cb[k] * l1 + cc[k] * l2) * texel[k]);
                let target = &mut self.pixels[y * self.width + x];
                for k in 0..4 {
                    target[k] = source[k] + target[k] * (1.0 - source[3]);
                }
            }
        }
    }

    /// Writes the image as an 8-bit RGBA PNG (straight alpha).
    fn save_png(&self, path: &Path) {
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

/// Reads an 8-bit RGB or RGBA PNG (the scenery behind the HUD).
fn read_png(path: &Path) -> ColorImage {
    let file = BufReader::new(File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())));
    let mut reader = png::Decoder::new(file).read_info().expect("PNG lisible");
    let mut buffer = vec![0; reader.output_buffer_size().expect("taille du PNG")];
    let info = reader.next_frame(&mut buffer).expect("image PNG");
    let bytes = &buffer[..info.buffer_size()];
    let size = [info.width as usize, info.height as usize];
    match info.color_type {
        png::ColorType::Rgba => ColorImage::from_rgba_unmultiplied(size, bytes),
        png::ColorType::Rgb => ColorImage::from_rgb(size, bytes),
        other => panic!("{}: format PNG non pris en charge ({other:?})", path.display()),
    }
}

/// An egui context set up like the application's, plus the textures it uploaded.
struct Renderer {
    ctx: Context,
    textures: HashMap<TextureId, Texture>,
}

impl Renderer {
    fn new() -> Self {
        let ctx = Context::default();
        super::setup(&ctx);
        Renderer { ctx, textures: HashMap::new() }
    }

    fn input(size: Vec2, pixels_per_point: f32, events: Vec<Event>) -> RawInput {
        let mut input =
            RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..Default::default() };
        let info = ViewportInfo { native_pixels_per_point: Some(pixels_per_point), ..Default::default() };
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
                        let texture = Texture { size: image.size, pixels: image.pixels.clone(), linear };
                        self.textures.insert(*id, texture);
                    }
                    Some([x0, y0]) => {
                        let texture = self.textures.get_mut(id).expect("texture connue");
                        let width = texture.size[0];
                        for y in 0..image.size[1] {
                            for x in 0..image.size[0] {
                                texture.pixels[(y0 + y) * width + x0 + x] = image.pixels[y * image.size[0] + x];
                            }
                        }
                    }
                }
            }
        }
        delta.clear();
    }

    /// One egui pass with `events`.
    fn pass(&mut self, size: Vec2, events: Vec<Event>, ui_fn: &mut dyn FnMut(&mut Ui)) {
        let mut output = self.ctx.run_ui(Self::input(size, 1.0, events), |ui| ui_fn(ui));
        self.apply(&mut output.textures_delta);
    }

    /// Runs `ui_fn` on a screen of `size` points and rasterises it at `pixels_per_point`.
    fn render(
        &mut self,
        size: Vec2,
        pixels_per_point: f32,
        background: Color32,
        ui_fn: &mut dyn FnMut(&mut Ui),
    ) -> Image {
        // The first pass loads the glyphs; the second one is painted.
        let mut first = self.ctx.run_ui(Self::input(size, pixels_per_point, Vec::new()), |ui| ui_fn(ui));
        self.apply(&mut first.textures_delta);
        let mut output = self.ctx.run_ui(Self::input(size, pixels_per_point, Vec::new()), |ui| ui_fn(ui));
        self.apply(&mut output.textures_delta);
        let ppp = output.pixels_per_point;
        let primitives = self.ctx.tessellate(output.shapes, ppp);
        let mut image = Image::new((size.x * ppp).round() as usize, (size.y * ppp).round() as usize, background);
        for primitive in &primitives {
            if let Primitive::Mesh(mesh) = &primitive.primitive {
                image.draw(mesh, primitive.clip_rect, ppp, &self.textures);
            }
        }
        image
    }
}

/// Folder of the renders: `MACROHUD_RENDUS`, else `target/rendus-hud` of the crate.
fn output_dir() -> PathBuf {
    std::env::var_os("MACROHUD_RENDUS")
        .map_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("rendus-hud"), PathBuf::from)
}

// Views ---------------------------------------------------------------------------------------

/// A state to draw, with the settings the user unfolded.
#[derive(Clone)]
struct View {
    key: &'static str,
    state: HudState,
    open: Option<MacroId>,
}

impl View {
    fn size(&self) -> Vec2 {
        set_open_macro(self.open);
        window_size(&self.state)
    }

    fn paint(&self, ui: &mut Ui) -> Vec<HudAction> {
        set_open_macro(self.open);
        paint(ui, &self.state, true)
    }
}

const HOLD: MacroId = MacroId::HOLD_CLICK;

fn stopped() -> HudState {
    samples().remove(0).1
}

fn with(change: impl FnOnce(&mut HudState)) -> HudState {
    let mut state = stopped();
    change(&mut state);
    state
}

fn coming(id: &'static str, name: &'static str, hotkey: &str) -> MacroView {
    MacroView {
        id: MacroId(id),
        name,
        description: "Exemple de macro pas encore disponible.",
        status: MacroStatus::Stopped,
        hotkey: hotkey.into(),
        options: MacroOptions::HoldClick { button: MouseButton::Left },
        available: false,
    }
}

/// The six views of the mock-up (variant 2), with the contract's types.
fn mockup_views() -> Vec<View> {
    let view = |key, state, open| View { key, state, open };
    vec![
        view("a", stopped(), Some(HOLD)),
        view("b", with(|s| s.macros[0].status = MacroStatus::Active), None),
        view("c", with(|s| s.capturing = Some(HOLD)), None),
        view(
            "d",
            with(|s| {
                s.message =
                    Some(Message::new(MessageLevel::Warning, "Alt+Tab est déjà pris par Windows. F8 reste actif."))
            }),
            Some(HOLD),
        ),
        view(
            "e",
            with(|s| {
                s.macros.push(coming("clic-repete", "Clic répété", "F9"));
                s.macros.push(coming("touche-maintenue", "Touche maintenue", "F10"));
            }),
            None,
        ),
        view("f1", with(|s| s.collapsed = true), None),
        view(
            "f2",
            with(|s| {
                s.collapsed = true;
                s.macros[0].status = MacroStatus::Active;
            }),
            None,
        ),
    ]
}

/// The HUD alone, transparent around it, with the shadow of the mock-up.
fn render_alone(renderer: &mut Renderer, view: &View, ppp: f32) -> Image {
    let size = view.size();
    let [side, top, bottom] = style::SHADOW_MARGIN;
    let canvas = vec2(size.x + 2.0 * side, size.y + top + bottom);
    renderer.render(canvas, ppp, Color32::TRANSPARENT, &mut |ui| {
        let rect = Rect::from_min_size(pos2(side, top), size);
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            view.paint(ui);
        });
    })
}

// Tests ---------------------------------------------------------------------------------------

/// Same sizes as the mock-up (measured in `maquettes/index.html`), plus the 1 px black ring.
#[test]
fn the_views_have_the_mockup_sizes() {
    let expected = [("a", 290.0, 190.0), ("b", 290.0, 86.0), ("c", 290.0, 242.0), ("e", 290.0, 186.0)];
    let views = mockup_views();
    for (key, width, height) in expected {
        let view = views.iter().find(|v| v.key == key).expect("vue");
        assert_eq!(view.size(), vec2(width, height), "vue {key}");
    }
    // Folded pills: 158 × 34 and 151 × 34 in the mock-up (± the text's own width).
    for (key, width) in [("f1", 160.0), ("f2", 153.0)] {
        let size = views.iter().find(|v| v.key == key).expect("vue").size();
        assert_eq!(size.y, 36.0, "vue {key}");
        assert!((size.x - width).abs() <= 3.0, "vue {key} : {size:?}");
    }
    // The message sits under the hotkey line, 8 px apart (no action button: the contract does
    // not say which macro a message is about).
    let d = views.iter().find(|v| v.key == "d").expect("vue").size();
    assert!(d.y > 190.0 + 8.0 + 60.0, "{d:?}");
    set_open_macro(None);
}

/// Every situation of the contract fits the panel's width and draws without asking anything.
#[test]
fn every_situation_has_a_sensible_size() {
    for (name, state) in samples() {
        set_open_macro(None);
        let size = window_size(&state);
        if state.collapsed {
            let width = if state.message.is_some() { 290.0 } else { 160.0 };
            assert!(size.x <= width + 3.0, "{name} : {size:?}");
        } else {
            assert_eq!(size.x, 290.0, "{name}");
        }
        assert!(size.y >= 36.0 && size.y < 600.0, "{name} : {size:?}");
    }
}

/// Writes every view as PNG and checks pixels of the real rendering.
#[test]
fn renders_the_views_and_the_situations() {
    let mut renderer = Renderer::new();
    let dir = output_dir();
    let [side, top, _] = style::SHADOW_MARGIN.map(|v| v as usize);
    for view in mockup_views() {
        let image = render_alone(&mut renderer, &view, 1.0);
        image.save_png(&dir.join(format!("vue-{}.png", view.key)));
        render_alone(&mut renderer, &view, 2.0).save_png(&dir.join(format!("vue-{}@2x.png", view.key)));
        // Black ring, then the grey border, in the middle of the top edge.
        assert_eq!(image.pixel(side + 60, top), Color32::BLACK, "anneau, vue {}", view.key);
        assert_eq!(image.pixel(side + 60, top + 1), style::EDGE, "bord, vue {}", view.key);
        // Line of the macro: green when active, dark otherwise (above the text, left of the key).
        let row = image.pixel(side + 200, top + 2 + 32 + 3);
        match view.key {
            "b" => assert_eq!(row, style::ON),
            "a" | "c" | "d" | "e" => assert_eq!(row, style::ROW, "vue {}", view.key),
            _ => {}
        }
    }
    for (number, (name, state)) in samples().into_iter().enumerate() {
        let view = View { key: name, state, open: None };
        let image = render_alone(&mut renderer, &view, 1.0);
        image.save_png(&dir.join(format!("situation-{:02}.png", number + 1)));
        assert_eq!(image.pixel(side + 40, top), Color32::BLACK, "anneau, situation {name}");
    }
    set_open_macro(None);
}

// Interaction: real pointer events through egui ------------------------------------------------

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }
}

/// Pointer moves over `from`, presses, moves to `to`, releases; returns the HUD's actions.
fn gesture(view: &View, from: Pos2, to: Pos2) -> Vec<HudAction> {
    let mut renderer = Renderer::new();
    let mut actions = Vec::new();
    let steps = [
        vec![Event::PointerMoved(from)],
        vec![press(from, true)],
        vec![Event::PointerMoved(from.lerp(to, 0.5))],
        vec![Event::PointerMoved(to)],
        vec![press(to, false)],
        vec![],
    ];
    for events in steps {
        renderer.pass(vec2(400.0, 400.0), events, &mut |ui| {
            set_open_macro(view.open);
            actions.extend(paint(ui, &view.state, false));
        });
    }
    actions
}

fn click(view: &View, at: Pos2) -> Vec<HudAction> {
    gesture(view, at, at)
}

fn view_of(state: HudState, open: Option<MacroId>) -> View {
    View { key: "essai", state, open }
}

#[test]
fn clicks_ask_for_the_contract_actions() {
    let stopped = view_of(stopped(), None);
    // ▶ of the first line: ring 1 + border 1 + 14 px; the line starts under the 32 px title bar.
    let toggle = pos2(2.0 + 14.0 + 16.0, 2.0 + 32.0 + 24.0);
    assert_eq!(click(&stopped, toggle), vec![HudAction::Start(HOLD)]);
    let active = view_of(with(|s| s.macros[0].status = MacroStatus::Active), None);
    assert_eq!(click(&active, toggle), vec![HudAction::Stop(HOLD)]);

    // Title bar: « Quitter », « Replier », and dragging it moves the window.
    assert_eq!(click(&stopped, pos2(2.0 + 286.0 - 4.0 - 20.0, 17.0)), vec![HudAction::Quit]);
    assert_eq!(click(&stopped, pos2(2.0 + 286.0 - 4.0 - 64.0 - 6.0 - 30.0, 17.0)), vec![HudAction::SetCollapsed(true)]);
    assert_eq!(gesture(&stopped, pos2(150.0, 17.0), pos2(190.0, 30.0)), vec![HudAction::StartDrag]);

    // Settings: « Droit » in the selector, « Changer » on the hotkey line.
    let open = view_of(stopped.state.clone(), Some(HOLD));
    let settings_top = 2.0 + 32.0 + 50.0;
    let right = pos2(2.0 + 12.0 + 133.0, settings_top + 8.0 + 15.0 + 4.0 + 15.0);
    let held_right = MacroOptions::HoldClick { button: MouseButton::Right };
    assert_eq!(click(&open, right), vec![HudAction::SetOptions(HOLD, held_right)]);
    let change = pos2(2.0 + 12.0 + 266.0 - 20.0, settings_top + 67.0 + 13.0);
    assert_eq!(click(&open, change), vec![HudAction::StartCapture(HOLD)]);

    // « Annuler » while a hotkey is awaited.
    let capture = view_of(with(|s| s.capturing = Some(HOLD)), None);
    let cancel = pos2(2.0 + 12.0 + 266.0 - 11.0 - 20.0, settings_top + 67.0 + 11.0 + 24.0 + 6.0 + 13.0);
    assert_eq!(click(&capture, cancel), vec![HudAction::CancelCapture]);

    // The folded pill unfolds, and moves the window when dragged.
    let folded = view_of(with(|s| s.collapsed = true), None);
    let width = folded.size().x;
    assert_eq!(click(&folded, pos2(width - 2.0 - 4.0 - 12.0, 18.0)), vec![HudAction::SetCollapsed(false)]);
    assert_eq!(gesture(&folded, pos2(12.0, 18.0), pos2(60.0, 40.0)), vec![HudAction::StartDrag]);
    set_open_macro(None);
}

#[test]
fn the_arrow_unfolds_the_settings_without_asking_anything() {
    let closed = view_of(stopped(), None);
    let arrow = pos2(2.0 + 286.0 - 6.0 - 12.0, 2.0 + 32.0 + 24.0);
    let mut renderer = Renderer::new();
    let mut actions = Vec::new();
    for events in [vec![Event::PointerMoved(arrow)], vec![press(arrow, true)], vec![press(arrow, false)], vec![]] {
        renderer.pass(vec2(400.0, 400.0), events, &mut |ui| actions.extend(paint(ui, &closed.state, false)));
    }
    assert!(actions.is_empty());
    assert_eq!(window_size(&closed.state).y, 190.0, "réglages dépliés");
    set_open_macro(None);
    assert_eq!(window_size(&closed.state).y, 86.0);
}

#[test]
fn macros_to_come_cannot_be_clicked() {
    let view = mockup_views().into_iter().find(|v| v.key == "e").expect("vue e");
    for line in [1.0, 2.0] {
        let y = 2.0 + 32.0 + 50.0 * line + 24.0;
        for x in [32.0, 200.0, 270.0] {
            assert!(click(&view, pos2(x, y)).is_empty(), "ligne {line}, x {x}");
        }
    }
    set_open_macro(None);
}

/// A long version (local build: « 0.1.0-dev ») is cut before « Replier » instead of running under it.
#[test]
fn a_long_version_stays_clear_of_the_buttons() {
    let mut renderer = Renderer::new();
    let mut state = stopped();
    state.version = "0.1.0-dev-très-longue";
    let view = View { key: "version", state, open: None };
    let image = render_alone(&mut renderer, &view, 1.0);
    let [side, top, _] = style::SHADOW_MARGIN.map(|v| v as usize);
    // Between the version and « Replier » (4 px before the button), the bar keeps its own colour.
    let replier = 2 + 286 - 4 - 69 - 6 - 68;
    for x in replier - 3..replier {
        for y in 8..24 {
            assert_eq!(image.pixel(side + x, top + y), style::PANEL, "x {x}, y {y}");
        }
    }
    set_open_macro(None);
}

/// The state line never needs « … », whatever the button and the state.
#[test]
fn state_lines_fit_their_line() {
    for button in MouseButton::ALL {
        for status in [MacroStatus::Stopped, MacroStatus::Active] {
            let view = MacroView { status, options: MacroOptions::HoldClick { button }, ..stopped().macros.remove(0) };
            let (_, words, weight) = super::state_line(&view);
            // Room in a line: 286 px inside, minus ▶, the F8 key, the arrow and the gaps, minus the icon.
            let room = 218.0 - 8.0 - 54.0 - 13.0;
            assert!(text::width(&words, 12.0, weight) <= room, "« {words} » dépasse");
        }
    }
}

#[test]
fn a_long_hotkey_is_cut_not_the_panel() {
    let long = with(|s| s.macros[0].hotkey = "Ctrl+Alt+Maj+Pavé num. séparateur".into());
    set_open_macro(Some(HOLD));
    assert_eq!(window_size(&long).x, 290.0);
    let folded = with(|s| {
        s.macros[0].hotkey = "Ctrl+Alt+Maj+Pavé num. séparateur".into();
        s.collapsed = true;
    });
    assert!(window_size(&folded).x <= 2.0 + 1.0 + 6.0 + 12.0 + 6.0 + 15.0 + 50.0 + 8.0 + 84.0 + 6.0 + 29.0);
    set_open_macro(None);
}

#[test]
fn folded_messages_show_under_the_pill() {
    let state = with(|s| {
        s.collapsed = true;
        s.message = Some(Message::new(MessageLevel::Warning, "Sortez la souris du HUD avant d'appuyer sur F8."));
    });
    set_open_macro(None);
    let size = window_size(&state);
    assert_eq!(size.x, 290.0);
    assert!(size.y > 36.0 + 6.0 + 60.0, "{size:?}");
}

// Sheets laid out like the mock-up's -------------------------------------------------------------

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

// The page of the mock-ups (maquettes/outils/generer.mjs).
const PAGE: Color32 = rgb(0xe7e5e0);
const SECTION: Color32 = rgb(0xf7f6f3);
const SECTION_EDGE: Color32 = rgb(0xd3d0c9);
const INK: Color32 = rgb(0x1c1d20);
const INK_2: Color32 = rgb(0x43464c);
const INK_3: Color32 = rgb(0x686b72);
const SHEET_WIDTH: f32 = 1600.0;
const SECTION_LEFT: f32 = 40.0;
const SECTION_WIDTH: f32 = 1520.0;
const PADDING: f32 = 28.0;
const COLUMN: f32 = 474.0;
const GAP: f32 = 20.0;
const CAPTION: f32 = 32.0;
const HUD_GAP: f32 = 18.0;

struct Tile {
    letter: String,
    caption: String,
    scene: &'static str,
    views: Vec<View>,
}

fn scene_label(scene: &str) -> &'static str {
    match scene {
        "bureau" => "fond : bureau clair",
        "nuit" => "fond : jeu de nuit",
        _ => "fond : jeu en plein jour",
    }
}

/// UV rectangle showing the middle of a texture at the proportions of `size` (CSS « cover »).
fn cover(texture: &TextureHandle, size: Vec2) -> Rect {
    let [w, h] = texture.size().map(|v| v as f32);
    let scale = (size.x / w).max(size.y / h);
    let (u, v) = (size.x / scale / w, size.y / scale / h);
    Rect::from_min_max(pos2((1.0 - u) / 2.0, (1.0 - v) / 2.0), pos2((1.0 + u) / 2.0, (1.0 + v) / 2.0))
}

fn sheet(renderer: &mut Renderer, number: &str, title: &str, idea: &str, tiles: &[Tile]) -> Image {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("maquettes").join("fonds");
    let scenes: HashMap<&str, TextureHandle> = ["bureau", "nuit", "jour"]
        .into_iter()
        .map(|name| {
            let image = read_png(&dir.join(format!("{name}.png")));
            (name, renderer.ctx.load_texture(name, image, TextureOptions::LINEAR))
        })
        .collect();
    renderer.pass(vec2(SHEET_WIDTH, 100.0), Vec::new(), &mut |_| {});

    let sizes: Vec<Vec<Vec2>> = tiles.iter().map(|t| t.views.iter().map(View::size).collect()).collect();
    let content_height = |huds: &[Vec2]| huds.iter().map(|s| s.y).sum::<f32>() + HUD_GAP * (huds.len() as f32 - 1.0);
    let rows: Vec<f32> = sizes
        .chunks(3)
        .map(|row| row.iter().map(|huds| (content_height(huds) + 56.0).max(380.0)).fold(0.0, f32::max) + CAPTION)
        .collect();
    let text_left = SECTION_LEFT + 1.0 + PADDING + 56.0 + 16.0;
    let idea_width = SECTION_LEFT + SECTION_WIDTH - 1.0 - PADDING - text_left;
    let idea_lines = text::wrap(idea, 16.0, 400.0, idea_width);
    let idea_height = idea_lines.len() as f32 * 23.0;
    let head = (34.0 + 3.0 + idea_height).max(56.0);
    let section_top = 20.0;
    let grid_top = section_top + 1.0 + 26.0 + head + 20.0;
    let section_bottom = grid_top + rows.iter().sum::<f32>() + GAP * (rows.len() as f32 - 1.0) + 30.0 + 1.0;
    let height = (section_bottom + 20.0).ceil();

    renderer.render(vec2(SHEET_WIDTH, height), 1.0, PAGE, &mut |ui| {
        let p = ui.painter().clone();
        let section =
            Rect::from_min_max(pos2(SECTION_LEFT, section_top), pos2(SECTION_LEFT + SECTION_WIDTH, section_bottom));
        p.rect(section, 18, SECTION, egui::Stroke::new(1.0, SECTION_EDGE), egui::StrokeKind::Inside);
        // Heading: number, title, idea.
        let head_top = section_top + 1.0 + 26.0;
        let badge =
            Rect::from_min_size(pos2(SECTION_LEFT + 1.0 + PADDING, head_top + (head - 56.0) / 2.0), vec2(56.0, 56.0));
        p.rect_filled(badge, 14, INK);
        let digit = text::line(&p, number, 32.0, 800.0, Color32::WHITE, f32::INFINITY);
        text::centred(&p, &digit, badge.center().x, badge.top(), 56.0);
        let text_top = head_top + (head - (34.0 + 3.0 + idea_height)) / 2.0;
        text::draw(&p, title, 28.0, 800.0, INK, text_left, text_top, 34.0);
        for (k, line) in idea_lines.iter().enumerate() {
            text::draw(&p, line, 16.0, 400.0, INK_2, text_left, text_top + 37.0 + 23.0 * k as f32, 23.0);
        }
        // Tiles: caption, scenery, HUDs centred.
        let mut row_top = grid_top;
        for (r, row_height) in rows.iter().enumerate() {
            for c in 0..3 {
                let index = r * 3 + c;
                let Some(tile) = tiles.get(index) else { continue };
                let left = SECTION_LEFT + 1.0 + PADDING + c as f32 * (COLUMN + GAP);
                let letter = Rect::from_min_size(pos2(left, row_top), vec2(24.0, 24.0));
                p.rect_filled(letter, 6, INK);
                let glyph = text::line(&p, &tile.letter, 14.0, 700.0, Color32::WHITE, f32::INFINITY);
                text::centred(&p, &glyph, letter.center().x, letter.top(), 24.0);
                let label = text::line(&p, scene_label(tile.scene), 13.0, 500.0, INK_3, f32::INFINITY);
                text::put(&p, &label, left + COLUMN - label.size().x, row_top, 24.0);
                let room = COLUMN - 32.0 - label.size().x - 12.0;
                let caption = text::line(&p, &tile.caption, 15.0, 600.0, INK, room);
                text::put(&p, &caption, letter.right() + 8.0, row_top, 24.0);
                let stage = Rect::from_min_size(pos2(left, row_top + CAPTION), vec2(COLUMN, row_height - CAPTION));
                let texture = &scenes[tile.scene];
                p.add(
                    RectShape::filled(stage, 12, Color32::WHITE)
                        .with_texture(texture.id(), cover(texture, stage.size())),
                );
                let mut y = (stage.center().y - content_height(&sizes[index]) / 2.0).round();
                for (k, view) in tile.views.iter().enumerate() {
                    let size = sizes[index][k];
                    let rect = Rect::from_min_size(pos2((stage.center().x - size.x / 2.0).round(), y), size);
                    ui.scope_builder(UiBuilder::new().max_rect(rect).id_salt((index, k)), |ui| {
                        view.paint(ui);
                    });
                    y += size.y + HUD_GAP;
                }
            }
            row_top += row_height + GAP;
        }
    })
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// The comparison sheets, only when `MACROHUD_RENDUS` names where to write them (slow in debug).
#[test]
fn comparison_sheets() {
    if std::env::var_os("MACROHUD_RENDUS").is_none() {
        return;
    }
    let mut renderer = Renderer::new();
    let dir = output_dir();
    let scenes = ["bureau", "nuit", "jour"];

    let mut views = mockup_views().into_iter();
    let mut take = |n: usize| views.by_ref().take(n).collect::<Vec<_>>();
    let captions = [
        ("a", "Arrêtée"),
        ("b", "Active, bouton gauche"),
        ("c", "Capture d’un nouveau raccourci"),
        ("d", "Raccourci refusé"),
        ("e", "Trois macros, dont deux « À venir »"),
        ("f", "Repliée : arrêtée, puis active"),
    ];
    let tiles: Vec<Tile> = captions
        .iter()
        .enumerate()
        .map(|(k, (letter, caption))| Tile {
            letter: (*letter).to_owned(),
            caption: (*caption).to_owned(),
            scene: scenes[k % 3],
            views: take(if *letter == "f" { 2 } else { 1 }),
        })
        .collect();
    let idea = "Les six vues de la maquette, dessinées par le code du HUD branché sur le contrat \
                (src/hud, egui 0.37, police Inter) et rendues sans écran, à taille réelle. À comparer avec \
                maquettes/variante-2.png.";
    let image = sheet(&mut renderer, "2", "Variante 2 — Liste compacte : rendu réel egui", idea, &tiles);
    image.save_png(&dir.join("variante-2-egui.png"));

    // The situations of the contract, then the two cases the engine explains with a message.
    let mut situations = samples();
    situations.push((
        "après « Activer » : délai de 3 s",
        with(|s| {
            s.macros[0].status = MacroStatus::Active;
            s.message = Some(Message::new(
                MessageLevel::Info,
                "Le bouton gauche sera enfoncé dans 3 secondes : placez la souris là où il doit agir.",
            ));
        }),
    ));
    situations.push((
        "F8 avec la souris sur le HUD replié",
        with(|s| {
            s.collapsed = true;
            s.message = Some(Message::new(MessageLevel::Warning, "Sortez la souris du HUD avant d'appuyer sur F8."));
        }),
    ));
    let tiles: Vec<Tile> = situations
        .into_iter()
        .enumerate()
        .map(|(k, (name, state))| Tile {
            letter: (k + 1).to_string(),
            caption: capitalised(name),
            scene: scenes[k % 3],
            views: vec![View { key: name, state, open: None }],
        })
        .collect();
    let idea = "Chaque situation de contract::samples(), telle que le HUD la dessine, puis les deux cas que le \
                moteur explique par un message : le délai de 3 s après « Activer », et F8 refusé avec la souris \
                sur le HUD (ici replié).";
    let image = sheet(&mut renderer, "+", "Toutes les situations du contrat", idea, &tiles);
    image.save_png(&dir.join("situations-egui.png"));
    set_open_macro(None);
}
