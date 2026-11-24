//! The HUD: variant 2 « Liste compacte » of the mock-ups (decision D-12, `maquettes/variante-2.png`),
//! drawn with egui. A title bar, one line per macro (▶ button, state, hotkey on the right), the
//! settings of a macro unfolded under its line, the messages, and a small pill when folded.
//! « Multi-clic » follows track A of its preview (D-26, `maquettes/multi-clic/apercu.html`): its
//! speed is set with a slider from 1 to 15, also while it runs. The external macros (D-32) follow
//! MacroHUD's own, with lines of the same shape (D-37, `maquettes/macro-externe/apercu.html`): step
//! pill, « En pause » and « Terminée » from their script, amber box, sliders for their settings.
//!
//! What the rest of MacroHUD uses (interface set by the "programmation" session):
//!
//! - [`setup`]: called once at start (installs the Inter font);
//! - [`draw`]: draws the HUD from a [`HudState`] and returns the [`HudAction`]s the user asked for;
//! - [`window_size`]: the window's inner size, in points, for a given state (the window follows it);
//! - [`TRANSPARENT`]: whether the window is transparent (rounded corners, folded pill);
//! - [`MESSAGE_DURATION`]: how long a message stays shown;
//! - [`window_icon`]: the window's icon (taskbar, Alt+Tab): the logo (D-33), from `icone.ico`,
//!   which `build.rs` also makes the icon of `macrohud.exe`.
//!
//! Everything the HUD shows comes from `crate::contract` (docs/CONTRAT-HUD.md); everything it asks
//! goes through [`HudAction`]. Between frames it only remembers which macro has its settings
//! unfolded. Text is measured with fonts of its own, so [`window_size`] and [`draw`] compute the
//! same layout. `cargo test` renders every situation to PNG, without a screen (`render.rs`).

mod icons;
mod logo;
mod parts;
mod style;
mod text;

#[cfg(test)]
mod render;

use std::cell::Cell;
use std::time::Duration;

use eframe::egui::{self, CursorIcon, Id, Painter, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

use crate::contract::{
    Badge, BadgeColor, ExternalLine, ExternalMacroView, HudAction, HudState, MacroId, MacroOptions, MacroStatus,
    MacroView, MessageKind, MessageLevel, MouseButton,
};
use icons::Icon;
use parts::{AlertBox, IconButton, KEY_HEIGHT, KeyLook, MessageBox, PillColour};
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

/// The window's icon (taskbar, Alt+Tab): the logo of MacroHUD (D-33).
pub fn window_icon() -> egui::IconData {
    logo::window_icon()
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

// The macros -----------------------------------------------------------------------------------

/// One macro of the HUD: one of MacroHUD's own, or an external one (D-32), whose line has the same
/// shape (D-37).
#[derive(Clone, Copy)]
enum Item<'a> {
    Own(&'a MacroView),
    External(&'a ExternalMacroView),
}

impl<'a> Item<'a> {
    fn id(self) -> MacroId {
        match self {
            Item::Own(view) => view.id,
            Item::External(view) => view.id,
        }
    }

    fn name(self) -> &'a str {
        match self {
            Item::Own(view) => view.name,
            Item::External(view) => &view.name,
        }
    }

    /// Its hotkey as text; empty for an external macro that has none.
    fn hotkey(self) -> &'a str {
        match self {
            Item::Own(view) => &view.hotkey,
            Item::External(view) => &view.hotkey,
        }
    }

    fn status(self) -> &'a MacroStatus {
        match self {
            Item::Own(view) => &view.status,
            Item::External(view) => &view.status,
        }
    }

    fn active(self) -> bool {
        *self.status() == MacroStatus::Active
    }

    /// Something can be asked of it: not a macro « à venir ».
    fn available(self) -> bool {
        match self {
            Item::Own(view) => view.available,
            Item::External(_) => true,
        }
    }

    /// Active, its clicks waiting while the mouse is on the HUD.
    fn suspended(self) -> bool {
        match self {
            Item::Own(view) => view.suspended,
            Item::External(view) => view.suspended,
        }
    }

    /// The step pill of an active external macro.
    fn badge(self) -> Option<&'a Badge> {
        match self {
            Item::External(view) if self.active() => view.badge.as_ref(),
            _ => None,
        }
    }

    /// An external macro whose script stopped by itself (« Terminée »): blue border.
    fn finished(self) -> bool {
        matches!(self, Item::External(view) if !self.active() && matches!(view.line, ExternalLine::Finished(_)))
    }
}

