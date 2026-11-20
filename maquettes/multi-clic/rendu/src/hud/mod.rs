//! The HUD with two macros, « Maintenir le clic » (F8) and « Multi-clic » (F9), and three ways of
//! setting how fast « Multi-clic » clicks (task P2.2). A fork, for mock-ups, of the layout of
//! `src/hud/mod.rs` (variant 2, decisions D-12 and D-16); the colours, measures, fonts, icons and
//! building blocks are the HUD's own files, compiled here unchanged. Nothing here is asked of the
//! engine: these renders only show.

#[path = "../../../../../src/hud/icons.rs"]
mod icons;
#[path = "../../../../../src/hud/parts.rs"]
mod parts;
#[path = "../../../../../src/hud/style.rs"]
mod style;
#[path = "../../../../../src/hud/text.rs"]
mod text;

use eframe::egui::{
    self, Color32, CursorIcon, Id, Painter, Pos2, Rect, Role, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, WidgetInfo,
    epaint::PathShape, pos2, vec2,
};

use crate::contract::{HudState, MacroStatus, MacroView, MouseButton};
use icons::Icon;
use parts::{IconButton, KEY_HEIGHT, KeyLook};
use style as s;

/// Installs Inter, like the application.
pub fn setup(ctx: &egui::Context) {
    text::install(ctx);
}

// What a preview shows ---------------------------------------------------------------------------

/// How « Multi-clic » sets its speed: the three tracks of P2.2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    /// A: a slider from 1 to 15, the value written above it.
    Slider,
    /// B: « − » and « + » on each side of the value.
    Stepper,
    /// C: three presets, and « − » « + » for a finer setting.
    Presets,
}

/// The presets of track C: 10, the default (D-22), is « Moyen ».
pub const PRESETS: [(&str, u8); 3] = [("Lent", 5), ("Moyen", 10), ("Rapide", 15)];

/// « Multi-clic » as `docs/CONTRAT-HUD.md` describes it (additions of 2026-11-20, D-18 to D-22).
/// `src/contract.rs` does not have it yet: the "programmation" session adds it (P2.1).
#[derive(Clone, Debug)]
pub struct MultiClick {
    pub status: MacroStatus,
    pub hotkey: String,
    pub button: MouseButton,
    /// Clicks per second, from `min` to `max`.
    pub rate: u8,
    pub min: u8,
    pub max: u8,
    /// Active, but its clicks wait: the mouse is on the HUD.
    pub suspended: bool,
}

/// A preview: the title bar and « Maintenir le clic » of the contract, then « Multi-clic ».
#[derive(Clone, Debug)]
pub struct Preview {
    /// Name, version and « Maintenir le clic » (its first macro).
    pub state: HudState,
    pub multi: MultiClick,
    pub speed: Speed,
    /// The settings of « Multi-clic » are unfolded.
    pub open: bool,
}

/// « 10 clics/s », « 1 clic/s ».
pub fn rate_text(rate: u8) -> String {
    if rate == 1 { "1 clic/s".to_owned() } else { format!("{rate} clics/s") }
}

/// The clicked button in one word: « gauche », « droit », « milieu ».
fn button_word(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "gauche",
        MouseButton::Right => "droit",
        MouseButton::Middle => "milieu",
    }
}

// Layout -----------------------------------------------------------------------------------------

/// Width of a settings block's content (12 px left padding, 8 px right), as in the HUD.
const CONTENT_WIDTH: f32 = s::PANEL_WIDTH - 2.0 - 12.0 - 8.0;
/// Widest keycap on a macro line, as in the HUD.
const KEY_MAX_IN_ROW: f32 = 84.0;
/// Track A: the slider's line (bounds, track, thumb).
const SLIDER_HEIGHT: f32 = 24.0;
const THUMB_RADIUS: f32 = 8.0;
/// Track B: as high as the button selector above it.
const STEPPER_HEIGHT: f32 = parts::SELECTOR_HEIGHT;
/// Track C: the « Réglage fin » line, as high as « Raccourci ».
const FINE_HEIGHT: f32 = 26.0;

