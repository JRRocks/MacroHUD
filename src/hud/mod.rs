//! The HUD: variant 2 « Liste compacte » of the mock-ups (decision D-12, `maquettes/variante-2.png`),
//! drawn with egui. A title bar, one line per macro (▶ button, state, hotkey on the right), the
//! settings of a macro unfolded under its line, the messages, and a small pill when folded.
//!
//! What the rest of MacroHUD uses (interface set by the "programmation" session):
//!
//! - [`setup`]: called once at start (installs the Inter font);
//! - [`draw`]: draws the HUD from a [`HudState`] and returns the [`HudAction`]s the user asked for;
//! - [`window_size`]: the window's inner size, in points, for a given state (the window follows it);
//! - [`TRANSPARENT`]: whether the window is transparent (rounded corners, folded pill);
//! - [`MESSAGE_DURATION`]: how long a message stays shown;
//! - [`window_icon`]: the window's icon (taskbar). If `src/hud/icone.ico` exists, it also becomes
//!   the icon of `macrohud.exe` (see `build.rs`).
//!
//! Everything the HUD shows comes from `crate::contract` (docs/CONTRAT-HUD.md); everything it asks
//! goes through [`HudAction`]. Between frames it only remembers which macro has its settings
//! unfolded. Text is measured with fonts of its own, so [`window_size`] and [`draw`] compute the
//! same layout. `cargo test` renders every situation to PNG, without a screen (`render.rs`).

mod icons;
mod parts;
mod style;
mod text;

#[cfg(test)]
mod render;

use std::cell::Cell;
use std::time::Duration;

use eframe::egui::{self, CursorIcon, Id, Painter, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::contract::{HudAction, HudState, MacroId, MacroOptions, MacroStatus, MacroView, MessageLevel, MouseButton};
use icons::Icon;
use parts::{IconButton, KEY_HEIGHT, KeyLook, MessageBox};
use style as s;

/// The window is transparent around the panel: rounded corners, and the folded pill.
pub const TRANSPARENT: bool = true;

/// How long a message stays shown: long enough to read two lines, and to cover the 3 s before
/// « Activer » presses the button.
pub const MESSAGE_DURATION: Duration = Duration::from_secs(6);

/// Called once, before the first frame: installs Inter.
pub fn setup(ctx: &egui::Context) {
    text::install(ctx);
}

/// The window's icon: no icon of its own yet (it would need its own mock-ups), so the default one.
pub fn window_icon() -> egui::IconData {
    egui::IconData::default()
}

/// The window's inner size, in points: the HUD, its 1 px black ring included.
pub fn window_size(state: &HudState) -> Vec2 {
    Layout::new(state, open_macro()).size
}

/// Draws the HUD at the top left of `ui`; returns what the user asked for during this frame.
pub fn draw(ui: &mut Ui, state: &HudState) -> Vec<HudAction> {
    paint(ui, state, false)
}

// Memory ---------------------------------------------------------------------------------------

thread_local! {
    /// The macro whose settings the user unfolded (one HUD per program).
    static OPEN: Cell<Option<MacroId>> = const { Cell::new(None) };
}

fn open_macro() -> Option<MacroId> {
    OPEN.with(Cell::get)
}

fn set_open_macro(id: Option<MacroId>) {
    OPEN.with(|open| open.set(id));
}

// Layout ---------------------------------------------------------------------------------------

/// Width of a settings block's content (12 px left padding, 8 px right).
const CONTENT_WIDTH: f32 = s::PANEL_WIDTH - 2.0 - 12.0 - 8.0;
/// Width of the message shown under the folded pill (as wide as the unfolded panel).
const BUBBLE_WIDTH: f32 = s::PANEL_WIDTH;
/// Widest keycap on a macro line and on the pill; a longer hotkey is cut with « … ».
const KEY_MAX_IN_ROW: f32 = 84.0;

/// The full-width blocks stacked under the title bar.
enum Block {
    /// Line of macro `i`.
    Row(usize),
    /// Why macro `i` cannot run.
    Notice(MessageBox),
    /// Settings of macro `i`, with the current message.
    Options(usize, Option<MessageBox>),
    /// The current message, when no settings are unfolded.
    Message(MessageBox),
}

/// Where everything goes; computed the same way by [`window_size`] and [`draw`].
struct Layout<'a> {
    state: &'a HudState,
    /// Macro whose settings show under its line.
    open: Option<usize>,
    blocks: Vec<(Block, f32)>,
    /// Folded: width of the pill (its border included, not its ring).
    chip_width: f32,
    /// Folded: the current message, under the pill.
    bubble: Option<MessageBox>,
    size: Vec2,
}

