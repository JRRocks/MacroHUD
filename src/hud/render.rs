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

use super::icons::Icon;
use super::parts::KeyLook;
use super::{paint, parts, set_open_macro, style, text, window_size};
use crate::contract::{
    BadgeColor, ClickRate, ExternalLine, HudAction, HudState, MacroId, MacroOptions, MacroStatus, MacroView, Message,
    MessageLevel, MouseButton, samples,
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
        self.render_pointed(size, pixels_per_point, background, None, ui_fn)
    }

    /// The same, the mouse pointer resting at `pointer` (hover effects show).
    fn render_pointed(
        &mut self,
        size: Vec2,
        pixels_per_point: f32,
        background: Color32,
        pointer: Option<Pos2>,
        ui_fn: &mut dyn FnMut(&mut Ui),
    ) -> Image {
        let events = || pointer.map(|pos| vec![Event::PointerMoved(pos)]).unwrap_or_default();
        // The first pass loads the glyphs (and places the pointer); the second one is painted.
        let mut first = self.ctx.run_ui(Self::input(size, pixels_per_point, events()), |ui| ui_fn(ui));
        self.apply(&mut first.textures_delta);
        let mut output = self.ctx.run_ui(Self::input(size, pixels_per_point, events()), |ui| ui_fn(ui));
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
        suspended: false,
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
                    Some(Message::hotkey_refused(HOLD, "Alt+Tab", "Alt+Tab est déjà pris par Windows. F8 reste actif."))
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
    render_pointed(renderer, view, ppp, None)
}

/// The same, with the mouse pointer drawn at `spot` (from the top left of the HUD), hovering there.
fn render_pointed(renderer: &mut Renderer, view: &View, ppp: f32, spot: Option<Pos2>) -> Image {
    let size = view.size();
    let [side, top, bottom] = style::SHADOW_MARGIN;
    let canvas = vec2(size.x + 2.0 * side, size.y + top + bottom);
    let origin = pos2(side, top);
    let pointer = spot.map(|spot| origin + spot.to_vec2());
    renderer.render_pointed(canvas, ppp, Color32::TRANSPARENT, pointer, &mut |ui| {
        let rect = Rect::from_min_size(origin, size);
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            view.paint(ui);
        });
        if let Some(tip) = pointer {
            paint_pointer(ui.painter(), tip);
        }
    })
}

/// The arrow of the system's pointer, its tip at `tip`, as on the « Multi-clic » preview: not part
/// of the HUD, it shows where the mouse is in the renders.
fn paint_pointer(p: &egui::Painter, tip: Pos2) {
    let at = |x: f32, y: f32| tip + vec2(x, y);
    // Head: a triangle whose lower edge runs from (0, 16) to (11.3, 11.3). Tail: a bar leaving that
    // edge between A and B, going down to the right. Filled apart (egui fills convex shapes only),
    // the tail reaching a little into the head so that no seam shows, then outlined as one.
    let (a, b) = (at(3.6, 14.5), at(6.8, 13.17));
    let down = vec2(0.419, 0.908);
    let (c, d) = (b + 6.0 * down, a + 6.0 * down);
    let head = vec![at(0.0, 0.0), at(0.0, 16.0), at(11.3, 11.3)];
    let tail = vec![a - 1.5 * down, b - 1.5 * down, c, d];
    p.add(egui::Shape::convex_polygon(head, Color32::WHITE, egui::Stroke::NONE));
    p.add(egui::Shape::convex_polygon(tail, Color32::WHITE, egui::Stroke::NONE));
    let outline = vec![at(0.0, 0.0), at(0.0, 16.0), a, d, c, b, at(11.3, 11.3)];
    p.add(egui::epaint::PathShape::closed_line(outline, egui::Stroke::new(1.0, Color32::BLACK)));
}

// « Multi-clic » ------------------------------------------------------------------------------

const MULTI: MacroId = MacroId::MULTI_CLICK;

/// The situation of the contract called `name`.
fn sample(name: &str) -> HudState {
    samples().into_iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("situation « {name} »")).1
}

/// The two macros, « Multi-clic » `status`d, its clicks waiting or not.
fn two_macros(status: MacroStatus, suspended: bool) -> HudState {
    let mut state = sample("deux macros");
    state.macros[1].status = status;
    state.macros[1].suspended = suspended;
    state
}

/// The slider's line in the open settings of « Multi-clic » (second macro), from the top left of
/// the HUD: ring, border, title bar, two lines, then « Bouton cliqué », its selector, « Rapidité ».
fn slider_line() -> Rect {
    let top = 2.0 + 32.0 + 2.0 * 50.0 + 8.0 + 15.0 + 4.0 + 30.0 + 10.0 + 15.0 + 4.0;
    Rect::from_min_size(pos2(2.0 + 12.0, top), vec2(266.0, parts::SLIDER_HEIGHT))
}