/// Every macro, in the order of the HUD: MacroHUD's own, then the external ones (D-32).
fn items(state: &HudState) -> Vec<Item<'_>> {
    state.macros.iter().map(Item::Own).chain(state.external_macros.iter().map(Item::External)).collect()
}

// Layout ---------------------------------------------------------------------------------------

/// Width of a settings block's content (12 px left padding, 8 px right).
const CONTENT_WIDTH: f32 = s::PANEL_WIDTH - 2.0 - 12.0 - 8.0;
/// Width of the message shown under the folded pill (as wide as the unfolded panel).
const BUBBLE_WIDTH: f32 = s::PANEL_WIDTH;
/// Widest keycap on a macro line and on the pill; a longer hotkey is cut with « … ».
const KEY_MAX_IN_ROW: f32 = 84.0;
/// Widest state words on the folded pill (« Active 12/100 »); longer ones are cut with « … ».
const CHIP_WORDS_MAX: f32 = 140.0;
/// The folded pill is never wider than the unfolded HUD (D-54).
const CHIP_MAX: f32 = s::PANEL_WIDTH;

/// The full-width blocks stacked under the title bar.
enum Block {
    /// Line of macro `i`.
    Row(usize),
    /// Why a macro cannot run.
    Notice(MessageBox),
    /// The amber box of an external macro, under its line.
    Alert(AlertBox),
    /// Settings of macro `i`, with the current message when it concerns that macro.
    Options(usize, Option<MessageBox>),
    /// The current message, below the macros.
    Message(MessageBox),
}

/// Where everything goes; computed the same way by [`window_size`] and [`draw`].
struct Layout<'a> {
    state: &'a HudState,
    /// Every macro, in the order of the HUD.
    items: Vec<Item<'a>>,
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
        let items = items(state);
        let usable = |id: MacroId| items.iter().position(|item| item.id() == id && item.available());
        // A refused hotkey keeps its macro's settings open, like the capture that led to it.
        let refused = state
            .message
            .as_ref()
            .filter(|m| matches!(m.kind, MessageKind::HotkeyRefused { .. }))
            .and_then(|m| m.about);
        // Settings show while their macro waits for a hotkey or just had one refused, else when the
        // user unfolded them.
        let open = state
            .capturing
            .and_then(usable)
            .or_else(|| refused.and_then(usable))
            .or_else(|| remembered.and_then(usable));
        // « Choisir une autre touche » only for a macro that can take a new hotkey.
        let message_box = |width: f32| {
            state
                .message
                .as_ref()
                .map(|m| MessageBox::from_contract(m, m.about.filter(|&id| usable(id).is_some()), width))
        };
        let mut blocks = Vec::new();
        let (chip, bubble, size);
        if state.collapsed {
            chip = chip_width(main_item(&items));
            bubble = message_box(BUBBLE_WIDTH);
            let mut folded = vec2(chip + 2.0, s::CHIP_HEIGHT + 2.0);
            if let Some(bubble) = &bubble {
                folded.x = folded.x.max(BUBBLE_WIDTH + 2.0);
                folded.y += 6.0 + bubble.height() + 2.0;
            }
            size = folded;
        } else {
            (chip, bubble) = (0.0, None);
            // The message sits in the settings of the macro it concerns when they are open, else below.
            let open_id = open.map(|i| items[i].id());
            let in_settings = state.message.as_ref().is_some_and(|m| m.about.is_some() && m.about == open_id);
            for (i, item) in items.iter().enumerate() {
                blocks.push((Block::Row(i), s::ROW_HEIGHT));
                if let MacroStatus::Unavailable(reason) = item.status() {
                    let notice = MessageBox::new(MessageLevel::Warning, "Macro indisponible", reason, CONTENT_WIDTH);
                    let height = framed_height(notice.height());
                    blocks.push((Block::Notice(notice), height));
                }
                if let Item::External(view) = item
                    && let Some(alert) = &view.alert
                {
                    let alert = AlertBox::new(alert, CONTENT_WIDTH);
                    let height = framed_height(alert.height());
                    blocks.push((Block::Alert(alert), height));
                }
                if open == Some(i) {
                    let message = if in_settings { message_box(CONTENT_WIDTH) } else { None };
                    let capturing = state.capturing == Some(item.id());
                    let height = match item {
                        Item::Own(view) => options_height(view.options, capturing, message.as_ref()),
                        Item::External(view) => external_options_height(view, capturing, message.as_ref()),
                    };
                    blocks.push((Block::Options(i, message), height));
                }
            }
            if !in_settings && let Some(message) = message_box(CONTENT_WIDTH) {
                let height = framed_height(message.height());
                blocks.push((Block::Message(message), height));
            }
            let content: f32 = blocks.iter().map(|(_, height)| height).sum();
            // Ring, border, title bar, blocks, border, ring.
            size = vec2(s::PANEL_WIDTH + 2.0, 1.0 + 1.0 + s::BAR_HEIGHT + content + 1.0 + 1.0);
        }
        Layout { state, items, open, blocks, chip_width: chip, bubble, size }
    }
}