/// The speed setting: its title line, then its control.
fn speed_height(speed: Speed) -> f32 {
    let title = s::LINE_12 + 4.0;
    match speed {
        Speed::Slider => title + SLIDER_HEIGHT,
        Speed::Stepper => title + STEPPER_HEIGHT,
        Speed::Presets => title + parts::SELECTOR_HEIGHT + 8.0 + FINE_HEIGHT,
    }
}

/// Settings of « Multi-clic »: clicked button, speed, hotkey.
fn options_height(speed: Speed) -> f32 {
    let selector = s::LINE_12 + 4.0 + parts::SELECTOR_HEIGHT + 10.0;
    8.0 + selector + speed_height(speed) + 10.0 + 26.0 + 10.0 + 1.0
}

/// The window's inner size, in points: the HUD, its 1 px black ring included.
pub fn size(preview: &Preview) -> Vec2 {
    let options = if preview.open { options_height(preview.speed) } else { 0.0 };
    vec2(s::PANEL_WIDTH + 2.0, 1.0 + 1.0 + s::BAR_HEIGHT + 2.0 * s::ROW_HEIGHT + options + 1.0 + 1.0)
}

/// What a macro line shows.
struct Row<'a> {
    name: &'a str,
    hotkey: &'a str,
    active: bool,
    open: bool,
    mark: Mark,
    words: String,
    weight: f32,
}

/// The sign before the state line.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Icon(Icon),
    /// Two bars: the clicks wait (not among the HUD's icons yet).
    Pause,
}

/// « Maintenir le clic », exactly as the HUD writes it (D-16).
fn hold_row(view: &MacroView) -> Row<'_> {
    let button = button_word(view.options.button());
    let (mark, words, weight) = match view.status {
        MacroStatus::Active => (Icon::Dot, format!("Active · {button} enfoncé"), s::SEMIBOLD),
        _ => (Icon::Stop, format!("Arrêtée · bouton {button}"), s::REGULAR),
    };
    Row {
        name: view.name,
        hotkey: &view.hotkey,
        active: view.status == MacroStatus::Active,
        open: false,
        mark: Mark::Icon(mark),
        words,
        weight,
    }
}

/// The state line of « Multi-clic »: « Arrêtée · gauche, 10/s », « Active · gauche, 10/s », and
/// « En pause · souris ici » while its clicks wait. « Active · 10 clics/s, gauche » would not fit
/// (150 pt for 143, beside the hotkey): the line would be cut.
pub fn multi_words(multi: &MultiClick) -> String {
    let what = format!("{}, {}/s", button_word(multi.button), multi.rate);
    match (&multi.status, multi.suspended) {
        (MacroStatus::Active, true) => "En pause · souris ici".to_owned(),
        (MacroStatus::Active, false) => format!("Active · {what}"),
        _ => format!("Arrêtée · {what}"),
    }
}

fn multi_row(multi: &MultiClick, open: bool) -> Row<'_> {
    let active = multi.status == MacroStatus::Active;
    let mark = match (active, multi.suspended) {
        (true, true) => Mark::Pause,
        (true, false) => Mark::Icon(Icon::Dot),
        _ => Mark::Icon(Icon::Stop),
    };
    Row {
        name: "Multi-clic",
        hotkey: &multi.hotkey,
        active,
        open,
        mark,
        words: multi_words(multi),
        weight: if active { s::SEMIBOLD } else { s::REGULAR },
    }
}

/// Room for the state line of a macro line whose hotkey is `hotkey`: as in `paint_row`.
pub fn state_room(hotkey: &str, active: bool) -> f32 {
    let look = if active { KeyLook::OnGreen } else { KeyLook::Normal };
    let content_right = s::PANEL_WIDTH - 2.0;
    let arrow_left = content_right - 6.0 - 24.0;
    let key_left = arrow_left - 8.0 - parts::keycap_width(hotkey, look, KEY_MAX_IN_ROW);
    let left = 14.0 + 32.0 + 8.0;
    key_left - 8.0 - left - (9.0 + 4.0)
}