/// The three views of track A of the preview (`maquettes/multi-clic/a-*@2x.png`), drawn by the real
/// HUD: settings open with the macro stopped; active; active, the mouse on the thumb (clicks wait).
/// The last one has the pointer of the preview, 3 px right and down of the thumb's centre.
fn multi_click_views() -> Vec<(View, Option<Pos2>)> {
    let thumb = parts::slider_thumb(slider_line(), parts::Range::click_rate(), 10);
    let view = |key, state| View { key, state, open: Some(MULTI) };
    vec![
        (view("multi-1-reglages", two_macros(MacroStatus::Stopped, false)), None),
        (view("multi-2-active", two_macros(MacroStatus::Active, false)), None),
        (view("multi-3-en-pause", two_macros(MacroStatus::Active, true)), Some(thumb + vec2(3.0, 3.0))),
    ]
}

// Tests ---------------------------------------------------------------------------------------

/// Same sizes as the mock-up (measured in `maquettes/index.html`), plus the 1 px black ring.
#[test]
fn the_views_have_the_mockup_sizes() {
    let expected =
        [("a", 290.0, 190.0), ("b", 290.0, 86.0), ("c", 290.0, 242.0), ("d", 290.0, 311.0), ("e", 290.0, 186.0)];
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
    set_open_macro(None);
}