/// A box in its own block: 8 px above it, then 10 px and the block's line below.
fn framed_height(box_height: f32) -> f32 {
    8.0 + box_height + 10.0 + 1.0
}

/// A setting with a slider: its title line, the slider, 10 px.
const SLIDER_SETTING_HEIGHT: f32 = s::LINE_12 + 4.0 + parts::SLIDER_HEIGHT + 10.0;

/// Settings: button selector, speed of « Multi-clic », hotkey line (or the capture zone), message.
fn options_height(options: MacroOptions, capturing: bool, message: Option<&MessageBox>) -> f32 {
    let selector = s::LINE_12 + 4.0 + parts::SELECTOR_HEIGHT + 10.0;
    let speed = match options {
        MacroOptions::MultiClick { .. } => SLIDER_SETTING_HEIGHT,
        MacroOptions::HoldClick { .. } => 0.0,
    };
    let hotkey = if capturing { parts::CAPTURE_HEIGHT } else { 26.0 };
    let message = message.map_or(0.0, |m| 8.0 + m.height());
    8.0 + selector + speed + hotkey + message + 10.0 + 1.0
}

/// The help line of an external macro's settings, wrapped, with room for its signs (D-56).
fn info_lines(view: &ExternalMacroView) -> Vec<String> {
    view.info
        .as_deref()
        .map(str::trim)
        .filter(|info| !info.is_empty())
        .map_or_else(Vec::new, |info| text::wrap(&text::with_sign_room(info), 12.0, s::REGULAR, CONTENT_WIDTH))
}

/// Settings of an external macro (D-37): a slider per setting, the hotkey line (or the capture
/// zone), the message, the help line.
fn external_options_height(view: &ExternalMacroView, capturing: bool, message: Option<&MessageBox>) -> f32 {
    let settings = view.settings.len() as f32 * SLIDER_SETTING_HEIGHT;
    let hotkey = if capturing { parts::CAPTURE_HEIGHT } else { 26.0 };
    let message = message.map_or(0.0, |m| 8.0 + m.height());
    let lines = info_lines(view).len();
    let info = if lines == 0 { 0.0 } else { 10.0 + lines as f32 * s::LINE_MESSAGE };
    8.0 + settings + hotkey + message + info + 10.0 + 1.0
}

/// The macro the folded pill shows: the active one (one at a time, D-20), else the first available
/// one of MacroHUD.
fn main_item<'a>(items: &[Item<'a>]) -> Option<Item<'a>> {
    items
        .iter()
        .find(|item| item.active())
        .or_else(|| items.iter().find(|item| matches!(item, Item::Own(_)) && item.available()))
        .or_else(|| items.first())
        .copied()
}

/// Icon and words of the pill. A macro whose clicks wait for the mouse to leave the HUD shows the
/// two bars of « En pause » but keeps its words: the pill does not change size under the mouse.
/// An active external macro adds the text of its state (« Active 12/100 »), or says « En pause »
/// while its script waits.
fn chip_state(item: Item<'_>) -> (Icon, String) {
    match item.status() {
        MacroStatus::Active => {
            let icon = if item.suspended() { Icon::Pause } else { Icon::Dot };
            match item {
                Item::External(view) => match &view.line {
                    ExternalLine::Waiting(_) => (Icon::Pause, "En pause".to_owned()),
                    ExternalLine::Active(Some(state)) if !state.trim().is_empty() => {
                        (icon, format!("Active {}", state.trim()))
                    }
                    _ => (icon, "Active".to_owned()),
                },
                Item::Own(_) => (icon, "Active".to_owned()),
            }
        }
        MacroStatus::Stopped => (Icon::Stop, "Arrêtée".to_owned()),
        MacroStatus::Unavailable(_) => (Icon::Warning, "Indisponible".to_owned()),
    }
}

