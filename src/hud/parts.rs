//! Building blocks of the HUD: clickable areas, keycap, button selector, small buttons, speed
//! slider, capture zone and message box. Each block paints exactly the mock-up measures (variant 2;
//! for the slider, track A of the « Multi-clic » preview, D-26).

use eframe::egui::{
    Color32, CornerRadius, CursorIcon, EventFilter, Id, Key, Painter, Rect, Response, Role, Sense, Shape, Stroke,
    StrokeKind, Ui, WidgetInfo, pos2, vec2,
};

use super::icons::{self, Icon};
use super::style as s;
use super::text;
use crate::contract::{ClickRate, MacroId, Message, MessageKind, MessageLevel, MouseButton};

/// Clickable area: hand cursor on hover, accessibility label, keyboard focus ring.
pub fn clickable(ui: &Ui, rect: Rect, id: Id, label: &str, radius: u8) -> Response {
    let response = ui.interact(rect, id, Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let label = label.to_owned();
    response.widget_info(move || WidgetInfo::labeled(Role::Button, true, &label));
    if response.has_focus() {
        let ring = Stroke::new(2.0, s::FOCUS);
        ui.painter().rect_stroke(rect.expand(2.0), radius.saturating_add(2), ring, StrokeKind::Inside);
    }
    response
}

/// Background of a button for its current state.
pub fn state_fill(response: &Response, idle: Color32, hover: Color32, press: Color32) -> Color32 {
    if response.is_pointer_button_down_on() {
        press
    } else if response.hovered() {
        hover
    } else {
        idle
    }
}

/// Background of a full-width block (row, settings, message); the last block follows the
/// rounded corners of the panel.
pub fn block(p: &Painter, rect: Rect, fill: Color32, line: Color32, last: bool) {
    if last {
        let corners = CornerRadius { sw: s::INNER_RADIUS, se: s::INNER_RADIUS, ..CornerRadius::ZERO };
        p.rect_filled(rect, corners, fill);
        // The line still shows above the border, cut by the rounded corners.
        let x = rect.x_range();
        p.hline((x.min + 4.0)..=(x.max - 4.0), rect.bottom() - 0.5, Stroke::new(1.0, line));
    } else {
        p.rect_filled(rect, 0, fill);
        p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, line));
    }
}

/// Coloured stripe on the left of a row. In the last block its bottom follows the panel corner.
pub fn stripe(p: &Painter, rect: Rect, color: Color32, block_bottom: f32, last: bool) {
    if !last {
        p.rect_filled(rect, 0, color);
        return;
    }
    let r = f32::from(s::INNER_RADIUS);
    let (cx, cy) = (rect.left() + r, block_bottom - r);
    let arc_y = |x: f32| (cy + (r * r - (cx - x).powi(2)).max(0.0).sqrt()).min(rect.bottom());
    let mut points = vec![rect.left_top(), rect.right_top()];
    let steps = 6;
    for k in 0..=steps {
        let x = rect.right() - rect.width() * k as f32 / steps as f32;
        points.push(pos2(x, arc_y(x)));
    }
    p.add(Shape::convex_polygon(points, color, Stroke::NONE));
}

// Keycap ---------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyLook {
    Normal,
    /// On the green row of an active macro.
    OnGreen,
    /// Macro « À venir ».
    Off,
    /// Waiting for a key: dashed outline and « … ».
    Waiting,
}

pub const KEY_HEIGHT: f32 = 24.0;
const KEY_SIZE: f32 = 12.0;

/// Width of a keycap showing `label`, at most `max` (a longer label is cut with « … »).
pub fn keycap_width(label: &str, look: KeyLook, max: f32) -> f32 {
    let min = if look == KeyLook::Waiting { 40.0 } else { 30.0 };
    (text::width(label, KEY_SIZE, s::EXTRABOLD) + 12.0).ceil().max(min).min(max)
}