/// Every situation of the contract fits the panel's width and draws without asking anything.
#[test]
fn every_situation_has_a_sensible_size() {
    for (name, state) in samples() {
        set_open_macro(None);
        let size = window_size(&state);
        if state.collapsed {
            // The pill of MacroHUD's macros is small; that of an external macro carries its state
            // and step pill, never wider than the HUD (D-54).
            let external = state.external_macros.iter().any(|m| m.status == MacroStatus::Active);
            let width = if state.message.is_some() || external { 290.0 } else { 160.0 };
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

/// In the window (no shadow), the HUD paints nothing outside its rounded shape: the corner pixels
/// stay fully transparent. A line or a darkening around the window comes from the system (D-16).
#[test]
fn nothing_is_painted_outside_the_rounded_shape() {
    let mut renderer = Renderer::new();
    for view in mockup_views() {
        let size = view.size();
        let image = renderer.render(size, 1.0, Color32::TRANSPARENT, &mut |ui| {
            set_open_macro(view.open);
            paint(ui, &view.state, false);
        });
        let (w, h) = (image.width - 1, image.height - 1);
        for (x, y) in [(0, 0), (w, 0), (0, h), (w, h), (1, 1), (w - 1, 1), (1, h - 1), (w - 1, h - 1)] {
            assert_eq!(image.pixel(x, y).a(), 0, "vue {}, pixel ({x}, {y})", view.key);
        }
        // The black ring of the mock-up is the outermost pixel row, opaque.
        assert_eq!(image.pixel(image.width / 2, 0), Color32::BLACK, "vue {}", view.key);
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

/// « Raccourci refusé » (D-16): « Choisir une autre touche » asks a new hotkey for the macro the
/// message concerns; folded, it also unfolds the HUD so that « Appuie sur une touche… » shows.
#[test]
fn choosing_another_key_after_a_refusal() {
    let refused = |collapsed| {
        with(|s| {
            s.collapsed = collapsed;
            s.message =
                Some(Message::hotkey_refused(HOLD, "Alt+Tab", "Alt+Tab est déjà pris par Windows. F8 reste actif."))
        })
    };
    // Unfolded: the settings stay open under the line, the box sits under the hotkey line.
    let unfolded = view_of(refused(false), None);
    assert_eq!(unfolded.size(), vec2(290.0, 311.0), "réglages ouverts, comme la maquette");
    let box_top = 2.0 + 32.0 + 50.0 + 8.0 + 15.0 + 4.0 + 30.0 + 10.0 + 26.0 + 8.0;
    let button = pos2(2.0 + 12.0 + 11.0 + 18.0 + 10.0 + 60.0, box_top + 11.0 + 18.0 + 3.0 + 2.0 * 16.0 + 8.0 + 14.0);
    assert_eq!(click(&unfolded, button), vec![HudAction::StartCapture(HOLD)]);
    // Folded: under the pill.
    let folded = view_of(refused(true), None);
    let box_top = 36.0 + 6.0 + 1.0;
    let button = pos2(1.0 + 11.0 + 18.0 + 10.0 + 60.0, box_top + 11.0 + 18.0 + 3.0 + 2.0 * 16.0 + 8.0 + 14.0);
    assert_eq!(click(&folded, button), vec![HudAction::SetCollapsed(false), HudAction::StartCapture(HOLD)]);
    // Another kind of message has no button there.
    let general = view_of(
        with(|s| {
            s.collapsed = true;
            s.message = Some(
                Message::new(MessageLevel::Warning, "Alt+Tab est déjà pris par Windows. F8 reste actif.")
                    .concerning(HOLD),
            )
        }),
        None,
    );
    assert!(click(&general, button).is_empty());
    set_open_macro(None);
}

/// A message about a macro sits in its open settings; a general one below the macros.
#[test]
fn messages_sit_with_their_macro() {
    let text = "Raccourci inchangé : F8.";
    let about = with(|s| s.message = Some(Message::new(MessageLevel::Info, text).concerning(HOLD)));
    let general = with(|s| s.message = Some(Message::new(MessageLevel::Info, text)));
    set_open_macro(Some(HOLD));
    // In the settings: 8 px above the box; below the macros: its own block (8 + box + 10 + line).
    assert_eq!(window_size(&general).y - window_size(&about).y, 11.0);
    set_open_macro(None);
    assert_eq!(window_size(&general).y, window_size(&about).y, "réglages fermés : en bas dans les deux cas");
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

/// The state line never needs « … », whatever the macro, the button, the speed and the state.
#[test]
fn state_lines_fit_their_line() {
    // Room in a line: 286 px inside, minus ▶, the F8 (or F9) key, the arrow and the gaps, the icon.
    let room = 218.0 - 8.0 - 54.0 - 13.0;
    let multi = sample("deux macros").macros.remove(1);
    for button in MouseButton::ALL {
        let mut options = vec![MacroOptions::HoldClick { button }];
        for rate in ClickRate::MIN.per_second()..=ClickRate::MAX.per_second() {
            options.push(MacroOptions::MultiClick { button, rate: ClickRate::new(rate) });
        }
        for options in options {
            for (status, suspended) in
                [(MacroStatus::Stopped, false), (MacroStatus::Active, false), (MacroStatus::Active, true)]
            {
                let base = if matches!(options, MacroOptions::HoldClick { .. }) {
                    stopped().macros.remove(0)
                } else {
                    multi.clone()
                };
                let view = MacroView { status, options, suspended, ..base };
                let (_, words, weight) = super::state_line(&view);
                assert!(text::width(&words, 12.0, weight) <= room, "« {words} » dépasse");
            }
        }
    }
}

/// « Multi-clic » reads as on track A of its preview (D-26).
#[test]
fn multi_click_lines_read_as_chosen() {
    let line = |status, suspended, options| {
        let mut view = sample("deux macros").macros.remove(1);
        (view.status, view.suspended, view.options) = (status, suspended, options);
        super::state_line(&view)
    };
    let left = MacroOptions::MultiClick { button: MouseButton::Left, rate: ClickRate::DEFAULT };
    let right = MacroOptions::MultiClick { button: MouseButton::Right, rate: ClickRate::MIN };
    assert_eq!(
        line(MacroStatus::Stopped, false, left),
        (Some(Icon::Stop), "Arrêtée · gauche, 10/s".into(), style::REGULAR)
    );
    assert_eq!(
        line(MacroStatus::Active, false, left),
        (Some(Icon::Dot), "Active · gauche, 10/s".into(), style::SEMIBOLD)
    );
    assert_eq!(
        line(MacroStatus::Active, true, left),
        (Some(Icon::Pause), "En pause · souris ici".into(), style::SEMIBOLD)
    );
    assert_eq!(line(MacroStatus::Stopped, false, right).1, "Arrêtée · droit, 1/s");
    // The pause changes neither the size of the HUD nor that of the folded pill.
    for open in [None, Some(MULTI)] {
        set_open_macro(open);
        let active = window_size(&two_macros(MacroStatus::Active, false));
        assert_eq!(window_size(&two_macros(MacroStatus::Active, true)), active);
    }
    let folded = |suspended| {
        let mut state = two_macros(MacroStatus::Active, suspended);
        state.collapsed = true;
        window_size(&state)
    };
    assert_eq!(folded(true), folded(false));
    set_open_macro(None);
}

/// The settings of « Multi-clic »: as tall as the preview's (293 px with the ring), 54 px more than
/// those of « Maintenir le clic » (the speed: title, slider, gap).
#[test]
fn multi_click_settings_have_the_preview_size() {
    set_open_macro(Some(MULTI));
    assert_eq!(window_size(&two_macros(MacroStatus::Stopped, false)), vec2(290.0, 293.0));
    set_open_macro(Some(HOLD));
    assert_eq!(window_size(&two_macros(MacroStatus::Stopped, false)).y, 293.0 - 53.0);
    set_open_macro(None);
    assert_eq!(window_size(&two_macros(MacroStatus::Stopped, false)).y, 2.0 + 32.0 + 100.0 + 2.0);
}

/// The slider asks for the new speed at once, stopped or running, mouse on the HUD or not (D-27):
/// a click on a notch, a drag, nothing when the speed does not change.
#[test]
fn the_speed_slider_asks_for_the_new_speed() {
    let line = slider_line();
    let at = |rate: u32| parts::slider_thumb(line, parts::Range::click_rate(), rate.into());
    let set = |rate: u32| {
        HudAction::SetOptions(MULTI, MacroOptions::MultiClick { button: MouseButton::Left, rate: ClickRate::new(rate) })
    };
    for (status, suspended) in
        [(MacroStatus::Stopped, false), (MacroStatus::Active, false), (MacroStatus::Active, true)]
    {
        let view = view_of(two_macros(status, suspended), Some(MULTI));
        assert_eq!(click(&view, at(3)), vec![set(3)], "cran 3");
        assert_eq!(click(&view, at(3) + vec2(4.0, 5.0)), vec![set(3)], "près du cran 3");
        assert_eq!(click(&view, pos2(line.left() + 22.0, line.center().y)), vec![set(1)], "début de la piste");
        assert_eq!(click(&view, pos2(line.right() - 22.0, line.center().y)), vec![set(15)], "fin de la piste");
        assert!(click(&view, at(10)).is_empty(), "la rapidité ne change pas");
        let dragged = gesture(&view, at(10), at(15));
        assert_eq!(dragged.last(), Some(&set(15)), "glissé jusqu'à 15 : {dragged:?}");
        assert!(dragged.iter().all(|a| matches!(a, HudAction::SetOptions(MULTI, _))), "{dragged:?}");
    }
    // The bounds and the track are not clickable outside the track.
    let view = view_of(two_macros(MacroStatus::Stopped, false), Some(MULTI));
    assert!(click(&view, pos2(line.left() + 3.0, line.center().y)).is_empty(), "« 1 »");
    set_open_macro(None);
}

/// With the keyboard focus on the slider, the arrows move the speed by one.
#[test]
fn the_arrows_move_the_speed() {
    let state = two_macros(MacroStatus::Active, false);
    let mut renderer = Renderer::new();
    let mut actions = Vec::new();
    let key = |key| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
    let steps = [vec![], vec![], vec![key(egui::Key::ArrowRight)], vec![], vec![key(egui::Key::ArrowLeft)], vec![]];
    for (k, events) in steps.into_iter().enumerate() {
        renderer.pass(vec2(400.0, 400.0), events, &mut |ui| {
            set_open_macro(Some(MULTI));
            if k == 0 {
                let slider = ui.make_persistent_id("macrohud").with(1usize).with("rapidite");
                ui.memory_mut(|m| m.request_focus(slider));
            }
            actions.extend(paint(ui, &state, false));
        });
    }
    let set = |rate| {
        HudAction::SetOptions(MULTI, MacroOptions::MultiClick { button: MouseButton::Left, rate: ClickRate::new(rate) })
    };
    assert_eq!(actions, vec![set(11), set(9)]);
    set_open_macro(None);
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
        s.message =
            Some(Message::new(MessageLevel::Warning, "Sors la souris du HUD avant d'appuyer sur F8.").concerning(HOLD));
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

/// « Multi-clic » drawn by the real HUD: the three views of track A of its preview, and pixels that
/// tell them apart (the pause sign, the thumb). With `MACROHUD_RENDUS`, also at 200 % for the page
/// `maquettes/multi-clic/resultat.html`, with every situation of the contract where it shows and the
/// pill folded while its clicks wait.
#[test]
fn renders_multi_click() {
    let mut renderer = Renderer::new();
    let dir = output_dir();
    let [side, top, _] = style::SHADOW_MARGIN.map(|v| v as usize);
    let big = std::env::var_os("MACROHUD_RENDUS").is_some();
    for (view, spot) in multi_click_views() {
        let image = render_pointed(&mut renderer, &view, 1.0, spot);
        image.save_png(&dir.join(format!("{}.png", view.key)));
        if big {
            render_pointed(&mut renderer, &view, 2.0, spot).save_png(&dir.join(format!("{}@2x.png", view.key)));
        }
        // The state line of « Multi-clic » (second line): a bar of « ❚❚ », and between the bars.
        let (bar, gap) = (image.pixel(side + 57, top + 118), image.pixel(side + 60, top + 118));
        match view.key {
            "multi-3-en-pause" => assert_eq!((bar, gap), (style::ON_INK, style::ON)),
            "multi-2-active" => assert_eq!(gap, style::ON_INK, "le point « Active »"),
            _ => assert_eq!(gap, style::TEXT_2, "le carré « Arrêtée »"),
        }
        // The thumb, on 10 clicks per second.
        let thumb = parts::slider_thumb(slider_line(), parts::Range::click_rate(), 10);
        assert_eq!(image.pixel(side + thumb.x as usize - 3, top + thumb.y as usize - 3), style::KEY, "{}", view.key);
    }
    if !big {
        return;
    }
    for (number, (name, state)) in samples().into_iter().enumerate() {
        // The situations of « Multi-clic » (those with an external macro have their views below).
        if state.macros.iter().any(|m| m.id == MULTI) && state.external_macros.is_empty() {
            let view = View { key: name, state, open: None };
            render_alone(&mut renderer, &view, 2.0).save_png(&dir.join(format!("situation-{:02}@2x.png", number + 1)));
        }
    }
    let mut folded = two_macros(MacroStatus::Active, true);
    folded.collapsed = true;
    let view = View { key: "multi-4-replie-en-pause", state: folded, open: None };
    render_alone(&mut renderer, &view, 2.0).save_png(&dir.join("multi-4-replie-en-pause@2x.png"));
    set_open_macro(None);
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
        "après « Activer » : décompte de 3 s",
        with(|s| {
            s.message = Some(
                Message::new(
                    MessageLevel::Info,
                    "Activation dans 3 s : place la souris là où le clic doit rester enfoncé.",
                )
                .concerning(HOLD),
            );
        }),
    ));
    situations.push((
        "F8 avec la souris sur le HUD replié",
        with(|s| {
            s.collapsed = true;
            s.message = Some(
                Message::new(MessageLevel::Warning, "Sors la souris du HUD avant d'appuyer sur F8.").concerning(HOLD),
            );
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
                moteur explique par un message : le décompte de 3 s après « Activer », et F8 refusé avec la souris \
                sur le HUD (ici replié).";
    let image = sheet(&mut renderer, "+", "Toutes les situations du contrat", idea, &tiles);
    image.save_png(&dir.join("situations-egui.png"));
    set_open_macro(None);
}

// External macros (D-32, D-37) -----------------------------------------------------------------

const EXAMPLE: MacroId = MacroId("exemple");

/// The views of the external macro's preview (`maquettes/macro-externe/apercu.html`, D-37), drawn
/// by the real HUD with the preview's example texts (situations of the contract), then the cases
/// the preview does not draw.
fn external_views() -> Vec<View> {
    let view = |key, name, open| View { key, state: sample(name), open };
    vec![
        view("externe-1-reglages", "trois macros, macro externe arrêtée", Some(EXAMPLE)),
        view("externe-2-active", "macro externe active, pastille ambre", None),
        view("externe-3-pastille-verte", "macro externe active, pastille verte", None),
        view("externe-3-pastille-grise", "macro externe active, pastille grise", None),
        view("externe-3-pastille-bleue", "macro externe active, pastille bleue", None),
        view("externe-4-en-attente", "macro externe en attente", None),
        view("externe-5-terminee", "macro externe terminée", None),
        view("externe-6-replie", "replié, macro externe active", None),
        view("externe-7-alerte", "macro externe, alerte de son script", None),
        view("externe-8-ne-demarre-pas", "macro externe qui ne peut pas démarrer", None),
        view("externe-souris-sur-le-hud", "macro externe suspendue (souris sur le HUD)", None),
        view("externe-avant-premier-etat", "macro externe active, avant son premier état", None),
        view("externe-sans-raccourci", "macro externe sans raccourci ni réglage", Some(EXAMPLE)),
    ]
}

/// The line of the external macro: the third, under the title bar and the two lines of MacroHUD.
const EXTERNAL_ROW_TOP: f32 = 2.0 + 32.0 + 2.0 * 50.0;

/// The external macro's line reads as on its preview.
#[test]
fn external_lines_read_as_the_preview() {
    let line = |name: &str| super::external_state_line(&sample(name).external_macros[0]);
    let words = |name: &str| line(name).1;
    assert_eq!(
        line("trois macros, macro externe arrêtée"),
        (Some(Icon::Stop), "Arrêtée · limite 100 clics".into(), style::REGULAR)
    );
    assert_eq!(
        line("macro externe active, pastille ambre"),
        (Some(Icon::Dot), "Active · 12/100".into(), style::SEMIBOLD)
    );
    assert_eq!(words("macro externe active, avant son premier état"), "Active");
    assert_eq!(
        line("macro externe en attente"),
        (Some(Icon::Pause), "En pause · attend sa fenêtre".into(), style::SEMIBOLD)
    );
    assert_eq!(line("macro externe suspendue (souris sur le HUD)").1, "En pause · souris ici");
    assert_eq!(line("macro externe terminée"), (Some(Icon::Check), "Terminée · 100/100 clics".into(), style::REGULAR));
    assert_eq!(words("macro externe qui ne peut pas démarrer"), "Arrêtée");
    // The pill: the active macro's, the words of its state on it, never wider than the HUD (D-54).
    let folded = sample("replié, macro externe active");
    assert_eq!(super::chip_state(super::main_item(&super::items(&folded)).expect("macro")).1, "Active 12/100");
    set_open_macro(None);
    assert!(window_size(&folded).x <= 290.0, "{:?}", window_size(&folded));
}

/// Settings of the external macro: its slider with the value above it, the hotkey, the help line
/// on three lines (the HUD is narrower than the preview's drawing); the HUD grows by one line per external macro, keeping its width.
#[test]
fn external_settings_have_their_size() {
    let stopped = sample("trois macros, macro externe arrêtée");
    assert_eq!(super::info_lines(&stopped.external_macros[0]).len(), 3);
    set_open_macro(Some(EXAMPLE));
    let settings = 8.0 + (15.0 + 4.0 + 24.0 + 10.0) + 26.0 + (10.0 + 3.0 * 16.0) + 10.0 + 1.0;
    assert_eq!(window_size(&stopped), vec2(290.0, 2.0 + 32.0 + 3.0 * 50.0 + settings + 2.0));
    set_open_macro(None);
    assert_eq!(window_size(&stopped), vec2(290.0, 2.0 + 32.0 + 3.0 * 50.0 + 2.0));
    // The amber box: under the line, the whole title then the text, as one paragraph.
    let alert = sample("macro externe, alerte de son script");
    let rows = parts::AlertBox::new(alert.external_macros[0].alert.as_ref().expect("encadré"), 266.0).rows();
    assert_eq!(
        rows.join(" "),
        "La fenêtre visée est en plein écran. Passe-la en fenêtre : en plein écran, la macro ne voit pas l'écran."
    );
    assert!(rows.len() >= 2, "{rows:?}");
}

/// The sliders of an external macro ask for the value at once, on its steps, stopped or running
/// (like the speed of « Multi-clic », D-27); nothing when the value does not change.
#[test]
fn external_sliders_ask_for_their_value() {
    let line =
        Rect::from_min_size(pos2(14.0, EXTERNAL_ROW_TOP + 50.0 + 8.0 + 15.0 + 4.0), vec2(266.0, parts::SLIDER_HEIGHT));
    let range = parts::Range { min: 10, max: 500, step: 10 };
    let at = |value: i64| parts::slider_thumb(line, range, value);
    let set = |value: i64| HudAction::SetSetting(EXAMPLE, "limite".into(), value);
    for active in [false, true] {
        let mut state = sample("trois macros, macro externe arrêtée");
        if active {
            state.external_macros[0].status = MacroStatus::Active;
            state.external_macros[0].line = ExternalLine::Active(Some("12/100".into()));
        }
        let view = view_of(state, Some(EXAMPLE));
        assert_eq!(click(&view, at(250)), vec![set(250)], "cran 250");
        assert_eq!(click(&view, at(250) + vec2(2.0, 4.0)), vec![set(250)], "près du cran 250");
        assert_eq!(click(&view, at(500)), vec![set(500)], "fin de la piste");
        assert!(click(&view, at(100)).is_empty(), "la valeur ne change pas");
        let dragged = gesture(&view, at(100), at(300));
        assert_eq!(dragged.last(), Some(&set(300)), "{dragged:?}");
    }
    set_open_macro(None);
}

/// The views of the preview drawn by the real HUD, with pixels that tell them apart; at 200 % in
/// `maquettes/rendu-egui/` with `MACROHUD_RENDUS`.
#[test]
fn renders_external_macros() {
    let mut renderer = Renderer::new();
    let dir = output_dir();
    let [side, top, _] = style::SHADOW_MARGIN.map(|v| v as usize);
    let big = std::env::var_os("MACROHUD_RENDUS").is_some();
    for view in external_views() {
        let image = render_alone(&mut renderer, &view, 1.0);
        if big {
            render_alone(&mut renderer, &view, 2.0).save_png(&dir.join(format!("{}@2x.png", view.key)));
        }
        if view.state.collapsed {
            continue;
        }
        // The left stripe of the external macro's line: blue once « Terminée », dark green active.
        let stripe = image.pixel(side + 3, top + EXTERNAL_ROW_TOP as usize + 20);
        match view.key {
            "externe-5-terminee" => assert_eq!(stripe, style::INFO, "liseré bleu"),
            "externe-1-reglages" | "externe-7-alerte" | "externe-8-ne-demarre-pas" | "externe-sans-raccourci" => {
                assert_eq!(stripe, style::STRIPE, "{}", view.key);
            }
            _ => assert_eq!(stripe, style::ON_INK, "{}", view.key),
        }
        // The step pill, on the name line against the key (D-38): its colour near its top, and
        // nothing of it lower down, where its right end stood before (15 to 35 px into the line).
        let external = &view.state.external_macros[0];
        if let Some(badge) = external.badge.as_ref().filter(|_| external.status == MacroStatus::Active) {
            let key = parts::keycap_width("F10", KeyLook::OnGreen, 84.0);
            let right = 2.0 + 286.0 - 6.0 - 24.0 - 8.0 - key - 8.0;
            let pill_left = right - parts::pill_width(&badge.text);
            let row = top + EXTERNAL_ROW_TOP as usize;
            let below = image.pixel(side + right as usize - 4, row + 32);
            assert_eq!(below, style::ON, "{} : plus de pastille en bas", view.key);
            // Its new top (6 px into the line), above its text, in the middle of its left end.
            let fill = image.pixel(side + pill_left as usize + 10, row + 8);
            let expected = match badge.color {
                BadgeColor::Amber => style::WARN,
                BadgeColor::Blue => style::INFO,
                BadgeColor::Green | BadgeColor::Grey => style::ON_INK,
            };
            assert_eq!(fill, expected, "{}", view.key);
        }
    }
    set_open_macro(None);
}

/// « ▶ » and « ■ », which Inter lacks, keep their room in a wrapped text and come back as the
/// HUD's icons (D-56); the rows of an amber box hold the whole title and text.
#[test]
fn signs_keep_their_room() {
    let lines = text::wrap(&text::with_sign_room("F10 ou ▶ l'arrêtent, ■ aussi."), 12.0, style::REGULAR, 400.0);
    assert_eq!(lines.len(), 1);
    let mut renderer = Renderer::new();
    let mut spots = Vec::new();
    renderer.pass(vec2(400.0, 100.0), Vec::new(), &mut |ui| {
        let galley = text::line(ui.painter(), &lines[0], 12.0, style::REGULAR, Color32::WHITE, 400.0);
        spots = text::sign_spots(&galley);
    });
    assert_eq!(spots.iter().map(|(sign, _)| *sign).collect::<String>(), "▶■");
    let before = text::width("F10 ou ", 12.0, style::REGULAR);
    assert!((spots[0].1 - before - 4.7).abs() < 2.0, "{spots:?}");
    // Cut where it may be: the sign stays whole at the end of a row.
    let narrow = text::wrap(&text::with_sign_room("Arrête avec ▶ puis relance."), 12.0, style::REGULAR, 80.0);
    assert!(narrow.iter().any(|row| row.contains('\u{A0}')), "{narrow:?}");
    assert_eq!(narrow.concat().matches('\u{A0}').count(), 3, "{narrow:?}");
}

/// The step pill on the name line (D-38, D-39): with F10 and each pill of the preview, its right
/// edge 8 px from the key, 20 px high from 6 to 26 px into the line (around the name's line box,
/// above the state line), and « Active · 12/100 » whole; a long name stops before the pill, cut.
/// Without a pill nothing moves, and the folded pill keeps its width.
#[test]
fn the_pill_sits_on_the_name_line() {
    let content = Rect::from_min_size(pos2(2.0, EXTERNAL_ROW_TOP), vec2(286.0, 49.0));
    let pills = [
        ("macro externe active, pastille ambre", "Attente"),
        ("macro externe active, pastille verte", "Clic !"),
        ("macro externe active, pastille grise", "Pause 8 s"),
        ("macro externe active, pastille bleue", "Relance"),
    ];
    for (name, text) in pills {
        let state = sample(name);
        let view = &state.external_macros[0];
        assert_eq!((view.hotkey.as_str(), view.badge.as_ref().map(|b| b.text.as_str())), ("F10", Some(text)));
        let geometry = super::row_geometry(super::Item::External(view), content, KeyLook::OnGreen);
        let (key, pill) = (geometry.key.expect("F10"), geometry.pill.expect("pastille"));
        assert_eq!(key.left() - pill.right(), 8.0, "{name} : 8 px de la touche");
        assert_eq!((pill.top() - content.top(), pill.bottom() - content.top()), (6.0, 26.0), "{name}");
        assert_eq!(pill.center().y, geometry.text_top + style::LINE_14 / 2.0, "{name} : centrée sur le nom");
        assert!(pill.bottom() <= geometry.text_top + style::LINE_14 + 2.0, "{name} : au-dessus du texte d'état");
        assert_eq!(pill.width(), parts::pill_width(text), "{name} : largeur inchangée");
        assert_eq!(geometry.name_width, pill.left() - 8.0 - geometry.left, "{name} : le nom s'arrête avant");
        assert_eq!(geometry.state_width, key.left() - 8.0 - geometry.left, "{name} : le texte va jusqu'à la touche");
        let (_, words, weight) = super::external_state_line(view);
        assert!(words.ends_with("/100"), "{words}");
        assert!(text::width(&words, 12.0, weight) <= geometry.state_width - 13.0, "« {words} » coupé ({name})");
    }
    // A long name stops 8 px before the pill, cut with « … ».
    let mut long = sample("macro externe active, pastille grise");
    long.external_macros[0].name = "Exemple au nom très long".into();
    let geometry = super::row_geometry(super::Item::External(&long.external_macros[0]), content, KeyLook::OnGreen);
    assert!(text::width("Exemple au nom très long", 14.0, style::SEMIBOLD) > geometry.name_width, "nom coupé");
    // Without a pill, the line does not change.
    for name in ["trois macros, macro externe arrêtée", "macro externe en attente", "macro externe terminée"] {
        let state = sample(name);
        let view = &state.external_macros[0];
        let look = if view.status == MacroStatus::Active { KeyLook::OnGreen } else { KeyLook::Normal };
        let geometry = super::row_geometry(super::Item::External(view), content, look);
        assert!(geometry.pill.is_none(), "{name}");
        assert_eq!(geometry.name_width, geometry.state_width, "{name}");
    }
    // The folded pill keeps its width (D-54).
    set_open_macro(None);
    assert_eq!(window_size(&sample("replié, macro externe active")), vec2(265.0, 36.0));
}

impl Image {
    /// The part `width` × `height` of the image from (`left`, `top`), in pixels.
    fn crop(&self, left: usize, top: usize, width: usize, height: usize) -> Image {
        let pixels = (top..top + height).flat_map(|y| self.pixels[y * self.width + left..][..width].to_vec()).collect();
        Image { width, height, pixels }
    }

    /// Images of one width, one under the other.
    fn stack(images: &[Image]) -> Image {
        let width = images[0].width;
        assert!(images.iter().all(|image| image.width == width));
        let pixels = images.iter().flat_map(|image| image.pixels.clone()).collect();
        Image { width, height: images.iter().map(|image| image.height).sum(), pixels }
    }
}

/// With `MACROHUD_RENDUS`: the line of the external macro cut out of the real HUD at 200 %, for its
/// four pills stacked as on the preview's variant A, and with a long name
/// (`maquettes/macro-externe/pastille-ligne-du-nom-resultat.html`).
#[test]
fn renders_the_pill_on_the_name_line() {
    if std::env::var_os("MACROHUD_RENDUS").is_none() {
        return;
    }
    let mut renderer = Renderer::new();
    let dir = output_dir();
    let [side, top, _] = style::SHADOW_MARGIN.map(|v| v as usize * 2);
    let line = |renderer: &mut Renderer, mut state: HudState| {
        // A message below the macros: the line is not the last block, its corners stay square, as on
        // the preview's drawing (the message is cut away).
        state.message = Some(Message::new(MessageLevel::Info, "Nouveau raccourci de « Exemple » : F10."));
        let image = render_alone(renderer, &View { key: "ligne", state, open: None }, 2.0);
        // The ring and the border on each side; the line from its top to its bottom line.
        image.crop(side, top + EXTERNAL_ROW_TOP as usize * 2, 290 * 2, 50 * 2)
    };
    let names = [
        "macro externe active, pastille ambre",
        "macro externe active, pastille verte",
        "macro externe active, pastille grise",
        "macro externe active, pastille bleue",
    ];
    let lines: Vec<Image> = names.iter().map(|name| line(&mut renderer, sample(name))).collect();
    Image::stack(&lines).save_png(&dir.join("externe-pastille-ligne-du-nom@2x.png"));
    let mut long = sample("macro externe active, pastille grise");
    long.external_macros[0].name = "Exemple au nom très long".into();
    line(&mut renderer, long).save_png(&dir.join("externe-pastille-nom-long@2x.png"));
    set_open_macro(None);
}