/// Room for the pill's words: at most [`CHIP_WORDS_MAX`], and what the pill leaves under
/// [`CHIP_MAX`] once its grip, icon, step pill, hotkey and arrow are placed; the words are cut first.
fn chip_words_room(item: Item<'_>) -> f32 {
    let mut others = 1.0 + 6.0 + 12.0 + 6.0 + 10.0 + 5.0 + 2.0 + 6.0 + 24.0 + 4.0 + 1.0;
    if let Some(badge) = item.badge() {
        others += parts::pill_width(&badge.text) + 6.0;
    }
    if !item.hotkey().is_empty() {
        others += parts::keycap_width(item.hotkey(), KeyLook::Normal, KEY_MAX_IN_ROW) + 6.0;
    }
    (CHIP_MAX - others).clamp(0.0, CHIP_WORDS_MAX)
}

/// Width of the folded pill, its border included.
fn chip_width(main: Option<Item<'_>>) -> f32 {
    let mut width = 1.0 + 6.0 + 12.0 + 6.0;
    if let Some(item) = main {
        let (_, words) = chip_state(item);
        width += 10.0 + 5.0 + text::width(&words, 12.0, s::BOLD).min(chip_words_room(item)) + 2.0 + 6.0;
        if let Some(badge) = item.badge() {
            width += parts::pill_width(&badge.text) + 6.0;
        }
        if !item.hotkey().is_empty() {
            width += parts::keycap_width(item.hotkey(), KeyLook::Normal, KEY_MAX_IN_ROW) + 6.0;
        }
    }
    (width + 24.0 + 4.0 + 1.0).ceil()
}

/// The held (or clicked) button in one word, the selector's: « gauche », « droit », « milieu ».
fn button_word(view: &MacroView) -> &'static str {
    match view.options.button() {
        MouseButton::Left => "gauche",
        MouseButton::Right => "droit",
        MouseButton::Middle => "milieu",
    }
}

/// Drawing of a kind of macro, shown instead of the ▶ button while it is « à venir ».
fn kind_icon(view: &MacroView) -> Icon {
    match view.options {
        MacroOptions::HoldClick { .. } | MacroOptions::MultiClick { .. } => Icon::Mouse,
    }
}

/// Icon, words and weight of the state line under a macro's name, as on the mock-ups.
/// « Maintenir le clic » (D-16): « Arrêtée · bouton gauche », « Active · gauche enfoncé ». During the
/// 3 s after « Activer » the engine keeps the macro « arrêtée » and counts down in a message, so
/// « enfoncé » is always true. « Multi-clic » (D-26): « Arrêtée · gauche, 10/s », « Active · gauche,
/// 10/s », and « En pause · souris ici » with two bars while its clicks wait for the mouse to leave
/// the HUD; the line stays green, the HUD keeps its size.
fn state_line(view: &MacroView) -> (Option<Icon>, String, f32) {
    if !view.available {
        return (None, "À venir".to_owned(), s::REGULAR);
    }
    let button = button_word(view);
    match (&view.status, view.options) {
        (MacroStatus::Unavailable(_), _) => (Some(Icon::Warning), "Indisponible".to_owned(), s::SEMIBOLD),
        (MacroStatus::Active, MacroOptions::MultiClick { .. }) if view.suspended => {
            (Some(Icon::Pause), "En pause · souris ici".to_owned(), s::SEMIBOLD)
        }
        (MacroStatus::Active, MacroOptions::MultiClick { rate, .. }) => {
            (Some(Icon::Dot), format!("Active · {button}, {}/s", rate.per_second()), s::SEMIBOLD)
        }
        (MacroStatus::Stopped, MacroOptions::MultiClick { rate, .. }) => {
            (Some(Icon::Stop), format!("Arrêtée · {button}, {}/s", rate.per_second()), s::REGULAR)
        }
        (MacroStatus::Active, MacroOptions::HoldClick { .. }) => {
            (Some(Icon::Dot), format!("Active · {button} enfoncé"), s::SEMIBOLD)
        }
        (MacroStatus::Stopped, MacroOptions::HoldClick { .. }) => {
            (Some(Icon::Stop), format!("Arrêtée · bouton {button}"), s::REGULAR)
        }
    }
}