/// Width of a state line's words.
pub fn state_width(words: &str, active: bool) -> f32 {
    text::width(words, 12.0, if active { s::SEMIBOLD } else { s::REGULAR })
}

// Painting ---------------------------------------------------------------------------------------

/// Draws the HUD at the top left of `ui`, with the drop shadow of the mock-ups.
pub fn paint(ui: &mut Ui, preview: &Preview) {
    text::new_frame();
    let (rect, _) = ui.allocate_exact_size(size(preview), Sense::hover());
    let p = ui.painter().clone();
    let panel = rect.shrink(1.0);
    p.add(s::SHADOW.as_shape(panel, s::RADIUS));
    p.rect_filled(panel, s::RADIUS, s::PANEL);
    let inner = panel.shrink(1.0);
    let id = ui.make_persistent_id("macrohud");
    let mut top = paint_bar(ui, &p, inner, id, &preview.state);
    let rows = [hold_row(&preview.state.macros[0]), multi_row(&preview.multi, preview.open)];
    for (k, row) in rows.iter().enumerate() {
        let last = k + 1 == rows.len() && !preview.open;
        let area = Rect::from_min_size(pos2(inner.left(), top), vec2(inner.width(), s::ROW_HEIGHT));
        paint_row(ui, &p, area, id.with(k), row, last);
        top += s::ROW_HEIGHT;
    }
    if preview.open {
        let area = Rect::from_min_size(pos2(inner.left(), top), vec2(inner.width(), options_height(preview.speed)));
        paint_options(ui, &p, area, id.with("multi-clic"), preview);
    }
    // Border and ring last: blocks never cover them.
    p.rect_stroke(panel, s::RADIUS, Stroke::new(1.0, s::EDGE), StrokeKind::Inside);
    p.rect_stroke(rect, s::RADIUS + 1, Stroke::new(1.0, s::RING), StrokeKind::Inside);
}

/// Title bar, as in the HUD: drag handle, name and version, « Replier », « Quitter ».
fn paint_bar(ui: &Ui, p: &Painter, inner: Rect, id: Id, state: &HudState) -> f32 {
    let bar = Rect::from_min_size(inner.min, vec2(inner.width(), s::BAR_HEIGHT - 1.0));
    let middle = bar.center().y;
    let button_top = (middle - 12.0).round();
    let quit_width = parts::bar_button_width("Quitter");
    let quit = Rect::from_min_size(pos2(bar.right() - 4.0 - quit_width, button_top), vec2(quit_width, 24.0));
    let fold_width = parts::bar_button_width("Replier");
    let fold = Rect::from_min_size(pos2(quit.left() - 6.0 - fold_width, button_top), vec2(fold_width, 24.0));

    let grip = Rect::from_min_size(pos2(bar.left() + 6.0, middle - 8.0), vec2(12.0, 16.0));
    icons::paint(p, Icon::Grip, grip, s::GRIP);
    let line_top = bar.top() + ((bar.height() - s::LINE_12) / 2.0).round();
    let mut x = grip.right() + 6.0;
    x += text::draw(p, state.app_name, 12.0, s::BOLD, s::TEXT, x, line_top, s::LINE_12) + 6.0;
    let version = text::line(p, state.version, 12.0, s::REGULAR, s::TEXT_2, fold.left() - 4.0 - x);
    text::put(p, &version, x, line_top, s::LINE_12);

    parts::bar_button(ui, p, fold, id.with("replier"), Icon::Up, "Replier");
    parts::bar_button(ui, p, quit, id.with("quitter"), Icon::Close, "Quitter");
    p.hline(inner.x_range(), bar.bottom() + 0.5, Stroke::new(1.0, s::BAR_LINE));
    inner.top() + s::BAR_HEIGHT
}

