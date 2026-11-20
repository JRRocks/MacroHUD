//! egui's triangles rasterised in software, the way its GPU back-ends do (premultiplied alpha,
//! colours multiplied and blended in gamma space), then written as PNG. The same rasteriser as the
//! HUD's render tests (`src/hud/render.rs`), so that these images and those of `maquettes/rendu-egui/`
//! come out alike; here a pass can also receive pointer events (hover).

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use eframe::egui::epaint::{ImageData, Mesh, Primitive, TextureId, Vertex};
use eframe::egui::{
    Color32, Context, Event, Pos2, RawInput, Rect, TextureFilter, TexturesDelta, Ui, Vec2, ViewportId, ViewportInfo,
};

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
pub struct Image {
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
    fn new(width: usize, height: usize, background: Color32) -> Self {
        let fill = [background.r(), background.g(), background.b(), background.a()].map(|v| f32::from(v) / 255.0);
        Image { width, height, pixels: vec![fill; width * height] }
    }

    /// Alpha of a pixel, 0–255.
    pub fn alpha(&self, x: usize, y: usize) -> u8 {
        (self.pixels[y * self.width + x][3] * 255.0).round().clamp(0.0, 255.0) as u8
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

/// An egui context set up like the application's, plus the textures it uploaded.
pub struct Renderer {
    ctx: Context,
    textures: HashMap<TextureId, Texture>,
}

impl Renderer {
    pub fn new() -> Self {
        let ctx = Context::default();
        crate::hud::setup(&ctx);
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

    /// Runs `ui_fn` on a screen of `size` points, the pointer at `pointer` (or outside), and
    /// rasterises it at `pixels_per_point`.
    pub fn render(
        &mut self,
        size: Vec2,
        pixels_per_point: f32,
        pointer: Option<Pos2>,
        ui_fn: &mut dyn FnMut(&mut Ui),
    ) -> Image {
        let events = || match pointer {
            Some(pos) => vec![Event::PointerMoved(pos)],
            None => vec![Event::PointerGone],
        };
        // The first pass loads the glyphs and places the pointer; the second one is painted.
        let mut first = self.ctx.run_ui(Self::input(size, pixels_per_point, events()), |ui| ui_fn(ui));
        self.apply(&mut first.textures_delta);
        let mut output = self.ctx.run_ui(Self::input(size, pixels_per_point, events()), |ui| ui_fn(ui));
        self.apply(&mut output.textures_delta);
        let ppp = output.pixels_per_point;
        let primitives = self.ctx.tessellate(output.shapes, ppp);
        let size = (size * ppp).round();
        let mut image = Image::new(size.x as usize, size.y as usize, Color32::TRANSPARENT);
        for primitive in &primitives {
            if let Primitive::Mesh(mesh) = &primitive.primitive {
                image.draw(mesh, primitive.clip_rect, ppp, &self.textures);
            }
        }
        image
    }
}