impl<'a> Layout<'a> {
    fn new(state: &'a HudState, remembered: Option<MacroId>) -> Self {
        let usable = |id: MacroId| state.macros.iter().position(|m| m.id == id && m.available);
        // Settings show while their macro waits for a hotkey, else when the user unfolded them.
        let open = state.capturing.and_then(usable).or_else(|| remembered.and_then(usable));
        let mut layout = Layout { state, open, blocks: Vec::new(), chip_width: 0.0, bubble: None, size: Vec2::ZERO };
        let message_box = |width: f32| {
            state.message.as_ref().map(|m| MessageBox::new(m.level, parts::level_title(m.level), &m.text, width))
        };
        if state.collapsed {
            layout.chip_width = chip_width(main_macro(state));
            layout.bubble = message_box(BUBBLE_WIDTH);
            let mut size = vec2(layout.chip_width + 2.0, s::CHIP_HEIGHT + 2.0);
            if let Some(bubble) = &layout.bubble {
                size.x = size.x.max(BUBBLE_WIDTH + 2.0);
                size.y += 6.0 + bubble.height() + 2.0;
            }
            layout.size = size;
            return layout;
        }
        for (i, view) in state.macros.iter().enumerate() {
            layout.blocks.push((Block::Row(i), s::ROW_HEIGHT));
            if let MacroStatus::Unavailable(reason) = &view.status {
                let notice = MessageBox::new(MessageLevel::Warning, "Macro indisponible", reason, CONTENT_WIDTH);
                let height = framed_height(&notice);
                layout.blocks.push((Block::Notice(notice), height));
            }
            if open == Some(i) {
                let message = message_box(CONTENT_WIDTH);
                let height = options_height(state.capturing == Some(view.id), message.as_ref());
                layout.blocks.push((Block::Options(i, message), height));
            }
        }
        if open.is_none()
            && let Some(message) = message_box(CONTENT_WIDTH)
        {
            let height = framed_height(&message);
            layout.blocks.push((Block::Message(message), height));
        }
        let content: f32 = layout.blocks.iter().map(|(_, height)| height).sum();
        // Ring, border, title bar, blocks, border, ring.
        layout.size = vec2(s::PANEL_WIDTH + 2.0, 1.0 + 1.0 + s::BAR_HEIGHT + content + 1.0 + 1.0);
        layout
    }
}

/// A message in its own block: 8 px above it, then 10 px and the block's line below.
fn framed_height(message: &MessageBox) -> f32 {
    8.0 + message.height() + 10.0 + 1.0
}

/// Settings: button selector, hotkey line (or the capture zone), message.
fn options_height(capturing: bool, message: Option<&MessageBox>) -> f32 {
    let selector = s::LINE_12 + 4.0 + parts::SELECTOR_HEIGHT + 10.0;
    let hotkey = if capturing { parts::CAPTURE_HEIGHT } else { 26.0 };
    let message = message.map_or(0.0, |m| 8.0 + m.height());
    8.0 + selector + hotkey + message + 10.0 + 1.0
}

/// The macro the folded pill shows: the first active one, else the first available one.
fn main_macro(state: &HudState) -> Option<&MacroView> {
    state
        .macros
        .iter()
        .find(|m| m.status == MacroStatus::Active)
        .or_else(|| state.macros.iter().find(|m| m.available))
        .or_else(|| state.macros.first())
}

/// Icon and word of the pill.
fn chip_state(view: &MacroView) -> (Icon, &'static str) {
    match view.status {
        MacroStatus::Active => (Icon::Dot, "Active"),
        MacroStatus::Stopped => (Icon::Stop, "Arrêtée"),
        MacroStatus::Unavailable(_) => (Icon::Warning, "Indisponible"),
    }
}

/// Width of the folded pill, its border included.
fn chip_width(main: Option<&MacroView>) -> f32 {
    let mut width = 1.0 + 6.0 + 12.0 + 6.0;
    if let Some(view) = main {
        let (_, word) = chip_state(view);
        width += 10.0 + 5.0 + text::width(word, 12.0, s::BOLD) + 2.0 + 6.0;
        width += parts::keycap_width(&view.hotkey, KeyLook::Normal, KEY_MAX_IN_ROW) + 6.0;
    }
    (width + 24.0 + 4.0 + 1.0).ceil()
}