/// One macro line, as in the HUD: ▶ / ■, name and state, hotkey, settings arrow.
fn paint_row(ui: &Ui, p: &Painter, block: Rect, id: Id, row: &Row<'_>, last: bool) {
    let (fill, line, stripe) =
        if row.active { (s::ON, s::ON_LINE, s::ON_INK) } else { (s::ROW, s::ROW_LINE, s::STRIPE) };
    parts::block(p, block, fill, line, last);
    let content = Rect::from_min_size(block.min, vec2(block.width(), s::ROW_HEIGHT - 1.0));
    parts::stripe(p, Rect::from_min_size(content.min, vec2(4.0, content.height())), stripe, block.bottom(), last);
    let middle = content.center().y;

    let toggle = Rect::from_min_size(pos2(content.left() + 14.0, (middle - 16.0).round()), vec2(32.0, 32.0));
    let look = if row.active {
        IconButton {
            icon: Icon::Stop,
            icon_size: 13.0,
            ink: s::ON,
            fill: s::ON_INK,
            fill_hover: s::ON_INK_HOVER,
            fill_press: s::ON_INK,
            edge: None,
            radius: 8,
        }
    } else {
        IconButton {
            icon: Icon::Play,
            icon_size: 13.0,
            ink: s::TEXT,
            fill: s::CONTROL,
            fill_hover: s::CONTROL_HOVER,
            fill_press: s::CONTROL_PRESS,
            edge: Some(s::CONTROL_EDGE),
            radius: 8,
        }
    };
    parts::icon_button(ui, p, toggle, id.with("basculer"), &look, if row.active { "Arrêter" } else { "Activer" });

    let arrow = Rect::from_min_size(pos2(content.right() - 6.0 - 24.0, (middle - 12.0).round()), vec2(24.0, 24.0));
    let look = IconButton {
        icon: if row.open { Icon::Up } else { Icon::Down },
        icon_size: 13.0,
        ink: if row.active { s::ON_INK } else { s::TEXT_BUTTON },
        fill: Color32::TRANSPARENT,
        fill_hover: if row.active { s::ON_HOVER } else { s::ROW_HOVER },
        fill_press: if row.active { s::ON_HOVER } else { s::ROW_HOVER },
        edge: None,
        radius: 5,
    };
    let label = if row.open { "Masquer les réglages" } else { "Afficher les réglages" };
    parts::icon_button(ui, p, arrow, id.with("reglages"), &look, label);
    let key_look = if row.active { KeyLook::OnGreen } else { KeyLook::Normal };
    let key_width = parts::keycap_width(row.hotkey, key_look, KEY_MAX_IN_ROW);
    let key =
        Rect::from_min_size(pos2(arrow.left() - 8.0 - key_width, (middle - 12.0).round()), vec2(key_width, KEY_HEIGHT));
    parts::keycap(p, key, row.hotkey, key_look);

    let left = toggle.right() + 8.0;
    let max_width = key.left() - 8.0 - left;
    let (name_ink, state_ink) = if row.active { (s::ON_INK, s::ON_INK) } else { (s::TEXT, s::TEXT_2) };
    let text_top = content.top() + ((content.height() - (s::LINE_14 + 2.0 + s::LINE_12)) / 2.0).round();
    let name = text::line(p, row.name, 14.0, s::SEMIBOLD, name_ink, max_width);
    text::put(p, &name, left, text_top, s::LINE_14);
    let state_top = text_top + s::LINE_14 + 2.0;
    let centre = pos2(left + 4.5, state_top + s::LINE_12 / 2.0);
    match row.mark {
        Mark::Icon(icon) => icons::paint(p, icon, Rect::from_center_size(centre, vec2(9.0, 9.0)), state_ink),
        Mark::Pause => pause_sign(p, Rect::from_center_size(centre, vec2(9.0, 9.0)), state_ink),
    }
    let x = left + 9.0 + 4.0;
    let state = text::line(p, &row.words, 12.0, row.weight, state_ink, max_width - (x - left));
    text::put(p, &state, x, state_top, s::LINE_12);
}