/// The state line of an external macro (D-37): « ■ Arrêtée · limite 100 clics » (or « ■ Arrêtée »
/// alone), « ● Active · 12/100 », « ❚❚ En pause · attend sa fenêtre » while its script waits,
/// « ✓ Terminée · 100/100 clics » once it stopped by itself. While the mouse is on the HUD, like
/// « Multi-clic » (D-26, D-53): « ❚❚ En pause · souris ici ».
fn external_state_line(view: &ExternalMacroView) -> (Option<Icon>, String, f32) {
    let joined = |word: &str, text: Option<&str>| match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(text) => format!("{word} · {text}"),
        None => word.to_owned(),
    };
    match (&view.status, &view.line) {
        (MacroStatus::Unavailable(_), _) => (Some(Icon::Warning), "Indisponible".to_owned(), s::SEMIBOLD),
        (MacroStatus::Active, ExternalLine::Waiting(why)) => {
            (Some(Icon::Pause), joined("En pause", Some(why)), s::SEMIBOLD)
        }
        (MacroStatus::Active, _) if view.suspended => {
            (Some(Icon::Pause), "En pause · souris ici".to_owned(), s::SEMIBOLD)
        }
        (MacroStatus::Active, ExternalLine::Active(state)) => {
            (Some(Icon::Dot), joined("Active", state.as_deref()), s::SEMIBOLD)
        }
        (MacroStatus::Active, _) => (Some(Icon::Dot), "Active".to_owned(), s::SEMIBOLD),
        (MacroStatus::Stopped, ExternalLine::Finished(end)) => {
            (Some(Icon::Check), joined("Terminée", Some(end)), s::REGULAR)
        }
        (MacroStatus::Stopped, ExternalLine::Stopped(state)) => {
            (Some(Icon::Stop), joined("Arrêtée", state.as_deref()), s::REGULAR)
        }
        (MacroStatus::Stopped, _) => (Some(Icon::Stop), "Arrêtée".to_owned(), s::REGULAR),
    }
}

/// The state line of any macro.
fn item_state_line(item: Item<'_>) -> (Option<Icon>, String, f32) {
    match item {
        Item::Own(view) => state_line(view),
        Item::External(view) => external_state_line(view),
    }
}

/// The colours of the step pill, as the script names them.
fn pill_colour(color: BadgeColor) -> PillColour {
    match color {
        BadgeColor::Amber => PillColour::Amber,
        BadgeColor::Green => PillColour::Green,
        BadgeColor::Blue => PillColour::Blue,
        BadgeColor::Grey => PillColour::Grey,
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
                let message = message.as_ref();
                match layout.items[*i] {
                    Item::Own(view) => paint_options(ui, &p, area, id.with(i), layout, view, message, last, actions),
                    Item::External(view) => {
                        paint_external_options(ui, &p, area, id.with(i), layout, view, message, last, actions);
                    }
                }
            }
            Block::Notice(message) | Block::Message(message) => {
                parts::block(&p, area, s::OPTIONS, s::ROW_LINE, last);
                let rect = Rect::from_min_size(area.min + vec2(12.0, 8.0), vec2(CONTENT_WIDTH, message.height()));
                if parts::message(ui, &p, rect, id.with(("message", k)), message)
                    && let Some(macro_id) = message.retry
                {
                    actions.push(HudAction::StartCapture(macro_id));
                }
            }
            Block::Alert(alert) => {
                parts::block(&p, area, s::OPTIONS, s::ROW_LINE, last);
                let rect = Rect::from_min_size(area.min + vec2(12.0, 8.0), vec2(CONTENT_WIDTH, alert.height()));
                parts::alert(&p, rect, alert);
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

/// One macro line: ▶ / ■, name and state, step pill (external macros), hotkey, settings arrow.
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
    let item = layout.items[index];
    let active = item.active();
    let unavailable = matches!(item.status(), MacroStatus::Unavailable(_));
    let soon = !item.available();
    let open = layout.open == Some(index);
    let (fill, line, stripe) = if active {
        (s::ON, s::ON_LINE, s::ON_INK)
    } else if soon {
        (s::SOON_ROW, s::ROW_LINE, s::SOON_STRIPE)
    } else if unavailable {
        (s::ROW, s::ROW_LINE, s::WARN)
    } else if item.finished() {
        // « Terminée »: the blue border of the preview (D-37).
        (s::ROW, s::ROW_LINE, s::INFO)
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
        let kind = match item {
            Item::Own(view) => kind_icon(view),
            Item::External(_) => Icon::Mouse,
        };
        icons::paint(p, kind, Rect::from_center_size(toggle.center(), vec2(16.0, 16.0)), s::SOON_TEXT);
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
            actions.push(if active { HudAction::Stop(item.id()) } else { HudAction::Start(item.id()) });
        }
    }

    // Settings arrow, then the hotkey (if any) and the step pill (if any), from the right.
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
            set_open_macro(if open { None } else { Some(item.id()) });
        }
    }
    let mut right = arrow.left();
    if !item.hotkey().is_empty() {
        let key_look = if soon {
            KeyLook::Off
        } else if active {
            KeyLook::OnGreen
        } else {
            KeyLook::Normal
        };
        let key_width = parts::keycap_width(item.hotkey(), key_look, KEY_MAX_IN_ROW);
        let key =
            Rect::from_min_size(pos2(right - 8.0 - key_width, (middle - 12.0).round()), vec2(key_width, KEY_HEIGHT));
        parts::keycap(p, key, item.hotkey(), key_look);
        right = key.left();
    }
    if let Some(badge) = item.badge() {
        let width = parts::pill_width(&badge.text);
        let pill = Rect::from_min_size(
            pos2(right - 8.0 - width, (middle - parts::PILL_HEIGHT / 2.0).round()),
            vec2(width, parts::PILL_HEIGHT),
        );
        parts::pill(p, pill, pill_colour(badge.color), &badge.text);
        right = pill.left();
    }

    // Name, and the state line under it; both stop before the pill or the key, cut with « … » when
    // the room lacks (D-55).
    let left = toggle.right() + 8.0;
    let max_width = right - 8.0 - left;
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
    let name = text::line(p, item.name(), 14.0, s::SEMIBOLD, name_ink, max_width);
    text::put(p, &name, left, text_top, s::LINE_14);
    let state_top = text_top + s::LINE_14 + 2.0;
    let (icon, words, weight) = item_state_line(item);
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