/// The held button in one word, the selector's: « gauche », « droit », « milieu ».
fn button_word(view: &MacroView) -> &'static str {
    let MacroOptions::HoldClick { button } = view.options;
    match button {
        MouseButton::Left => "gauche",
        MouseButton::Right => "droit",
        MouseButton::Middle => "milieu",
    }
}

/// Drawing of a kind of macro, shown instead of the ▶ button while it is « à venir ».
fn kind_icon(view: &MacroView) -> Icon {
    match view.options {
        MacroOptions::HoldClick { .. } => Icon::Mouse,
    }
}

/// Icon, words and weight of the state line under a macro's name, as on the mock-up (D-16):
/// « Arrêtée · bouton gauche », « Active · gauche enfoncé ». During the 3 s after « Activer » the
/// engine keeps the macro « arrêtée » and counts down in a message, so « enfoncé » is always true.
fn state_line(view: &MacroView) -> (Option<Icon>, String, f32) {
    if !view.available {
        return (None, "À venir".to_owned(), s::REGULAR);
    }
    let button = button_word(view);
    match view.status {
        MacroStatus::Unavailable(_) => (Some(Icon::Warning), "Indisponible".to_owned(), s::SEMIBOLD),
        MacroStatus::Active => (Some(Icon::Dot), format!("Active · {button} enfoncé"), s::SEMIBOLD),
        MacroStatus::Stopped => (Some(Icon::Stop), format!("Arrêtée · bouton {button}"), s::REGULAR),
    }
}

// Painting -------------------------------------------------------------------------------------

/// Draws the HUD; `shadow` adds the drop shadow of the mock-up (renders only: in the window it
/// would need a transparent margin that swallows clicks around the HUD).
fn paint(ui: &mut Ui, state: &HudState, shadow: bool) -> Vec<HudAction> {
    text::new_frame();
    let layout = Layout::new(state, open_macro());
    let (rect, _) = ui.allocate_exact_size(layout.size, Sense::hover());
    let mut actions = Vec::new();
    if state.collapsed {
        paint_folded(ui, rect, &layout, shadow, &mut actions);
    } else {
        paint_panel(ui, rect, &layout, shadow, &mut actions);
    }
    actions
}

/// Area that moves the window when dragged; `label` names it for screen readers.
fn drag_area(ui: &Ui, rect: Rect, id: Id, label: String, actions: &mut Vec<HudAction>) {
    let response = ui.interact(rect, id, Sense::click_and_drag());
    response.widget_info(move || egui::WidgetInfo::labeled(egui::Role::TitleBar, true, &label));
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
    if response.drag_started() {
        actions.push(HudAction::StartDrag);
    }
}

fn paint_panel(ui: &Ui, rect: Rect, layout: &Layout<'_>, shadow: bool, actions: &mut Vec<HudAction>) {
    let p = ui.painter().clone();
    let panel = rect.shrink(1.0);
    if shadow {
        p.add(s::SHADOW.as_shape(panel, s::RADIUS));
    }
    p.rect_filled(panel, s::RADIUS, s::PANEL);
    let inner = panel.shrink(1.0);
    let id = ui.make_persistent_id("macrohud");
    let mut top = paint_bar(ui, &p, inner, id, layout.state, actions);
    let count = layout.blocks.len();
    for (k, (block, height)) in layout.blocks.iter().enumerate() {
        let last = k + 1 == count;
        let area = Rect::from_min_size(pos2(inner.left(), top), vec2(inner.width(), *height));
        match block {
            Block::Row(i) => paint_row(ui, &p, area, id.with(i), layout, *i, last, actions),
            Block::Options(i, message) => {
                paint_options(ui, &p, area, id.with(i), layout, *i, message.as_ref(), last, actions);
            }
            Block::Notice(message) | Block::Message(message) => {
                parts::block(&p, area, s::OPTIONS, s::ROW_LINE, last);
                let rect = Rect::from_min_size(area.min + vec2(12.0, 8.0), vec2(CONTENT_WIDTH, message.height()));
                parts::message(&p, rect, message);
            }
        }
        top += height;
    }
    // Border and ring last: blocks never cover them.
    p.rect_stroke(panel, s::RADIUS, Stroke::new(1.0, s::EDGE), StrokeKind::Inside);
    p.rect_stroke(rect, s::RADIUS + 1, Stroke::new(1.0, s::RING), StrokeKind::Inside);
}