/// Two upright bars in a 9 px square: the clicks wait.
fn pause_sign(p: &Painter, rect: Rect, color: Color32) {
    let bar = vec2(3.0, rect.height());
    p.rect_filled(Rect::from_min_size(pos2(rect.left() + 0.5, rect.top()), bar), 1, color);
    p.rect_filled(Rect::from_min_size(pos2(rect.right() - 3.5, rect.top()), bar), 1, color);
}

/// Settings of « Multi-clic », under its line: clicked button, speed, hotkey.
fn paint_options(ui: &Ui, p: &Painter, block: Rect, id: Id, preview: &Preview) {
    let multi = &preview.multi;
    parts::block(p, block, s::OPTIONS, s::ROW_LINE, true);
    let left = block.left() + 12.0;
    let mut y = block.top() + 8.0;

    text::draw(p, "Bouton cliqué", 12.0, s::SEMIBOLD, s::TEXT_2, left, y, s::LINE_12);
    y += s::LINE_12 + 4.0;
    let selector = Rect::from_min_size(pos2(left, y), vec2(CONTENT_WIDTH, parts::SELECTOR_HEIGHT));
    parts::selector(ui, p, selector, id.with("bouton"), multi.button);
    y += parts::SELECTOR_HEIGHT + 10.0;

    let speed = Rect::from_min_size(pos2(left, y), vec2(CONTENT_WIDTH, speed_height(preview.speed)));
    paint_speed(ui, p, speed, id.with("rapidite"), preview.speed, multi);
    y += speed.height() + 10.0;

    let label_width = text::draw(p, "Raccourci", 12.0, s::SEMIBOLD, s::TEXT_2, left, y, 26.0);
    let button_width = parts::small_button_width("Changer");
    let button = Rect::from_min_size(pos2(left + CONTENT_WIDTH - button_width, y), vec2(button_width, 26.0));
    parts::small_button(ui, p, button, id.with("changer"), "Changer");
    let room = button.left() - 6.0 - (left + label_width + 8.0);
    let key_width = parts::keycap_width(&multi.hotkey, KeyLook::Normal, room);
    let key = Rect::from_min_size(pos2(button.left() - 6.0 - key_width, y + 1.0), vec2(key_width, KEY_HEIGHT));
    parts::keycap(p, key, &multi.hotkey, KeyLook::Normal);
}

/// Title line of a setting: its name on the left, `aside` on the right.
fn title_line(p: &Painter, rect: Rect, title: &str, aside: &str, aside_strong: bool) {
    text::draw(p, title, 12.0, s::SEMIBOLD, s::TEXT_2, rect.left(), rect.top(), s::LINE_12);
    let (weight, ink) = if aside_strong { (s::BOLD, s::TEXT) } else { (s::REGULAR, s::TEXT_2) };
    let galley = text::line(p, aside, 12.0, weight, ink, f32::INFINITY);
    text::put(p, &galley, rect.right() - galley.size().x, rect.top(), s::LINE_12);
}

/// The speed setting of `speed`'s track, in `rect` (title line included).
fn paint_speed(ui: &Ui, p: &Painter, rect: Rect, id: Id, speed: Speed, multi: &MultiClick) {
    let value = rate_text(multi.rate);
    let control_top = rect.top() + s::LINE_12 + 4.0;
    match speed {
        Speed::Slider => {
            title_line(p, rect, "Rapidité", &value, true);
            let line = Rect::from_min_size(pos2(rect.left(), control_top), vec2(rect.width(), SLIDER_HEIGHT));
            slider(ui, p, line, id, multi);
        }
        Speed::Stepper => {
            title_line(p, rect, "Rapidité", &format!("de {} à {}", multi.min, multi.max), false);
            let line = Rect::from_min_size(pos2(rect.left(), control_top), vec2(rect.width(), STEPPER_HEIGHT));
            stepper(ui, p, line, id, multi);
        }
        Speed::Presets => {
            title_line(p, rect, "Rapidité", &value, true);
            let line = Rect::from_min_size(pos2(rect.left(), control_top), vec2(rect.width(), parts::SELECTOR_HEIGHT));
            let selected = PRESETS.iter().position(|&(_, rate)| rate == multi.rate);
            let labels = PRESETS.map(|(label, _)| label);
            segments(ui, p, line, id.with("prereglages"), &labels, selected);
            let fine_top = line.bottom() + 8.0;
            text::draw(p, "Réglage fin", 12.0, s::REGULAR, s::TEXT_2, rect.left(), fine_top, FINE_HEIGHT);
            let plus = Rect::from_min_size(pos2(rect.right() - FINE_HEIGHT, fine_top), Vec2::splat(FINE_HEIGHT));
            let minus = plus.translate(vec2(-FINE_HEIGHT - 6.0, 0.0));
            small_step(ui, p, minus, id.with("moins"), Sign::Minus, multi.rate > multi.min);
            small_step(ui, p, plus, id.with("plus"), Sign::Plus, multi.rate < multi.max);
        }
    }
}