/// Settings of a macro of MacroHUD, under its line: held or clicked button, speed of « Multi-clic »,
/// hotkey (or the capture zone), message.
#[expect(clippy::too_many_arguments, reason = "painting step with its layout inputs")]
fn paint_options(
    ui: &Ui,
    p: &Painter,
    block: Rect,
    id: Id,
    layout: &Layout<'_>,
    view: &MacroView,
    message: Option<&MessageBox>,
    last: bool,
    actions: &mut Vec<HudAction>,
) {
    parts::block(p, block, s::OPTIONS, s::ROW_LINE, last);
    let left = block.left() + 12.0;
    let mut y = block.top() + 8.0;

    let title = match view.options {
        MacroOptions::HoldClick { .. } => "Bouton maintenu",
        MacroOptions::MultiClick { .. } => "Bouton cliqué",
    };
    text::draw(p, title, 12.0, s::SEMIBOLD, s::TEXT_2, left, y, s::LINE_12);
    y += s::LINE_12 + 4.0;
    let selector = Rect::from_min_size(pos2(left, y), vec2(CONTENT_WIDTH, parts::SELECTOR_HEIGHT));
    if let Some(choice) = parts::selector(ui, p, selector, id.with("bouton"), view.options.button()) {
        actions.push(HudAction::SetOptions(view.id, view.options.with_button(choice)));
    }
    y += parts::SELECTOR_HEIGHT + 10.0;

    // « Multi-clic »: « Rapidité » and its value, then the slider. A new speed is asked for at once,
    // also while the macro runs: it counts from the next click (D-22, D-27).
    if let MacroOptions::MultiClick { rate, .. } = view.options {
        setting_title(p, left, y, "Rapidité", &rate.label());
        y += s::LINE_12 + 4.0;
        let line = Rect::from_min_size(pos2(left, y), vec2(CONTENT_WIDTH, parts::SLIDER_HEIGHT));
        if let Some(new) = parts::rate_slider(ui, p, line, id.with("rapidite"), rate) {
            actions.push(HudAction::SetOptions(view.id, view.options.with_rate(new)));
        }
        y += parts::SLIDER_HEIGHT + 10.0;
    }

    y = paint_hotkey(ui, p, left, y, id, layout, view.id, &view.hotkey, actions);
    paint_settings_message(ui, p, left, y, id, message, actions);
}