/// Title bar: drag handle, name and version, « Replier », « Quitter ». Returns its bottom.
fn paint_bar(ui: &Ui, p: &Painter, inner: Rect, id: Id, state: &HudState, actions: &mut Vec<HudAction>) -> f32 {
    let bar = Rect::from_min_size(inner.min, vec2(inner.width(), s::BAR_HEIGHT - 1.0));
    // Registered first: the buttons registered after it win the clicks.
    drag_area(ui, bar, id.with("barre"), state.title(), actions);

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
    // A long version (« 0.1.0-dev » of a local build) is cut before « Replier », never under it.
    let version = text::line(p, state.version, 12.0, s::REGULAR, s::TEXT_2, fold.left() - 4.0 - x);
    text::put(p, &version, x, line_top, s::LINE_12);

    if parts::bar_button(ui, p, fold, id.with("replier"), Icon::Up, "Replier").clicked() {
        actions.push(HudAction::SetCollapsed(true));
    }
    if parts::bar_button(ui, p, quit, id.with("quitter"), Icon::Close, "Quitter").clicked() {
        actions.push(HudAction::Quit);
    }
    p.hline(inner.x_range(), bar.bottom() + 0.5, Stroke::new(1.0, s::BAR_LINE));
    inner.top() + s::BAR_HEIGHT
}