/// Where the pointer's tip rests in view 3 (« souris sur le HUD »), from the top left of the HUD:
/// on the thumb (A), on « + » (B, C), a little aside so that the sign still shows.
pub fn pointer_spot(preview: &Preview) -> Pos2 {
    let multi = &preview.multi;
    let left = 1.0 + 1.0 + 12.0;
    let top = 1.0 + 1.0 + s::BAR_HEIGHT + 2.0 * s::ROW_HEIGHT + 8.0 + s::LINE_12 + 4.0 + parts::SELECTOR_HEIGHT + 10.0;
    let control_top = top + s::LINE_12 + 4.0;
    match preview.speed {
        Speed::Slider => {
            let line = Rect::from_min_size(pos2(left, control_top), vec2(CONTENT_WIDTH, SLIDER_HEIGHT));
            let (_, x_of) = slider_track(line, multi);
            pos2(x_of(multi.rate) + 3.0, line.center().y + 3.0)
        }
        Speed::Stepper => pos2(left + CONTENT_WIDTH - 3.0 - 20.0 + 6.0, control_top + STEPPER_HEIGHT / 2.0 + 4.0),
        Speed::Presets => {
            let fine_top = control_top + parts::SELECTOR_HEIGHT + 8.0;
            pos2(left + CONTENT_WIDTH - FINE_HEIGHT / 2.0 + 6.0, fine_top + FINE_HEIGHT / 2.0 + 5.0)
        }
    }
}

// Controls ---------------------------------------------------------------------------------------

/// Track A's geometry in its line: the track, and the thumb's centre for a value.
fn slider_track(line: Rect, multi: &MultiClick) -> (Rect, impl Fn(u8) -> f32) {
    let low = text::width(&multi.min.to_string(), 12.0, s::MEDIUM);
    let high = text::width(&multi.max.to_string(), 12.0, s::MEDIUM);
    let middle = line.center().y;
    let track =
        Rect::from_x_y_ranges((line.left() + low + 8.0)..=(line.right() - high - 8.0), (middle - 3.0)..=(middle + 3.0));
    let (min, max) = (f32::from(multi.min), f32::from(multi.max));
    let (start, end) = (track.left() + THUMB_RADIUS, track.right() - THUMB_RADIUS);
    (track, move |value: u8| (start + (end - start) * (f32::from(value) - min) / (max - min)).round())
}

