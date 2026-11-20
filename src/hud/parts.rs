//! Building blocks of the HUD: clickable areas, keycap, button selector, small buttons, capture
//! zone and message. Each block paints exactly the mock-up measures (variant 2).

use egui::{
    Color32, CornerRadius, CursorIcon, Id, Painter, Pos2, Rect, Response, Role, Sense, Shape,
    Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2,
};

use super::icons::{self, Icon};
use super::style as s;
use super::text;
use super::view::{Level, Message, MouseButton};

/// Clickable area: hand cursor on hover, accessibility label, keyboard focus ring.
pub fn clickable(ui: &Ui, rect: Rect, id: Id, label: &str, radius: u8) -> Response {
    let response = ui.interact(rect, id, Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let label = label.to_owned();
    response.widget_info(move || WidgetInfo::labeled(Role::Button, true, &label));
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            radius.saturating_add(2),
            Stroke::new(2.0, s::FOCUS),
            StrokeKind::Inside,
        );
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

/// Background of a full-width block (row, settings); the last block follows the rounded corners
/// of the panel and has no bottom line.
pub fn block(p: &Painter, rect: Rect, fill: Color32, line: Color32, last: bool) {
    if last {
        let corners = CornerRadius {
            sw: s::INNER_RADIUS,
            se: s::INNER_RADIUS,
            ..CornerRadius::ZERO
        };
        p.rect_filled(rect, corners, fill);
        // The line still shows above the border, cut by the rounded corners.
        let x = rect.x_range();
        p.hline(
            (x.min + 4.0)..=(x.max - 4.0),
            rect.bottom() - 0.5,
            Stroke::new(1.0, line),
        );
    } else {
        p.rect_filled(rect, 0, fill);
        p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, line));
    }
}

/// Coloured stripe on the left of a row. In the last row its bottom follows the panel corner.
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

pub fn keycap_width(p: &Painter, label: &str, look: KeyLook) -> f32 {
    let min = if look == KeyLook::Waiting { 40.0 } else { 30.0 };
    (text::width(p, label, KEY_SIZE, s::EXTRABOLD) + 12.0)
        .ceil()
        .max(min)
}