/// One macro line: ▶ / ■, name and state, hotkey, settings arrow.
#[expect(clippy::too_many_arguments, reason = "painting step with its layout inputs")]
fn paint_row(
    ui: &Ui,
    p: &Painter,
    block: Rect,
    id: Id,
    layout: &Layout<'_>,
    index: usize,
    last: bool,
    actions: &mut Vec<HudAction>,
) {
    let view = &layout.state.macros[index];
    let active = view.status == MacroStatus::Active;
    let unavailable = matches!(view.status, MacroStatus::Unavailable(_));
    let soon = !view.available;
    let open = layout.open == Some(index);
    let (fill, line, stripe) = if active {
        (s::ON, s::ON_LINE, s::ON_INK)
    } else if soon {
        (s::SOON_ROW, s::ROW_LINE, s::SOON_STRIPE)
    } else if unavailable {
        (s::ROW, s::ROW_LINE, s::WARN)
    } else {
        (s::ROW, s::ROW_LINE, s::STRIPE)
    };
    parts::block(p, block, fill, line, last);
    let content = Rect::from_min_size(block.min, vec2(block.width(), s::ROW_HEIGHT - 1.0));
    parts::stripe(p, Rect::from_min_size(content.min, vec2(4.0, content.height())), stripe, block.bottom(), last);
    let middle = content.center().y;

    // ▶ / ■ button; a macro « à venir » shows its kind instead, an unavailable one a dead button.
    let toggle = Rect::from_min_size(pos2(content.left() + 14.0, (middle - 16.0).round()), vec2(32.0, 32.0));
    if soon {
        icons::paint(p, kind_icon(view), Rect::from_center_size(toggle.center(), vec2(16.0, 16.0)), s::SOON_TEXT);
    } else if unavailable {
        p.rect_stroke(toggle, 8, Stroke::new(1.0, s::SOON_STRIPE), StrokeKind::Inside);
        icons::paint(p, Icon::Play, Rect::from_center_size(toggle.center(), vec2(13.0, 13.0)), s::SOON_TEXT);
    } else {
        let look = if active {
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
        let label = if active { "Arrêter" } else { "Activer" };
        if parts::icon_button(ui, p, toggle, id.with("basculer"), &look, label).clicked() {
            actions.push(if active { HudAction::Stop(view.id) } else { HudAction::Start(view.id) });
        }
    }

    // Settings arrow, then the hotkey, from the right.
    let arrow = Rect::from_min_size(pos2(content.right() - 6.0 - 24.0, (middle - 12.0).round()), vec2(24.0, 24.0));
    if soon {
        icons::paint(p, Icon::Down, Rect::from_center_size(arrow.center(), vec2(13.0, 13.0)), s::SOON_CHEVRON);
    } else {
        let look = IconButton {
            icon: if open { Icon::Up } else { Icon::Down },
            icon_size: 13.0,
            ink: if active { s::ON_INK } else { s::TEXT_BUTTON },
            fill: egui::Color32::TRANSPARENT,
            fill_hover: if active { s::ON_HOVER } else { s::ROW_HOVER },
            fill_press: if active { s::ON_HOVER } else { s::ROW_HOVER },
            edge: None,
            radius: 5,
        };
        let label = if open { "Masquer les réglages" } else { "Afficher les réglages" };
        if parts::icon_button(ui, p, arrow, id.with("reglages"), &look, label).clicked() {
            set_open_macro(if open { None } else { Some(view.id) });
        }
    }
    let key_look = if soon {
        KeyLook::Off
    } else if active {
        KeyLook::OnGreen
    } else {
        KeyLook::Normal
    };
    let key_width = parts::keycap_width(&view.hotkey, key_look, KEY_MAX_IN_ROW);
    let key_left = arrow.left() - 8.0 - key_width;
    let key = Rect::from_min_size(pos2(key_left, (middle - 12.0).round()), vec2(key_width, KEY_HEIGHT));
    parts::keycap(p, key, &view.hotkey, key_look);

    // Name, and the state line under it.
    let left = toggle.right() + 8.0;
    let max_width = key.left() - 8.0 - left;
    let (name_ink, state_ink) = if active {
        (s::ON_INK, s::ON_INK)
    } else if soon {
        (s::SOON_TEXT, s::SOON_TEXT)
    } else if unavailable {
        (s::TEXT, s::WARN_TEXT)
    } else {
        (s::TEXT, s::TEXT_2)
    };
    let text_top = content.top() + ((content.height() - (s::LINE_14 + 2.0 + s::LINE_12)) / 2.0).round();
    let name = text::line(p, view.name, 14.0, s::SEMIBOLD, name_ink, max_width);
    text::put(p, &name, left, text_top, s::LINE_14);
    let state_top = text_top + s::LINE_14 + 2.0;
    let (icon, words, weight) = state_line(view);
    let mut x = left;
    if let Some(icon) = icon {
        let size = if icon == Icon::Warning { 11.0 } else { 9.0 };
        let rect = Rect::from_center_size(pos2(x + 4.5, state_top + s::LINE_12 / 2.0), vec2(size, size));
        icons::paint(p, icon, rect, if icon == Icon::Warning { s::WARN } else { state_ink });
        x += 9.0 + 4.0;
    }
    let state = text::line(p, &words, 12.0, weight, state_ink, max_width - (x - left));
    text::put(p, &state, x, state_top, s::LINE_12);
}

/// Settings of a macro, under its line: held button, hotkey (or the capture zone), message.
#[expect(clippy::too_many_arguments, reason = "painting step with its layout inputs")]
fn paint_options(
    ui: &Ui,
    p: &Painter,
    block: Rect,
    id: Id,
    layout: &Layout<'_>,
    index: usize,
    message: Option<&MessageBox>,
    last: bool,
    actions: &mut Vec<HudAction>,
) {
    let view = &layout.state.macros[index];
    parts::block(p, block, s::OPTIONS, s::ROW_LINE, last);
    let left = block.left() + 12.0;
    let mut y = block.top() + 8.0;

    let MacroOptions::HoldClick { button } = view.options;
    text::draw(p, "Bouton maintenu", 12.0, s::SEMIBOLD, s::TEXT_2, left, y, s::LINE_12);
    y += s::LINE_12 + 4.0;
    let selector = Rect::from_min_size(pos2(left, y), vec2(CONTENT_WIDTH, parts::SELECTOR_HEIGHT));
    if let Some(choice) = parts::selector(ui, p, selector, id.with("bouton"), button) {
        actions.push(HudAction::SetOptions(view.id, MacroOptions::HoldClick { button: choice }));
    }
    y += parts::SELECTOR_HEIGHT + 10.0;

    if layout.state.capturing == Some(view.id) {
        let zone = Rect::from_min_size(pos2(left, y), vec2(CONTENT_WIDTH, parts::CAPTURE_HEIGHT));
        if parts::capture_zone(ui, p, zone, id.with("capture")) {
            actions.push(HudAction::CancelCapture);
        }
        y += parts::CAPTURE_HEIGHT;
    } else {
        let label_width = text::draw(p, "Raccourci", 12.0, s::SEMIBOLD, s::TEXT_2, left, y, 26.0);
        let button_width = parts::small_button_width("Changer");
        let button = Rect::from_min_size(pos2(left + CONTENT_WIDTH - button_width, y), vec2(button_width, 26.0));
        if parts::small_button(ui, p, button, id.with("changer"), "Changer").clicked() {
            actions.push(HudAction::StartCapture(view.id));
        }
        let room = button.left() - 6.0 - (left + label_width + 8.0);
        let key_width = parts::keycap_width(&view.hotkey, KeyLook::Normal, room);
        let key = Rect::from_min_size(pos2(button.left() - 6.0 - key_width, y + 1.0), vec2(key_width, KEY_HEIGHT));
        parts::keycap(p, key, &view.hotkey, KeyLook::Normal);
        y += 26.0;
    }

    if let Some(message) = message {
        y += 8.0;
        parts::message(p, Rect::from_min_size(pos2(left, y), vec2(CONTENT_WIDTH, message.height())), message);
    }
}

/// Folded: the pill, and under it the current message.
fn paint_folded(ui: &Ui, rect: Rect, layout: &Layout<'_>, shadow: bool, actions: &mut Vec<HudAction>) {
    let p = ui.painter().clone();
    let chip = Rect::from_min_size(rect.min, vec2(layout.chip_width + 2.0, s::CHIP_HEIGHT + 2.0));
    paint_chip(ui, &p, chip, layout.state, shadow, actions);
    if let Some(message) = &layout.bubble {
        let size = vec2(BUBBLE_WIDTH + 2.0, message.height() + 2.0);
        let outer = Rect::from_min_size(pos2(rect.left(), chip.bottom() + 6.0), size);
        if shadow {
            p.add(s::SHADOW.as_shape(outer.shrink(1.0), s::RADIUS));
        }
        parts::message(&p, outer.shrink(1.0), message);
        p.rect_stroke(outer, s::RADIUS + 1, Stroke::new(1.0, s::RING), StrokeKind::Inside);
    }
}

/// The pill: state and hotkey of the main macro, « Déplier ».
fn paint_chip(ui: &Ui, p: &Painter, rect: Rect, state: &HudState, shadow: bool, actions: &mut Vec<HudAction>) {
    let body = rect.shrink(1.0);
    let radius = (s::CHIP_HEIGHT / 2.0) as u8;
    if shadow {
        p.add(s::SHADOW.as_shape(body, radius));
    }
    let main = main_macro(state);
    let active = main.is_some_and(|m| m.status == MacroStatus::Active);
    p.rect_filled(body, radius, if active { s::ON } else { s::PANEL });
    let id = ui.make_persistent_id("macrohud-pastille");
    drag_area(ui, body, id.with("glisser"), state.title(), actions);

    let ink = if active { s::ON_INK } else { s::TEXT };
    let inner = body.shrink(1.0);
    let middle = inner.center().y;
    let grip = Rect::from_min_size(pos2(inner.left() + 6.0, middle - 8.0), vec2(12.0, 16.0));
    icons::paint(p, Icon::Grip, grip, if active { s::ON_INK } else { s::GRIP });
    let mut x = grip.right() + 6.0;
    if let Some(view) = main {
        let (icon, word) = chip_state(view);
        let icon_rect = Rect::from_center_size(pos2(x + 5.0, middle), vec2(10.0, 10.0));
        icons::paint(p, icon, icon_rect, if icon == Icon::Warning { s::WARN } else { ink });
        x += 10.0 + 5.0;
        let line_top = (middle - s::LINE_12 / 2.0).round();
        x += text::draw(p, word, 12.0, s::BOLD, ink, x, line_top, s::LINE_12) + 2.0 + 6.0;
        let look = if active { KeyLook::OnGreen } else { KeyLook::Normal };
        let key_width = parts::keycap_width(&view.hotkey, look, KEY_MAX_IN_ROW);
        let key = Rect::from_min_size(pos2(x, (middle - 12.0).round()), vec2(key_width, KEY_HEIGHT));
        parts::keycap(p, key, &view.hotkey, look);
    }
    let unfold = Rect::from_min_size(pos2(inner.right() - 4.0 - 24.0, (middle - 12.0).round()), vec2(24.0, 24.0));
    let look = IconButton {
        icon: Icon::Down,
        icon_size: 13.0,
        ink: if active { s::ON_INK } else { s::TEXT_BUTTON },
        fill: egui::Color32::TRANSPARENT,
        fill_hover: if active { s::ON_HOVER } else { s::HOVER },
        fill_press: if active { s::ON_HOVER } else { s::PRESS },
        edge: None,
        radius: 12,
    };
    if parts::icon_button(ui, p, unfold, id.with("deplier"), &look, "Déplier").clicked() {
        actions.push(HudAction::SetCollapsed(false));
    }
    p.rect_stroke(body, radius, Stroke::new(1.0, s::EDGE), StrokeKind::Inside);
    p.rect_stroke(rect, radius + 1, Stroke::new(1.0, s::RING), StrokeKind::Inside);
}