/// Track A: « 1 », the track with one notch per value, « 15 »; the light part and the thumb show
/// the value. Returns the value chosen by a click or a drag, if it changed.
fn slider(ui: &Ui, p: &Painter, line: Rect, id: Id, multi: &MultiClick) -> Option<u8> {
    let (track, x_of) = slider_track(line, multi);
    let middle = line.center().y;
    text::draw(p, &multi.min.to_string(), 12.0, s::MEDIUM, s::TEXT_2, line.left(), line.top(), line.height());
    let high = text::line(p, &multi.max.to_string(), 12.0, s::MEDIUM, s::TEXT_2, f32::INFINITY);
    text::put(p, &high, line.right() - high.size().x, line.top(), line.height());

    let hit = Rect::from_x_y_ranges(track.x_range(), line.y_range());
    let response = ui.interact(hit, id, Sense::click_and_drag());
    let hot = response.hovered() || response.dragged();
    if hot {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let value = multi.rate;
    response.widget_info(move || WidgetInfo::slider(true, f64::from(value), "Rapidité, en clics par seconde"));
    let mut chosen = None;
    if (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let nearest = (multi.min..=multi.max)
            .min_by(|&a, &b| (x_of(a) - pointer.x).abs().total_cmp(&(x_of(b) - pointer.x).abs()));
        chosen = nearest.filter(|&v| v != value);
    }

    p.rect_filled(track, 3, s::CONTROL);
    let x = x_of(value);
    p.rect_filled(Rect::from_x_y_ranges(track.left()..=x, track.y_range()), 3, s::KEY);
    for v in multi.min..=multi.max {
        let tick = x_of(v) + 0.5;
        p.vline(tick, (middle + 6.0)..=(middle + 9.0), Stroke::new(1.0, s::STRIPE));
    }
    if hot {
        p.circle_filled(pos2(x, middle), THUMB_RADIUS + 4.0, Color32::from_white_alpha(36));
    }
    p.circle(pos2(x, middle), THUMB_RADIUS, s::KEY, Stroke::new(1.0, s::KEY_LIP));
    chosen
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sign {
    Minus,
    Plus,
}

impl Sign {
    /// What it does, for screen readers.
    fn label(self) -> &'static str {
        match self {
            Sign::Minus => "Moins vite",
            Sign::Plus => "Plus vite",
        }
    }
}

/// « − » or « + », 10 px wide, 2 px thick, centred on `centre`.
fn paint_sign(p: &Painter, sign: Sign, centre: Pos2, color: Color32) {
    p.rect_filled(Rect::from_center_size(centre, vec2(10.0, 2.0)), 1, color);
    if sign == Sign::Plus {
        p.rect_filled(Rect::from_center_size(centre, vec2(2.0, 10.0)), 1, color);
    }
}

/// A « − » or « + » button; at a bound it is dimmed and does nothing. Returns `true` when clicked.
fn step_button(ui: &Ui, p: &Painter, rect: Rect, id: Id, sign: Sign, enabled: bool, framed: bool) -> bool {
    let radius = if framed { 6 } else { 5 };
    if !enabled {
        let response = ui.interact(rect, id, Sense::hover());
        response.widget_info(move || WidgetInfo::labeled(Role::Button, false, sign.label()));
        if framed {
            p.rect_stroke(rect, radius, Stroke::new(1.0, s::SOON_STRIPE), StrokeKind::Inside);
        }
        paint_sign(p, sign, rect.center(), s::SOON_TEXT);
        return false;
    }
    let response = parts::clickable(ui, rect, id, sign.label(), radius);
    let fill = parts::state_fill(&response, s::CONTROL, s::CONTROL_HOVER, s::CONTROL_PRESS);
    let stroke = if framed { Stroke::new(1.0, s::CONTROL_EDGE) } else { Stroke::NONE };
    p.rect(rect, radius, fill, stroke, StrokeKind::Inside);
    paint_sign(p, sign, rect.center(), s::TEXT);
    response.clicked()
}

/// Track B: « − », the value, « + », in a frame like the button selector's.
fn stepper(ui: &Ui, p: &Painter, rect: Rect, id: Id, multi: &MultiClick) -> Option<u8> {
    p.rect(rect, 7, s::PANEL, Stroke::new(1.0, s::EDGE), StrokeKind::Inside);
    let inner = rect.shrink(3.0);
    let width = 40.0;
    let minus = Rect::from_min_size(inner.min, vec2(width, inner.height()));
    let plus = Rect::from_min_size(pos2(inner.right() - width, inner.top()), vec2(width, inner.height()));
    let mut chosen = None;
    if step_button(ui, p, minus, id.with("moins"), Sign::Minus, multi.rate > multi.min, false) {
        chosen = Some(multi.rate - 1);
    }
    if step_button(ui, p, plus, id.with("plus"), Sign::Plus, multi.rate < multi.max, false) {
        chosen = Some(multi.rate + 1);
    }
    let galley = text::line(p, &rate_text(multi.rate), 14.0, s::BOLD, s::TEXT, inner.width() - 2.0 * width);
    text::centred(p, &galley, inner.center().x, inner.top(), inner.height());
    chosen
}

/// Track C's « − » / « + »: framed square buttons, like « Changer ».
fn small_step(ui: &Ui, p: &Painter, rect: Rect, id: Id, sign: Sign, enabled: bool) -> bool {
    step_button(ui, p, rect, id, sign, enabled, true)
}

/// Segments like the button selector's (`parts::selector`), with other words; returns the one
/// clicked, if it is not already selected.
fn segments(ui: &Ui, p: &Painter, rect: Rect, id: Id, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    p.rect(rect, 7, s::PANEL, Stroke::new(1.0, s::EDGE), StrokeKind::Inside);
    let inner = rect.shrink(3.0);
    let gap = 2.0;
    let count = labels.len() as f32;
    let width = (inner.width() - (count - 1.0) * gap) / count;
    let mut chosen = None;
    for (k, label) in labels.iter().enumerate() {
        let start = inner.left() + k as f32 * (width + gap);
        let segment = Rect::from_x_y_ranges(start.round()..=(start + width).round(), inner.y_range());
        let is_selected = selected == Some(k);
        let response = parts::clickable(ui, segment, id.with(k), label, 5);
        if response.clicked() && !is_selected {
            chosen = Some(k);
        }
        let (fill, weight, ink) = if is_selected {
            (s::KEY, s::BOLD, s::KEY_INK)
        } else if response.hovered() {
            (s::SEGMENT_HOVER, s::MEDIUM, s::SEGMENT_TEXT)
        } else {
            (Color32::TRANSPARENT, s::MEDIUM, s::SEGMENT_TEXT)
        };
        p.rect_filled(segment, 5, fill);
        let galley = text::line(p, label, 14.0, weight, ink, f32::INFINITY);
        let check = if is_selected { 11.0 + 4.0 } else { 0.0 };
        let left = (segment.center().x - (check + galley.size().x) / 2.0).round();
        if is_selected {
            let icon = Rect::from_min_size(pos2(left, segment.center().y - 5.5), vec2(11.0, 11.0));
            icons::paint(p, Icon::Check, icon, ink);
        }
        text::put(p, &galley, left + check, segment.top(), segment.height());
    }
    chosen
}

// The mouse pointer of view 3 --------------------------------------------------------------------

/// The arrow of the system's pointer, its tip at `tip`: white, outlined in black. Not part of the
/// HUD: it shows where the mouse is.
pub fn paint_pointer(p: &Painter, tip: Pos2) {
    let at = |x: f32, y: f32| tip + vec2(x, y);
    // Head: a triangle whose lower edge runs from (0, 16) to (11.3, 11.3). Tail: a bar leaving that
    // edge between A and B, going down to the right. Filled apart (egui fills convex shapes only),
    // the tail reaching a little into the head so that no seam shows, then outlined as one.
    let (a, b) = (at(3.6, 14.5), at(6.8, 13.17));
    let down = vec2(0.419, 0.908);
    let (c, d) = (b + 6.0 * down, a + 6.0 * down);
    let head = vec![at(0.0, 0.0), at(0.0, 16.0), at(11.3, 11.3)];
    let tail = vec![a - 1.5 * down, b - 1.5 * down, c, d];
    p.add(Shape::convex_polygon(head, Color32::WHITE, Stroke::NONE));
    p.add(Shape::convex_polygon(tail, Color32::WHITE, Stroke::NONE));
    let outline = vec![at(0.0, 0.0), at(0.0, 16.0), a, d, c, b, at(11.3, 11.3)];
    p.add(PathShape::closed_line(outline, Stroke::new(1.0, Color32::BLACK)));
}