/// A key of the keyboard: light face, darker 3 px lip at the bottom.
pub fn keycap(p: &Painter, rect: Rect, label: &str, look: KeyLook) {
    let radius = CornerRadius::same(6);
    let max_text = rect.width() - 12.0;
    if look == KeyLook::Waiting {
        dashed_rounded_rect(p, rect.shrink(1.0), 5.0, Stroke::new(2.0, s::INFO));
        let galley = text::line(p, label, KEY_SIZE, s::EXTRABOLD, s::INFO, max_text);
        text::centred(p, &galley, rect.center().x, rect.top(), rect.height());
        return;
    }
    let (face, lip, ink) = match look {
        KeyLook::OnGreen => (Color32::WHITE, s::ON_LINE, s::KEY_INK),
        KeyLook::Off => (s::KEY_OFF, s::KEY_OFF_LIP, s::KEY_OFF_INK),
        _ => (s::KEY, s::KEY_LIP, s::KEY_INK),
    };
    p.rect_filled(rect, radius, lip);
    let face_rect = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 3.0));
    p.rect_filled(face_rect, radius, face);
    let galley = text::line(p, label, KEY_SIZE, s::EXTRABOLD, ink, max_text);
    text::centred(p, &galley, rect.center().x, face_rect.top(), face_rect.height());
}

/// Dashed outline following a rounded rectangle (centre line of the stroke).
fn dashed_rounded_rect(p: &Painter, rect: Rect, radius: f32, stroke: Stroke) {
    let mut path = Vec::new();
    let corners = [
        (rect.right() - radius, rect.top() + radius, -90.0_f32),
        (rect.right() - radius, rect.bottom() - radius, 0.0),
        (rect.left() + radius, rect.bottom() - radius, 90.0),
        (rect.left() + radius, rect.top() + radius, 180.0),
    ];
    for (cx, cy, start) in corners {
        for k in 0..=6 {
            let angle = (start + 90.0 * k as f32 / 6.0).to_radians();
            path.push(pos2(cx + radius * angle.cos(), cy + radius * angle.sin()));
        }
    }
    path.push(path[0]);
    p.extend(Shape::dashed_line(&path, stroke, 4.0, 3.0));
}

// Small buttons --------------------------------------------------------------------------------

/// Text button without frame (title bar): icon, 4 px, text.
pub fn bar_button_width(label: &str) -> f32 {
    (6.0 + 10.0 + 4.0 + text::width(label, 12.0, s::SEMIBOLD) + 6.0).ceil()
}

pub fn bar_button(ui: &Ui, p: &Painter, rect: Rect, id: Id, icon: Icon, label: &str) -> Response {
    let response = clickable(ui, rect, id, label, 5);
    p.rect_filled(rect, 5, state_fill(&response, Color32::TRANSPARENT, s::HOVER, s::PRESS));
    let icon_rect = Rect::from_min_size(pos2(rect.left() + 6.0, rect.center().y - 5.0), vec2(10.0, 10.0));
    icons::paint(p, icon, icon_rect, s::TEXT_BUTTON);
    text::draw(p, label, 12.0, s::SEMIBOLD, s::TEXT_BUTTON, rect.left() + 20.0, rect.top(), rect.height());
    response
}

/// Framed button (« Changer », « Annuler »): 26 px high, 12 px semibold.
pub fn small_button_width(label: &str) -> f32 {
    (9.0 + text::width(label, 12.0, s::SEMIBOLD) + 9.0).ceil()
}

pub fn small_button(ui: &Ui, p: &Painter, rect: Rect, id: Id, label: &str) -> Response {
    let response = clickable(ui, rect, id, label, 6);
    let fill = state_fill(&response, s::CONTROL, s::CONTROL_HOVER, s::CONTROL_PRESS);
    p.rect(rect, 6, fill, Stroke::new(1.0, s::CONTROL_EDGE), StrokeKind::Inside);
    let galley = text::line(p, label, 12.0, s::SEMIBOLD, s::TEXT, f32::INFINITY);
    text::centred(p, &galley, rect.center().x, rect.top(), rect.height());
    response
}

/// Square icon button (▶ / ■, settings arrow, unfold).
pub struct IconButton {
    pub icon: Icon,
    pub icon_size: f32,
    pub ink: Color32,
    pub fill: Color32,
    pub fill_hover: Color32,
    pub fill_press: Color32,
    pub edge: Option<Color32>,
    pub radius: u8,
}

pub fn icon_button(ui: &Ui, p: &Painter, rect: Rect, id: Id, look: &IconButton, label: &str) -> Response {
    let response = clickable(ui, rect, id, label, look.radius);
    let fill = state_fill(&response, look.fill, look.fill_hover, look.fill_press);
    let stroke = look.edge.map_or(Stroke::NONE, |c| Stroke::new(1.0, c));
    p.rect(rect, look.radius, fill, stroke, StrokeKind::Inside);
    let icon_rect = Rect::from_center_size(rect.center(), vec2(look.icon_size, look.icon_size));
    icons::paint(p, look.icon, icon_rect, look.ink);
    response
}

