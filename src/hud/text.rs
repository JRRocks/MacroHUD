//! Inter, embedded in the executable, and the text helpers of the HUD.
//!
//! Inter (SIL Open Font License 1.1, see `police/OFL.txt`) is the font of the mock-ups. One
//! variable font file holds every weight; each text picks its weight on the `wght` axis.

use std::sync::Arc;

use egui::epaint::text::VariationCoords;
use egui::{
    Align, Color32, Context, FontData, FontDefinitions, FontFamily, FontId, Galley, Painter, Pos2,
    emath::GuiRounding as _,
    text::{LayoutJob, TextFormat},
};

/// Latin subset of Inter 4, variable on `wght` (100–900): the exact file the mock-ups used.
const INTER: &[u8] = include_bytes!("police/Inter-latin-variable.ttf");
const INTER_NAME: &str = "Inter";

/// Makes Inter the first proportional font; egui's own fonts stay as fallback.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        INTER_NAME.to_owned(),
        Arc::new(FontData::from_static(INTER)),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, INTER_NAME.to_owned());
    ctx.set_fonts(fonts);
}

fn format(size: f32, weight: f32, color: Color32, line_height: Option<f32>) -> TextFormat {
    TextFormat {
        font_id: FontId::new(size, FontFamily::Proportional),
        color,
        line_height,
        coords: VariationCoords::new([(b"wght", weight)]),
        valign: Align::Center,
        ..Default::default()
    }
}

/// One line of text; longer than `max_width`, it is cut and ends with « … ».
pub fn line(
    painter: &Painter,
    text: &str,
    size: f32,
    weight: f32,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, format(size, weight, color, None));
    job.wrap.max_width = max_width;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    job.wrap.overflow_character = Some('…');
    painter.layout_job(job)
}

/// Text that wraps within `max_width`, lines `line_height` points apart; `\n` starts a line.
pub fn paragraph(
    painter: &Painter,
    text: &str,
    size: f32,
    weight: f32,
    color: Color32,
    max_width: f32,
    line_height: f32,
) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, format(size, weight, color, Some(line_height)));
    job.wrap.max_width = max_width;
    painter.layout_job(job)
}

/// Width of one line of text.
pub fn width(painter: &Painter, text: &str, size: f32, weight: f32) -> f32 {
    line(painter, text, size, weight, Color32::WHITE, f32::INFINITY)
        .size()
        .x
}

/// Paints `galley` from `left`, centred in the line box that starts at `top` (CSS line box).
/// Returns the width of the text.
pub fn put(painter: &Painter, galley: &Arc<Galley>, left: f32, top: f32, line_height: f32) -> f32 {
    let y = top + (line_height - galley.size().y) / 2.0;
    let pos = Pos2::new(left, y).round_to_pixels(painter.pixels_per_point());
    painter.galley(pos, Arc::clone(galley), Color32::PLACEHOLDER);
    galley.size().x
}

/// Shortcut: lays out and paints one line, returns its width.
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
    let galley = line(painter, text, size, weight, color, f32::INFINITY);
    put(painter, &galley, left, top, line_height)
}