/// Settings of an external macro (D-37): a slider per setting, its value written above it; the
/// hotkey (or the capture zone); the message; the help line of its declaration.
#[expect(clippy::too_many_arguments, reason = "painting step with its layout inputs")]
fn paint_external_options(
    ui: &Ui,
    p: &Painter,
    block: Rect,
    id: Id,
    layout: &Layout<'_>,
    view: &ExternalMacroView,
    message: Option<&MessageBox>,
    last: bool,
    actions: &mut Vec<HudAction>,
) {
    parts::block(p, block, s::OPTIONS, s::ROW_LINE, last);
    let left = block.left() + 12.0;
    let mut y = block.top() + 8.0;

    // A new value is asked for at once; it goes to the script if it runs (D-34).
    for (k, setting) in view.settings.iter().enumerate() {
        setting_title(p, left, y, &setting.name, &setting.label());
        y += s::LINE_12 + 4.0;
        let line = Rect::from_min_size(pos2(left, y), vec2(CONTENT_WIDTH, parts::SLIDER_HEIGHT));
        let range = parts::Range { min: setting.min, max: setting.max, step: setting.step.max(1) };
        if let Some(value) = parts::slider(ui, p, line, id.with(("reglage", k)), setting.value, range, &setting.name) {
            actions.push(HudAction::SetSetting(view.id, setting.key.clone(), setting.snap(value)));
        }
        y += parts::SLIDER_HEIGHT + 10.0;
    }

    y = paint_hotkey(ui, p, left, y, id, layout, view.id, &view.hotkey, actions);
    y = paint_settings_message(ui, p, left, y, id, message, actions);
    let lines = info_lines(view);
    if !lines.is_empty() {
        y += 10.0;
        for line in &lines {
            // A few points of slack: the line breaks were measured, the painter must not cut again.
            let galley = text::line(p, line, 12.0, s::REGULAR, s::TEXT_2, CONTENT_WIDTH + 4.0);
            text::put(p, &galley, left, y, s::LINE_MESSAGE);
            let x = left.round();
            let signs: Vec<_> = text::sign_spots(&galley)
                .into_iter()
                .map(|(sign, middle)| (sign, pos2(x + middle, y + s::LINE_MESSAGE / 2.0)))
                .collect();
            parts::paint_signs(p, &signs, s::TEXT_2);
            y += s::LINE_MESSAGE;
        }
    }
}

/// Title line of a setting: its name on the left (cut before the value if needed), its value in
/// bold on the right.
fn setting_title(p: &Painter, left: f32, top: f32, name: &str, value: &str) {
    // The name first, then the value: the order in which the HUD always laid them out.
    let value_left = left + CONTENT_WIDTH - text::width(value, 12.0, s::BOLD);
    let name = text::line(p, name, 12.0, s::SEMIBOLD, s::TEXT_2, value_left - 8.0 - left);
    text::put(p, &name, left, top, s::LINE_12);
    let value = text::line(p, value, 12.0, s::BOLD, s::TEXT, f32::INFINITY);
    text::put(p, &value, left + CONTENT_WIDTH - value.size().x, top, s::LINE_12);
}

/// The hotkey line of a macro's settings (« Raccourci », the key, « Changer »), or the capture zone
/// while it waits for a key. A macro without a hotkey says « Aucun » (D-52). Returns its bottom.
#[expect(clippy::too_many_arguments, reason = "painting step with its layout inputs")]
fn paint_hotkey(
    ui: &Ui,
    p: &Painter,
    left: f32,
    top: f32,
    id: Id,
    layout: &Layout<'_>,
    macro_id: MacroId,
    hotkey: &str,
    actions: &mut Vec<HudAction>,
) -> f32 {
    if layout.state.capturing == Some(macro_id) {
        let zone = Rect::from_min_size(pos2(left, top), vec2(CONTENT_WIDTH, parts::CAPTURE_HEIGHT));
        if parts::capture_zone(ui, p, zone, id.with("capture")) {
            actions.push(HudAction::CancelCapture);
        }
        return top + parts::CAPTURE_HEIGHT;
    }
    let label_width = text::draw(p, "Raccourci", 12.0, s::SEMIBOLD, s::TEXT_2, left, top, 26.0);
    let button_width = parts::small_button_width("Changer");
    let button = Rect::from_min_size(pos2(left + CONTENT_WIDTH - button_width, top), vec2(button_width, 26.0));
    if parts::small_button(ui, p, button, id.with("changer"), "Changer").clicked() {
        actions.push(HudAction::StartCapture(macro_id));
    }
    let room = button.left() - 6.0 - (left + label_width + 8.0);
    if hotkey.is_empty() {
        let none = text::line(p, "Aucun", 12.0, s::REGULAR, s::TEXT_2, room);
        text::put(p, &none, button.left() - 8.0 - none.size().x, top, 26.0);
    } else {
        let key_width = parts::keycap_width(hotkey, KeyLook::Normal, room);
        let key = Rect::from_min_size(pos2(button.left() - 6.0 - key_width, top + 1.0), vec2(key_width, KEY_HEIGHT));
        parts::keycap(p, key, hotkey, KeyLook::Normal);
    }
    top + 26.0
}

