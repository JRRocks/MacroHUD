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
            // Only plain spaces end a row: the no-break spaces of a sign's room stay (see `SIGNS`).
            .map(|row| row.glyphs.iter().map(|g| g.chr).collect::<String>().trim_end_matches([' ', '\n']).to_owned())
            .collect()
    })
}

/// The HUD's signs that Inter lacks, in a text from outside MacroHUD (the help line or the box of
/// an external macro): « ▶ » (Activer) and « ■ » (Arrêter), drawn as the HUD's icons (D-56). In the
/// text laid out, each one becomes a zero-width marker (once for ▶, twice for ■) and three no-break
/// spaces, the room of its icon: egui never cuts a line inside it.
const SIGN_MARK: char = '\u{FEFF}';
const SIGN_ROOM: &str = "\u{A0}\u{A0}\u{A0}";

/// `text` with room for its signs (see [`SIGN_MARK`]).
pub fn with_sign_room(text: &str) -> String {
    text.replace('▶', &format!("{SIGN_MARK}{SIGN_ROOM}")).replace('■', &format!("{SIGN_MARK}{SIGN_MARK}{SIGN_ROOM}"))
}

/// The signs of a laid-out text: each sign, and the middle of its room from the galley's left.
pub fn sign_spots(galley: &Galley) -> Vec<(char, f32)> {
    let mut spots = Vec::new();
    for row in &galley.rows {
        let glyphs = &row.glyphs;
        let mut k = 0;
        while k < glyphs.len() {
            if glyphs[k].chr != SIGN_MARK {
                k += 1;
                continue;
            }
            let marks = glyphs[k..].iter().take_while(|g| g.chr == SIGN_MARK).count();
            let room: Vec<_> = glyphs[k + marks..].iter().take_while(|g| g.chr == '\u{A0}').take(3).collect();
            if let (Some(first), Some(last)) = (room.first(), room.last()) {
                let middle = row.pos.x + (first.pos.x + last.pos.x + last.advance_width) / 2.0;
                spots.push((if marks >= 2 { '■' } else { '▶' }, middle));
            }
            k += marks + room.len();
        }
    }
    spots
}

/// A paragraph made of sections of one size, each with its weight and colour (« ▲ Title. Text »),
/// cut into rows by the measuring fonts once: painted row by row, it keeps these very rows at any
/// scale, so that the layout and the painting agree.
pub struct Paragraph {
    /// Each row: runs of text with their weight and colour.
    rows: Vec<Vec<(String, f32, Color32)>>,
    size: f32,
    /// Room left free before the first row (an icon goes there).
    indent: f32,
}

impl Paragraph {
    pub fn new(sections: &[(&str, f32, Color32)], size: f32, max_width: f32, indent: f32) -> Self {
        let mut job = LayoutJob::default();
        for (k, &(text, weight, color)) in sections.iter().enumerate() {
            job.append(text, if k == 0 { indent } else { 0.0 }, format(size, weight, color));
        }
        job.wrap.max_width = max_width;
        let counts: Vec<usize> = measure(job, |galley| galley.rows.iter().map(|row| row.glyphs.len()).collect());
        let chars: Vec<(char, usize)> =
            sections.iter().enumerate().flat_map(|(k, (text, _, _))| text.chars().map(move |c| (c, k))).collect();
        let mut rows = Vec::new();
        let mut at = 0;
        for count in counts {
            let row = &chars[at.min(chars.len())..(at + count).min(chars.len())];
            at += count;
            // A row ends on the space where it was cut: not drawn.
            let kept = row.len() - row.iter().rev().take_while(|(c, _)| c.is_whitespace()).count();
            let mut runs: Vec<(String, usize)> = Vec::new();
            for &(c, k) in &row[..kept] {
                match runs.last_mut() {
                    Some((run, section)) if *section == k => run.push(c),
                    _ => runs.push((c.to_string(), k)),
                }
            }
            rows.push(runs.into_iter().map(|(run, k)| (run, sections[k].1, sections[k].2)).collect());
        }
        Paragraph { rows, size, indent }
    }

    /// Number of rows.
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    /// The text of each row, for tests.
    #[cfg(test)]
    pub fn row_texts(&self) -> Vec<String> {
        self.rows.iter().map(|runs| runs.iter().map(|(run, _, _)| run.as_str()).collect()).collect()
    }

    /// Paints the rows from `left`, the first at `top`, `line_height` apart. Returns where its
    /// signs go (see [`with_sign_room`]): each sign and the centre of its room.
    pub fn paint(&self, painter: &Painter, left: f32, top: f32, line_height: f32) -> Vec<(char, Pos2)> {
        let mut signs = Vec::new();
        for (r, runs) in self.rows.iter().enumerate() {
            let mut job = LayoutJob::default();
            for (k, (run, weight, color)) in runs.iter().enumerate() {
                let lead = if r == 0 && k == 0 { self.indent } else { 0.0 };
                job.append(run, lead, format(self.size, *weight, *color));
            }
            let galley = painter.layout_job(job);
            let row_top = top + r as f32 * line_height;
            put(painter, &galley, left, row_top, line_height);
            let x = Pos2::new(left, 0.0).round_to_pixels(painter.pixels_per_point()).x;
            signs.extend(
                sign_spots(&galley)
                    .into_iter()
                    .map(|(sign, mid)| (sign, Pos2::new(x + mid, row_top + line_height / 2.0))),
            );
        }
        signs
    }
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