/// A key of the keyboard: light face, darker 3 px lip at the bottom.
pub fn keycap(p: &Painter, rect: Rect, label: &str, look: KeyLook) {
    let radius = CornerRadius::same(6);
    if look == KeyLook::Waiting {
        dashed_rounded_rect(p, rect.shrink(1.0), 5.0, Stroke::new(2.0, s::INFO));
        let galley = text::line(p, label, KEY_SIZE, s::EXTRABOLD, s::INFO, f32::INFINITY);
        let left = (rect.center().x - galley.size().x / 2.0).round();
        text::put(p, &galley, left, rect.top(), rect.height());
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
    let galley = text::line(p, label, KEY_SIZE, s::EXTRABOLD, ink, f32::INFINITY);
    let left = (rect.center().x - galley.size().x / 2.0).round();
    text::put(p, &galley, left, face_rect.top(), face_rect.height());
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
pub fn bar_button_width(p: &Painter, label: &str) -> f32 {
    (6.0 + 10.0 + 4.0 + text::width(p, label, 12.0, s::SEMIBOLD) + 6.0).ceil()
}

pub fn bar_button(ui: &Ui, p: &Painter, rect: Rect, id: Id, icon: Icon, label: &str) -> Response {
    let response = clickable(ui, rect, id, label, 5);
    let fill = state_fill(&response, Color32::TRANSPARENT, s::HOVER, s::PRESS);
    p.rect_filled(rect, 5, fill);
    let icon_rect = Rect::from_min_size(
        pos2(rect.left() + 6.0, rect.center().y - 5.0),
        vec2(10.0, 10.0),
    );
    icons::paint(p, icon, icon_rect, s::TEXT_BUTTON);
    text::draw(
        p,
        label,
        12.0,
        s::SEMIBOLD,
        s::TEXT_BUTTON,
        rect.left() + 20.0,
        rect.top(),
        rect.height(),
    );
    response
}

/// Framed button (« Changer », « Annuler »): 26 px high, 12 px semibold.
pub fn small_button_width(p: &Painter, label: &str) -> f32 {
    (9.0 + text::width(p, label, 12.0, s::SEMIBOLD) + 9.0).ceil()
}

pub fn small_button(ui: &Ui, p: &Painter, rect: Rect, id: Id, label: &str) -> Response {
    let response = clickable(ui, rect, id, label, 6);
    let fill = state_fill(&response, s::CONTROL, s::CONTROL_HOVER, s::CONTROL_PRESS);
    p.rect(
        rect,
        6,
        fill,
        Stroke::new(1.0, s::CONTROL_EDGE),
        StrokeKind::Inside,
    );
    let galley = text::line(p, label, 12.0, s::SEMIBOLD, s::TEXT, f32::INFINITY);
    let left = (rect.center().x - galley.size().x / 2.0).round();
    text::put(p, &galley, left, rect.top(), rect.height());
    response
}

/// Square icon button (toggle ▶ / ■, settings arrow).
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

pub fn icon_button(
    ui: &Ui,
    p: &Painter,
    rect: Rect,
    id: Id,
    look: &IconButton,
    label: &str,
) -> Response {
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
pub fn selector(
    ui: &Ui,
    p: &Painter,
    rect: Rect,
    id: Id,
    selected: MouseButton,
) -> Option<MouseButton> {
    p.rect(
        rect,
        7,
        s::PANEL,
        Stroke::new(1.0, s::EDGE),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(3.0);
    let gap = 2.0;
    let width = (inner.width() - 2.0 * gap) / 3.0;
    let mut chosen = None;
    for (k, button) in MouseButton::ALL.into_iter().enumerate() {
        let start = inner.left() + k as f32 * (width + gap);
        let segment =
            Rect::from_x_y_ranges(start.round()..=(start + width).round(), inner.y_range());
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

// Shortcut capture -----------------------------------------------------------------------------

pub const CAPTURE_HEIGHT: f32 = 78.0;

/// « Appuie sur une touche… / Échap pour annuler ». Returns `true` when « Annuler » is clicked.
pub fn capture_zone(ui: &Ui, p: &Painter, rect: Rect, id: Id) -> bool {
    p.rect(
        rect,
        8,
        s::INFO_BG,
        Stroke::new(1.0, s::INFO),
        StrokeKind::Inside,
    );
    let inner = rect.shrink(11.0);
    let key_width = keycap_width(p, "…", KeyLook::Waiting);
    let key = Rect::from_min_size(inner.min, vec2(key_width, KEY_HEIGHT));
    keycap(p, key, "…", KeyLook::Waiting);
    text::draw(
        p,
        "Appuie sur une touche…",
        14.0,
        s::SEMIBOLD,
        s::TEXT,
        key.right() + 10.0,
        key.top(),
        KEY_HEIGHT,
    );
    let row_top = key.bottom() + 6.0;
    text::draw(
        p,
        "Échap pour annuler",
        12.0,
        s::REGULAR,
        s::INFO_TEXT,
        inner.left(),
        row_top,
        26.0,
    );
    let width = small_button_width(p, "Annuler");
    let button = Rect::from_min_size(pos2(inner.right() - width, row_top), vec2(width, 26.0));
    small_button(ui, p, button, id.with("annuler"), "Annuler").clicked()
}

// Messages -------------------------------------------------------------------------------------

struct MessageLook {
    fill: Color32,
    edge: Color32,
    icon: Icon,
    title: Color32,
    text: Color32,
    button: Color32,
    button_hover: Color32,
}

fn message_look(level: Level) -> MessageLook {
    match level {
        Level::Info => MessageLook {
            fill: s::INFO_BG,
            edge: s::INFO,
            icon: Icon::Info,
            title: s::TEXT,
            text: s::INFO_TEXT,
            button: s::INFO_BUTTON,
            button_hover: s::INFO_BUTTON,
        },
        Level::Warning => MessageLook {
            fill: s::WARN_BG,
            edge: s::WARN,
            icon: Icon::Warning,
            title: s::WARN_TITLE,
            text: s::WARN_TEXT,
            button: s::WARN_BUTTON,
            button_hover: s::WARN_BUTTON_HOVER,
        },
        Level::Error => MessageLook {
            fill: s::ERROR_BG,
            edge: s::ERROR,
            icon: Icon::Error,
            title: s::ERROR_TITLE,
            text: s::ERROR_TEXT,
            button: s::ERROR_BUTTON,
            button_hover: s::ERROR_BUTTON,
        },
    }
}

/// Text column of a message box `width` wide: 1 px border, 10 px padding, 18 px icon, 10 px gap,
/// 12 px padding, 1 px border.
fn message_text_width(width: f32) -> f32 {
    width - 52.0
}

pub const MESSAGE_ACTION: &str = "Choisir une autre touche";

pub fn message_height(p: &Painter, message: &Message, width: f32, with_action: bool) -> f32 {
    let detail = text::paragraph(
        p,
        &message.detail,
        12.0,
        s::REGULAR,
        Color32::WHITE,
        message_text_width(width),
        s::LINE_MESSAGE,
    );
    let action = if with_action { 8.0 + 28.0 } else { 0.0 };
    (1.0 + 10.0 + s::LINE_14 + 3.0 + detail.size().y + action + 12.0 + 1.0).ceil()
}

/// Message box (icon, bold title, explanation, optional action). Returns `true` when the action
/// « Choisir une autre touche » is clicked.
pub fn message(
    ui: &Ui,
    p: &Painter,
    rect: Rect,
    id: Id,
    message: &Message,
    with_action: bool,
) -> bool {
    let look = message_look(message.level);
    p.rect(
        rect,
        8,
        look.fill,
        Stroke::new(1.0, look.edge),
        StrokeKind::Inside,
    );
    let icon = Rect::from_min_size(rect.min + vec2(11.0, 12.0), vec2(18.0, 18.0));
    icons::paint(p, look.icon, icon, look.edge);
    let left = icon.right() + 10.0;
    let max_width = message_text_width(rect.width());
    let mut top = rect.top() + 11.0;
    let title = text::line(p, &message.title, 14.0, s::BOLD, look.title, max_width);
    text::put(p, &title, left, top, s::LINE_14);
    top += s::LINE_14 + 3.0;
    let detail = text::paragraph(
        p,
        &message.detail,
        12.0,
        s::REGULAR,
        look.text,
        max_width,
        s::LINE_MESSAGE,
    );
    p.galley(Pos2::new(left, top), detail.clone(), look.text);
    top += detail.size().y;
    if !with_action {
        return false;
    }
    top += 8.0;
    let width = (10.0 + text::width(p, MESSAGE_ACTION, 12.0, s::SEMIBOLD) + 10.0).ceil();
    let button = Rect::from_min_size(pos2(left, top.round()), vec2(width, 28.0));
    let response = clickable(ui, button, id.with("action"), MESSAGE_ACTION, 6);
    let fill = state_fill(&response, look.button, look.button_hover, look.button_hover);
    p.rect(
        button,
        6,
        fill,
        Stroke::new(1.0, look.edge),
        StrokeKind::Inside,
    );
    text::draw(
        p,
        MESSAGE_ACTION,
        12.0,
        s::SEMIBOLD,
        look.title,
        button.left() + 10.0,
        button.top(),
        28.0,
    );
    response.clicked()
}