/// The message about the macro whose settings are open, under its hotkey line. Returns its bottom.
fn paint_settings_message(
    ui: &Ui,
    p: &Painter,
    left: f32,
    top: f32,
    id: Id,
    message: Option<&MessageBox>,
    actions: &mut Vec<HudAction>,
) -> f32 {
    let Some(message) = message else {
        return top;
    };
    let rect = Rect::from_min_size(pos2(left, top + 8.0), vec2(CONTENT_WIDTH, message.height()));
    if parts::message(ui, p, rect, id.with("message"), message)
        && let Some(macro_id) = message.retry
    {
        actions.push(HudAction::StartCapture(macro_id));
    }
    rect.bottom()
}

/// Folded: the pill, and under it the current message. « Choisir une autre touche » there also
/// unfolds the HUD, so that « Appuie sur une touche… » shows.
fn paint_folded(ui: &Ui, rect: Rect, layout: &Layout<'_>, shadow: bool, actions: &mut Vec<HudAction>) {
    let p = ui.painter().clone();
    let chip = Rect::from_min_size(rect.min, vec2(layout.chip_width + 2.0, s::CHIP_HEIGHT + 2.0));
    paint_chip(ui, &p, chip, layout, shadow, actions);
    if let Some(message) = &layout.bubble {
        let size = vec2(BUBBLE_WIDTH + 2.0, message.height() + 2.0);
        let outer = Rect::from_min_size(pos2(rect.left(), chip.bottom() + 6.0), size);
        if shadow {
            p.add(s::SHADOW.as_shape(outer.shrink(1.0), s::RADIUS));
        }
        let id = ui.make_persistent_id("macrohud-message-replie");
        if parts::message(ui, &p, outer.shrink(1.0), id, message)
            && let Some(macro_id) = message.retry
        {
            actions.push(HudAction::SetCollapsed(false));
            actions.push(HudAction::StartCapture(macro_id));
        }
        p.rect_stroke(outer, s::RADIUS + 1, Stroke::new(1.0, s::RING), StrokeKind::Inside);
    }
}

/// The pill: state, step pill (external macros) and hotkey of the main macro, « Déplier ».
fn paint_chip(ui: &Ui, p: &Painter, rect: Rect, layout: &Layout<'_>, shadow: bool, actions: &mut Vec<HudAction>) {
    let body = rect.shrink(1.0);
    let radius = (s::CHIP_HEIGHT / 2.0) as u8;
    if shadow {
        p.add(s::SHADOW.as_shape(body, radius));
    }
    let main = main_item(&layout.items);
    let active = main.is_some_and(Item::active);
    p.rect_filled(body, radius, if active { s::ON } else { s::PANEL });
    let id = ui.make_persistent_id("macrohud-pastille");
    drag_area(ui, body, id.with("glisser"), layout.state.title(), actions);

    let ink = if active { s::ON_INK } else { s::TEXT };
    let inner = body.shrink(1.0);
    let middle = inner.center().y;
    let grip = Rect::from_min_size(pos2(inner.left() + 6.0, middle - 8.0), vec2(12.0, 16.0));
    icons::paint(p, Icon::Grip, grip, if active { s::ON_INK } else { s::GRIP });
    let mut x = grip.right() + 6.0;
    if let Some(item) = main {
        let (icon, words) = chip_state(item);
        let icon_rect = Rect::from_center_size(pos2(x + 5.0, middle), vec2(10.0, 10.0));
        icons::paint(p, icon, icon_rect, if icon == Icon::Warning { s::WARN } else { ink });
        x += 10.0 + 5.0;
        let line_top = (middle - s::LINE_12 / 2.0).round();
        let words = text::line(p, &words, 12.0, s::BOLD, ink, chip_words_room(item));
        x += text::put(p, &words, x, line_top, s::LINE_12) + 2.0 + 6.0;
        if let Some(badge) = item.badge() {
            let width = parts::pill_width(&badge.text);
            let pill = Rect::from_min_size(
                pos2(x, (middle - parts::PILL_HEIGHT / 2.0).round()),
                vec2(width, parts::PILL_HEIGHT),
            );
            parts::pill(p, pill, pill_colour(badge.color), &badge.text);
            x += width + 6.0;
        }
        if !item.hotkey().is_empty() {
            let look = if active { KeyLook::OnGreen } else { KeyLook::Normal };
            let key_width = parts::keycap_width(item.hotkey(), look, KEY_MAX_IN_ROW);
            let key = Rect::from_min_size(pos2(x, (middle - 12.0).round()), vec2(key_width, KEY_HEIGHT));
            parts::keycap(p, key, item.hotkey(), look);
        }
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
