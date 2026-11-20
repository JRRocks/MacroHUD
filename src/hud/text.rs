//! Inter, embedded in the executable, and the text of the HUD.
//!
//! Inter (SIL Open Font License 1.1, see `police/OFL.txt`) is the font of the mock-ups. One
//! variable font file holds every weight; each text picks its weight on the `wght` axis.
//!
//! Text is measured with fonts of its own (no egui context needed), so that `window_size` and
//! `draw` compute the very same layout; it is painted with the context's painter.

use std::cell::RefCell;
use std::sync::Arc;

use eframe::egui::epaint::text::{Fonts, TextOptions, VariationCoords};
use eframe::egui::{
    self, Align, Color32, FontData, FontDefinitions, FontFamily, FontId, Galley, Painter, Pos2,
    emath::GuiRounding as _,
    text::{LayoutJob, TextFormat},
};

/// Latin subset of Inter 4, variable on `wght` (100–900): the exact file the mock-ups used.
const INTER: &[u8] = include_bytes!("police/Inter-latin-variable.ttf");
const INTER_NAME: &str = "Inter";

/// egui's fonts with Inter first; egui's own fonts stay as fallback.
pub fn font_definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(INTER_NAME.to_owned(), Arc::new(FontData::from_static(INTER)));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, INTER_NAME.to_owned());
    fonts
}

fn format(size: f32, weight: f32, color: Color32) -> TextFormat {
    TextFormat {
        font_id: FontId::new(size, FontFamily::Proportional),
        color,
        coords: VariationCoords::new([(b"wght", weight)]),
        valign: Align::Center,
        ..Default::default()
    }
}

/// One line of text; longer than `max_width`, it is cut and ends with « … ».
fn line_job(text: &str, size: f32, weight: f32, color: Color32, max_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, format(size, weight, color));
    job.wrap.max_width = max_width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    job
}

// Measuring ------------------------------------------------------------------------------------

thread_local! {
    /// Fonts used only to measure (one HUD per program; parsed once).
    static MEASURE: RefCell<Option<Fonts>> = const { RefCell::new(None) };
}

fn measure<R>(job: LayoutJob, read: impl FnOnce(&Galley) -> R) -> R {
    MEASURE.with(|cell| {
        let mut cell = cell.borrow_mut();
        let fonts = cell.get_or_insert_with(|| Fonts::new(TextOptions::default(), font_definitions()));
        read(&fonts.with_pixels_per_point(1.0).layout_job(job))
    })
}

/// Keeps the measuring fonts small: forgets the layouts not used since the previous call.
pub fn new_frame() {
    MEASURE.with(|cell| {
        if let Some(fonts) = cell.borrow_mut().as_mut() {
            fonts.begin_pass(TextOptions::default(), Default::default());
            let _ = fonts.font_image_delta();
        }
    });
}

/// Width of one line of text, in points.
pub fn width(text: &str, size: f32, weight: f32) -> f32 {
    measure(line_job(text, size, weight, Color32::WHITE, f32::INFINITY), |g| g.size().x)
}

/// Lines of `text` wrapped within `max_width`; `\n` also starts a line.
pub fn wrap(text: &str, size: f32, weight: f32, max_width: f32) -> Vec<String> {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, format(size, weight, Color32::WHITE));
    job.wrap.max_width = max_width;
    measure(job, |galley| {
        galley
            .rows
            .iter()
            .map(|row| row.glyphs.iter().map(|g| g.chr).collect::<String>().trim_end().to_owned())
            .collect()
    })
}

// Painting -------------------------------------------------------------------------------------

/// One line laid out by the painter, cut with « … » beyond `max_width`.
pub fn line(painter: &Painter, text: &str, size: f32, weight: f32, color: Color32, max_width: f32) -> Arc<Galley> {
    painter.layout_job(line_job(text, size, weight, color, max_width))
}

/// Paints `galley` from `left`, centred in the line box that starts at `top` (CSS line box).
/// Returns the width of the text.
pub fn put(painter: &Painter, galley: &Arc<Galley>, left: f32, top: f32, line_height: f32) -> f32 {
    let y = top + (line_height - galley.size().y) / 2.0;
    let pos = Pos2::new(left, y).round_to_pixels(painter.pixels_per_point());
    painter.galley(pos, Arc::clone(galley), Color32::PLACEHOLDER);
    galley.size().x
}

/// Lays out and paints one line from `left`; returns its width.
#[expect(clippy::too_many_arguments, reason = "plain drawing helper")]
pub fn draw(
    painter: &Painter,
    text: &str,
    size: f32,
    weight: f32,
    color: Color32,
    left: f32,
    top: f32,
    line_height: f32,
) -> f32 {
    put(painter, &line(painter, text, size, weight, color, f32::INFINITY), left, top, line_height)
}

/// Paints one line centred on `centre_x`.
pub fn centred(painter: &Painter, galley: &Arc<Galley>, centre_x: f32, top: f32, line_height: f32) {
    put(painter, galley, (centre_x - galley.size().x / 2.0).round(), top, line_height);
}

/// Installs Inter in the egui context.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}