// Button selector (Gauche / Droit / Milieu) ----------------------------------------------------

pub const SELECTOR_HEIGHT: f32 = 30.0;

/// Three segments; returns the button clicked, if it is not already selected.
pub fn selector(ui: &Ui, p: &Painter, rect: Rect, id: Id, selected: MouseButton) -> Option<MouseButton> {
    p.rect(rect, 7, s::PANEL, Stroke::new(1.0, s::EDGE), StrokeKind::Inside);
    let inner = rect.shrink(3.0);
    let gap = 2.0;
    let width = (inner.width() - 2.0 * gap) / 3.0;
    let mut chosen = None;
    for (k, button) in MouseButton::ALL.into_iter().enumerate() {
        let start = inner.left() + k as f32 * (width + gap);
        let segment = Rect::from_x_y_ranges(start.round()..=(start + width).round(), inner.y_range());
        let is_selected = button == selected;
        let response = clickable(ui, segment, id.with(k), button.label(), 5);
        if response.clicked() && !is_selected {
            chosen = Some(button);
        }
        let (fill, weight, ink) = if is_selected {
            (s::KEY, s::BOLD, s::KEY_INK)
        } else if response.hovered() {
            (s::SEGMENT_HOVER, s::MEDIUM, s::SEGMENT_TEXT)
        } else {
            (Color32::TRANSPARENT, s::MEDIUM, s::SEGMENT_TEXT)
        };
        p.rect_filled(segment, 5, fill);
        let galley = text::line(p, button.label(), 14.0, weight, ink, f32::INFINITY);
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

// Speed of « Multi-clic »: slider from 1 to 15 (D-26, track A of maquettes/multi-clic) -----------

/// The slider's line: the bounds on each side, the track, the thumb.
pub const SLIDER_HEIGHT: f32 = 24.0;
const THUMB_RADIUS: f32 = 8.0;

/// The slider's track in its `line`, and the centre of the thumb for each speed: the thumb stays
/// inside the track at both ends, one notch per speed.
fn slider_track(line: Rect) -> (Rect, impl Fn(ClickRate) -> f32) {
    let low = text::width(&ClickRate::MIN.per_second().to_string(), 12.0, s::MEDIUM);
    let high = text::width(&ClickRate::MAX.per_second().to_string(), 12.0, s::MEDIUM);
    let middle = line.center().y;
    let track =
        Rect::from_x_y_ranges((line.left() + low + 8.0)..=(line.right() - high - 8.0), (middle - 3.0)..=(middle + 3.0));
    let (min, max) = (ClickRate::MIN.per_second() as f32, ClickRate::MAX.per_second() as f32);
    let (start, end) = (track.left() + THUMB_RADIUS, track.right() - THUMB_RADIUS);
    (track, move |rate: ClickRate| (start + (end - start) * (rate.per_second() as f32 - min) / (max - min)).round())
}

/// Centre of the thumb for `rate`, in a slider drawn in `line`.
#[cfg(test)]
pub fn slider_thumb(line: Rect, rate: ClickRate) -> eframe::egui::Pos2 {
    let (_, x_of) = slider_track(line);
    pos2(x_of(rate), line.center().y)
}

/// Every speed, slowest first.
fn rates() -> impl Iterator<Item = ClickRate> {
    (ClickRate::MIN.per_second()..=ClickRate::MAX.per_second()).map(ClickRate::new)
}

/// « 1 », the track with one notch per speed, « 15 »; the light part and the thumb show `rate`.
/// A click or a drag on the track picks the nearest speed; with the keyboard focus, the arrows move
/// it by one, Début and Fin to the bounds. Returns the speed chosen, if it changed.
pub fn rate_slider(ui: &Ui, p: &Painter, line: Rect, id: Id, rate: ClickRate) -> Option<ClickRate> {
    let (track, x_of) = slider_track(line);
    let middle = line.center().y;
    let bounds = [ClickRate::MIN, ClickRate::MAX].map(|r| r.per_second().to_string());
    text::draw(p, &bounds[0], 12.0, s::MEDIUM, s::TEXT_2, line.left(), line.top(), line.height());
    let high = text::line(p, &bounds[1], 12.0, s::MEDIUM, s::TEXT_2, f32::INFINITY);
    text::put(p, &high, line.right() - high.size().x, line.top(), line.height());

    let hit = Rect::from_x_y_ranges(track.x_range(), line.y_range());
    let response = ui.interact(hit, id, Sense::click_and_drag());
    let hot = response.hovered() || response.dragged();
    if hot {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let value = f64::from(rate.per_second());
    response.widget_info(move || WidgetInfo::slider(true, value, "Rapidité, en clics par seconde"));
    let mut chosen = None;
    if (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        chosen = rates().min_by(|&a, &b| (x_of(a) - pointer.x).abs().total_cmp(&(x_of(b) - pointer.x).abs()));
    }
    if response.has_focus() {
        // The arrows change the speed instead of moving the focus.
        let filter = EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() };
        ui.memory_mut(|m| m.set_focus_lock_filter(id, filter));
        let step = |key: Key| ui.input(|i| i.key_pressed(key));
        let current = rate.per_second();
        if step(Key::ArrowLeft) || step(Key::ArrowDown) {
            chosen = Some(ClickRate::new(current.saturating_sub(1)));
        } else if step(Key::ArrowRight) || step(Key::ArrowUp) {
            chosen = Some(ClickRate::new(current + 1));
        } else if step(Key::Home) {
            chosen = Some(ClickRate::MIN);
        } else if step(Key::End) {
            chosen = Some(ClickRate::MAX);
        }
    }

    p.rect_filled(track, 3, s::CONTROL);
    let x = x_of(rate);
    p.rect_filled(Rect::from_x_y_ranges(track.left()..=x, track.y_range()), 3, s::KEY);
    for notch in rates() {
        p.vline(x_of(notch) + 0.5, (middle + 6.0)..=(middle + 9.0), Stroke::new(1.0, s::STRIPE));
    }
    let thumb = pos2(x, middle);
    if hot {
        p.circle_filled(thumb, THUMB_RADIUS + 4.0, Color32::from_white_alpha(36));
    }
    p.circle(thumb, THUMB_RADIUS, s::KEY, Stroke::new(1.0, s::KEY_LIP));
    if response.has_focus() {
        p.circle_stroke(thumb, THUMB_RADIUS + 3.0, Stroke::new(2.0, s::FOCUS));
    }
    chosen.filter(|&new| new != rate)
}

// Shortcut capture -----------------------------------------------------------------------------

pub const CAPTURE_HEIGHT: f32 = 78.0;

/// « Appuie sur une touche… / Échap pour annuler ». Returns `true` when « Annuler » is clicked.
pub fn capture_zone(ui: &Ui, p: &Painter, rect: Rect, id: Id) -> bool {
    p.rect(rect, 8, s::INFO_BG, Stroke::new(1.0, s::INFO), StrokeKind::Inside);
    let inner = rect.shrink(11.0);
    let key = Rect::from_min_size(inner.min, vec2(keycap_width("…", KeyLook::Waiting, 40.0), KEY_HEIGHT));
    keycap(p, key, "…", KeyLook::Waiting);
    let left = key.right() + 10.0;
    text::draw(p, "Appuie sur une touche…", 14.0, s::SEMIBOLD, s::TEXT, left, key.top(), KEY_HEIGHT);
    let row_top = key.bottom() + 6.0;
    text::draw(p, "Échap pour annuler", 12.0, s::REGULAR, s::INFO_TEXT, inner.left(), row_top, 26.0);
    let width = small_button_width("Annuler");
    let button = Rect::from_min_size(pos2(inner.right() - width, row_top), vec2(width, 26.0));
    small_button(ui, p, button, id.with("annuler"), "Annuler").clicked()
}

// Messages -------------------------------------------------------------------------------------

/// A message ready to paint: its level gives the colours and the icon.
pub struct MessageBox {
    pub level: MessageLevel,
    pub title: &'static str,
    pub lines: Vec<String>,
    /// « Raccourci refusé »: the macro « Choisir une autre touche » asks a new hotkey for.
    pub retry: Option<MacroId>,
}

/// Title above a message of the contract: its level in words (never shown by colour alone).
pub fn level_title(level: MessageLevel) -> &'static str {
    match level {
        MessageLevel::Info => "Information",
        MessageLevel::Warning => "Attention",
        MessageLevel::Error => "Erreur",
    }
}

/// The button of a « Raccourci refusé » message.
pub const RETRY_LABEL: &str = "Choisir une autre touche";

/// Text column of a message box `width` wide: 1 px border, 10 px padding, 18 px icon, 10 px
/// gap, 12 px padding, 1 px border.
fn text_width(width: f32) -> f32 {
    width - 52.0
}

/// Splits « Alt+Tab est déjà pris par Windows. F8 reste actif. » before its last sentence, which then
/// starts its own line, as on the mock-up.
fn last_sentence_apart(text: &str) -> Vec<&str> {
    match text.trim_end().rfind(". ") {
        Some(end) => vec![&text[..=end], text[end + 2..].trim_start()],
        None => vec![text],
    }
}

impl MessageBox {
    pub fn new(level: MessageLevel, title: &'static str, text: &str, width: f32) -> Self {
        MessageBox { level, title, lines: text::wrap(text, 12.0, s::REGULAR, text_width(width)), retry: None }
    }

    /// A message of the contract. « Raccourci refusé » (D-16): amber box with that title, the text
    /// with its last sentence on its own line, and « Choisir une autre touche » when `retry` names
    /// a macro that can take a new hotkey.
    pub fn from_contract(message: &Message, retry: Option<MacroId>, width: f32) -> Self {
        match &message.kind {
            MessageKind::HotkeyRefused { .. } => {
                let lines = last_sentence_apart(&message.text)
                    .into_iter()
                    .flat_map(|part| text::wrap(part, 12.0, s::REGULAR, text_width(width)))
                    .collect();
                MessageBox { level: MessageLevel::Warning, title: "Raccourci refusé", lines, retry }
            }
            MessageKind::General => MessageBox::new(message.level, level_title(message.level), &message.text, width),
        }
    }

    pub fn height(&self) -> f32 {
        let button = if self.retry.is_some() { 8.0 + 28.0 } else { 0.0 };
        1.0 + 10.0 + s::LINE_14 + 3.0 + self.lines.len() as f32 * s::LINE_MESSAGE + button + 12.0 + 1.0
    }
}

struct Colours {
    fill: Color32,
    edge: Color32,
    icon: Icon,
    title: Color32,
    text: Color32,
    button: Color32,
    button_hover: Color32,
}

fn colours(level: MessageLevel) -> Colours {
    let base =
        |fill, edge, icon, title, text| Colours { fill, edge, icon, title, text, button: fill, button_hover: fill };
    match level {
        MessageLevel::Info => base(s::INFO_BG, s::INFO, Icon::Info, s::TEXT, s::INFO_TEXT),
        MessageLevel::Warning => Colours {
            button: s::WARN_BUTTON,
            button_hover: s::WARN_BUTTON_HOVER,
            ..base(s::WARN_BG, s::WARN, Icon::Warning, s::WARN_TITLE, s::WARN_TEXT)
        },
        MessageLevel::Error => base(s::ERROR_BG, s::ERROR, Icon::Error, s::ERROR_TITLE, s::ERROR_TEXT),
    }
}

/// Message box: icon, title in bold, the text below (already wrapped), and the « Choisir une autre
/// touche » button of a refused hotkey. Returns `true` when that button is clicked.
pub fn message(ui: &Ui, p: &Painter, rect: Rect, id: Id, message: &MessageBox) -> bool {
    let look = colours(message.level);
    p.rect(rect, 8, look.fill, Stroke::new(1.0, look.edge), StrokeKind::Inside);
    let icon = Rect::from_min_size(rect.min + vec2(11.0, 12.0), vec2(18.0, 18.0));
    icons::paint(p, look.icon, icon, look.edge);
    let left = icon.right() + 10.0;
    let max_width = text_width(rect.width());
    let mut top = rect.top() + 11.0;
    let title = text::line(p, message.title, 14.0, s::BOLD, look.title, max_width);
    text::put(p, &title, left, top, s::LINE_14);
    top += s::LINE_14 + 3.0;
    for line in &message.lines {
        // A few points of slack: the line breaks were measured, the painter must not cut again.
        let galley = text::line(p, line, 12.0, s::REGULAR, look.text, max_width + 4.0);
        text::put(p, &galley, left, top, s::LINE_MESSAGE);
        top += s::LINE_MESSAGE;
    }
    if message.retry.is_none() {
        return false;
    }
    let width = (10.0 + text::width(RETRY_LABEL, 12.0, s::SEMIBOLD) + 10.0).ceil();
    let button = Rect::from_min_size(pos2(left, top + 8.0), vec2(width, 28.0));
    let response = clickable(ui, button, id.with("autre-touche"), RETRY_LABEL, 6);
    let fill = state_fill(&response, look.button, look.button_hover, look.button_hover);
    p.rect(button, 6, fill, Stroke::new(1.0, look.edge), StrokeKind::Inside);
    text::draw(p, RETRY_LABEL, 12.0, s::SEMIBOLD, look.title, button.left() + 10.0, button.top(), 28.0);
    response.clicked()
}
