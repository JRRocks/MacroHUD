//! The MacroHUD heads-up display, drawn with egui.
//!
//! Look: variant 2 « Liste compacte » of the mock-ups (decision D-12, `maquettes/variante-2.png`):
//! a title bar, one line per macro (▶ button, state, shortcut key on the right), the settings of
//! a macro unfolded under its line, and a small pill when the HUD is folded.
//!
//! The HUD shows only what `docs/CONTRAT-HUD.md` lists ([`HudView`]) and only asks for what it
//! allows ([`HudAction`]). Everything is painted by hand to match the mock-up at 100 % scaling.
//!
//! Integration, once per frame:
//! 1. call [`install_fonts`] once at start-up;
//! 2. size the window with [`fit_window`]`(ctx, hud.size(ctx, &view))`: no shadow in the real
//!    window (see [`style::SHADOW`]); corners rounded with [`WINDOW_RADIUS`], so the window is
//!    transparent around the panel;
//! 3. call [`Hud::show`] in a frameless, transparent root `Ui` placed at (0, 0) and execute the
//!    returned actions; ask egui to repaint when the engine changes a state (F8 pressed in a game).
//!
//! `cargo test` renders the views of the mock-up to PNG without a screen (see `render.rs`).

mod icons;
mod parts;
pub mod style;
pub(crate) mod text;
mod view;

#[cfg(test)]
pub(crate) mod demo;
#[cfg(test)]
pub(crate) mod render;

use egui::{
    Context, CursorIcon, Id, LayerId, Painter, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
    ViewportCommand, pos2, vec2,
};

use icons::Icon;
use parts::{IconButton, KEY_HEIGHT, KeyLook};
use style as s;

pub use text::install_fonts;
pub use view::{
    HudAction, HudView, Level, MacroIcon, MacroOptions, MacroState, MacroView, Message, MouseButton,
};

/// Resizes the native window to `size` (from [`Hud::size`]) when it changed: one command per
/// change, not one per frame.
pub fn fit_window(ctx: &Context, size: Vec2) {
    let id = Id::unique("macrohud-taille-fenetre");
    if ctx.data(|d| d.get_temp::<Vec2>(id)) != Some(size) {
        ctx.data_mut(|d| d.insert_temp(id, size));
        ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
    }
}

/// Corner radius of the window: panel radius plus the 1 px ring.
pub const WINDOW_RADIUS: f32 = s::RADIUS as f32 + 1.0;

/// The HUD and its purely visual state (which settings are unfolded).
#[derive(Clone, Debug, Default)]
pub struct Hud {
    /// Macro whose settings the user unfolded under its line.
    open: Option<usize>,
    /// Paint the drop shadow of the mock-up. Off by default: in the real window the shadow would
    /// need a transparent margin that swallows clicks around the HUD.
    pub shadow: bool,
}

impl Hud {
    pub fn new() -> Self {
        Self::default()
    }

    /// Unfolds the settings of macro `index` (or folds them all with `None`).
    pub fn set_open(&mut self, index: Option<usize>) {
        self.open = index;
    }

    /// Size of the HUD for `view`, black ring included, shadow excluded: the window size.
    pub fn size(&self, ctx: &Context, view: &HudView) -> Vec2 {
        let p = ctx.layer_painter(LayerId::background());
        if view.collapsed {
            vec2(chip_width(&p, view) + 2.0, s::CHIP_HEIGHT + 2.0)
        } else {
            vec2(s::PANEL_WIDTH + 2.0, self.panel_height(&p, view) + 2.0)
        }
    }

    /// Paints the HUD at the cursor of `ui` and returns what the user asked for.
    pub fn show(&mut self, ui: &mut Ui, view: &HudView) -> Vec<HudAction> {
        let size = self.size(ui.ctx(), view);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let mut actions = Vec::new();
        if view.collapsed {
            self.paint_chip(ui, rect, view, &mut actions);
        } else {
            self.paint_panel(ui, rect, view, &mut actions);
        }
        actions
    }

    /// Settings shown under a line: the macro waiting for a shortcut, else the macro a message is
    /// about, else the one the user unfolded.
    fn open_row(&self, view: &HudView) -> Option<usize> {
        let valid = |i: usize| view.macros.get(i).is_some_and(|m| m.available);
        view.capture
            .filter(|&i| valid(i))
            .or_else(|| {
                view.message
                    .as_ref()
                    .and_then(|m| m.macro_index)
                    .filter(|&i| valid(i))
            })
            .or_else(|| self.open.filter(|&i| valid(i)))
    }

    /// Message not attached to an unfolded line: shown in its own block at the bottom.
    fn loose_message<'a>(&self, view: &'a HudView) -> Option<&'a Message> {
        let message = view.message.as_ref()?;
        let attached = message.macro_index.is_some() && message.macro_index == self.open_row(view);
        (!attached).then_some(message)
    }

    // Measures ---------------------------------------------------------------------------------

    /// The full-width blocks under the title bar, top to bottom.
    fn blocks<'a>(&self, view: &'a HudView) -> Vec<Block<'a>> {
        let open = self.open_row(view);
        let mut blocks = Vec::new();
        for (i, item) in view.macros.iter().enumerate() {
            blocks.push(Block::Row(i));
            if let MacroState::Unavailable { reason } = &item.state {
                blocks.push(Block::Notice(unavailable_notice(reason)));
            }
            if open == Some(i) {
                blocks.push(Block::Options(i));
            }
        }
        if let Some(message) = self.loose_message(view) {
            blocks.push(Block::Loose(message));
        }
        blocks
    }

    fn block_height(&self, p: &Painter, view: &HudView, block: &Block<'_>) -> f32 {
        match block {
            Block::Row(_) => s::ROW_HEIGHT,
            Block::Options(i) => self.options_height(p, view, *i),
            Block::Notice(message) => framed_message_height(p, message),
            Block::Loose(message) => framed_message_height(p, message),
        }
    }

    /// Height of the panel, its 1 px border included.
    fn panel_height(&self, p: &Painter, view: &HudView) -> f32 {
        let blocks = self.blocks(view);
        let content: f32 = blocks.iter().map(|b| self.block_height(p, view, b)).sum();
        1.0 + s::BAR_HEIGHT + content + 1.0
    }

    fn options_height(&self, p: &Painter, view: &HudView, i: usize) -> f32 {
        let mut height = 8.0;
        if matches!(view.macros[i].options, MacroOptions::HeldButton(_)) {
            height += s::LINE_12 + 4.0 + parts::SELECTOR_HEIGHT + 10.0;
        }
        height += if view.capture == Some(i) {
            parts::CAPTURE_HEIGHT
        } else {
            26.0
        };
        if let Some(message) = attached_message(view, i) {
            let with_action = message.level == Level::Warning;
            height += 8.0 + parts::message_height(p, message, content_width(), with_action);
        }
        height + 10.0 + 1.0
    }

    // Unfolded panel ---------------------------------------------------------------------------

    fn paint_panel(&mut self, ui: &Ui, rect: Rect, view: &HudView, actions: &mut Vec<HudAction>) {
        let p = ui.painter().clone();
        let panel = rect.shrink(1.0);
        if self.shadow {
            p.add(s::SHADOW.as_shape(panel, s::RADIUS));
        }
        p.rect_filled(panel, s::RADIUS, s::PANEL);
        let inner = panel.shrink(1.0);
        let id = ui.make_persistent_id("macrohud");
        let open = self.open_row(view);

        let mut top = self.paint_bar(ui, &p, inner, id, view, actions);
        let blocks = self.blocks(view);
        for (k, block) in blocks.iter().enumerate() {
            let last = k + 1 == blocks.len();
            let height = self.block_height(&p, view, block);
            match block {
                Block::Row(i) => {
                    let item = &view.macros[*i];
                    self.paint_row(
                        ui,
                        &p,
                        inner,
                        top,
                        id.with(i),
                        *i,
                        item,
                        open == Some(*i),
                        last,
                        actions,
                    );
                }
                Block::Options(i) => {
                    self.paint_options(ui, &p, inner, top, id.with(i), view, *i, last, actions);
                }
                Block::Notice(_) | Block::Loose(_) => {
                    let Some(message) = block.message() else {
                        continue;
                    };
                    let block =
                        Rect::from_min_size(pos2(inner.left(), top), vec2(inner.width(), height));
                    parts::block(&p, block, s::OPTIONS, s::ROW_LINE, last);
                    let size = vec2(content_width(), height - 19.0);
                    let rect = Rect::from_min_size(pos2(inner.left() + 12.0, top + 8.0), size);
                    parts::message(ui, &p, rect, id.with(("message", k)), message, false);
                }
            }
            top += height;
        }

        // Border and ring last: blocks never cover them.
        p.rect_stroke(
            panel,
            s::RADIUS,
            Stroke::new(1.0, s::EDGE),
            StrokeKind::Inside,
        );
        p.rect_stroke(
            rect,
            s::RADIUS + 1,
            Stroke::new(1.0, s::RING),
            StrokeKind::Inside,
        );
    }

    /// Title bar: drag handle, name and version, « Replier », « Quitter ». Returns its bottom.
    fn paint_bar(
        &self,
        ui: &Ui,
        p: &Painter,
        inner: Rect,
        id: Id,
        view: &HudView,
        actions: &mut Vec<HudAction>,
    ) -> f32 {
        let bar = Rect::from_min_size(inner.min, vec2(inner.width(), s::BAR_HEIGHT - 1.0));
        // Registered first: the buttons registered after it win the clicks.
        drag_area(ui, bar, id.with("barre"));

        let middle = bar.center().y;
        let grip = Rect::from_min_size(pos2(bar.left() + 6.0, middle - 8.0), vec2(12.0, 16.0));
        icons::paint(p, Icon::Grip, grip, s::GRIP);
        let line_top = bar.top() + ((bar.height() - s::LINE_12) / 2.0).round();
        let mut x = grip.right() + 6.0;
        x += text::draw(
            p,
            "MacroHUD",
            12.0,
            s::BOLD,
            s::TEXT,
            x,
            line_top,
            s::LINE_12,
        ) + 6.0;
        text::draw(
            p,
            &view.version,
            12.0,
            s::REGULAR,
            s::TEXT_2,
            x,
            line_top,
            s::LINE_12,
        );

        let button_top = (middle - 12.0).round();
        let quit_width = parts::bar_button_width(p, "Quitter");
        let quit = Rect::from_min_size(
            pos2(bar.right() - 4.0 - quit_width, button_top),
            vec2(quit_width, 24.0),
        );
        let fold_width = parts::bar_button_width(p, "Replier");
        let fold = Rect::from_min_size(
            pos2(quit.left() - 6.0 - fold_width, button_top),
            vec2(fold_width, 24.0),
        );
        if parts::bar_button(ui, p, fold, id.with("replier"), Icon::Up, "Replier").clicked() {
            actions.push(HudAction::SetCollapsed(true));
        }
        if parts::bar_button(ui, p, quit, id.with("quitter"), Icon::Close, "Quitter").clicked() {
            actions.push(HudAction::Quit);
        }
        p.hline(
            inner.x_range(),
            bar.bottom() + 0.5,
            Stroke::new(1.0, s::BAR_LINE),
        );
        inner.top() + s::BAR_HEIGHT
    }

    /// One macro line.
    #[expect(
        clippy::too_many_arguments,
        reason = "painting step with its layout inputs"
    )]
    fn paint_row(
        &mut self,
        ui: &Ui,
        p: &Painter,
        inner: Rect,
        top: f32,
        id: Id,
        index: usize,
        item: &MacroView,
        open: bool,
        last: bool,
        actions: &mut Vec<HudAction>,
    ) {
        let active = item.state == MacroState::Active;
        let unavailable = matches!(item.state, MacroState::Unavailable { .. });
        let soon = !item.available;
        let (fill, line, stripe) = if active {
            (s::ON, s::ON_LINE, s::ON_INK)
        } else if soon {
            (s::SOON_ROW, s::ROW_LINE, s::SOON_STRIPE)
        } else if unavailable {
            (s::ROW, s::ROW_LINE, s::WARN)
        } else {
            (s::ROW, s::ROW_LINE, s::STRIPE)
        };
        let block =
            Rect::from_min_size(pos2(inner.left(), top), vec2(inner.width(), s::ROW_HEIGHT));
        parts::block(p, block, fill, line, last);
        let content = Rect::from_min_size(block.min, vec2(block.width(), s::ROW_HEIGHT - 1.0));
        parts::stripe(
            p,
            Rect::from_min_size(content.min, vec2(4.0, content.height())),
            stripe,
            block.bottom(),
            last,
        );
        let middle = content.center().y;

        // ▶ / ■ button, or the icon of a macro « À venir ».
        let toggle = Rect::from_min_size(
            pos2(content.left() + 14.0, (middle - 16.0).round()),
            vec2(32.0, 32.0),
        );
        if soon {
            let icon = match item.icon {
                MacroIcon::Mouse => Icon::Mouse,
                MacroIcon::RepeatedClick => Icon::Repeat,
                MacroIcon::Key => Icon::Key,
            };
            icons::paint(
                p,
                icon,
                Rect::from_center_size(toggle.center(), vec2(16.0, 16.0)),
                s::SOON_TEXT,
            );
        } else if unavailable {
            p.rect_stroke(
                toggle,
                8,
                Stroke::new(1.0, s::SOON_STRIPE),
                StrokeKind::Inside,
            );
            icons::paint(
                p,
                Icon::Play,
                Rect::from_center_size(toggle.center(), vec2(13.0, 13.0)),
                s::SOON_TEXT,
            );
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
                actions.push(HudAction::Toggle(index));
            }
        }

        // Settings arrow, then the shortcut key, from the right.
        let arrow = Rect::from_min_size(
            pos2(content.right() - 6.0 - 24.0, (middle - 12.0).round()),
            vec2(24.0, 24.0),
        );
        if soon {
            icons::paint(
                p,
                Icon::Down,
                Rect::from_center_size(arrow.center(), vec2(13.0, 13.0)),
                s::SOON_CHEVRON,
            );
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
            let label = if open {
                "Masquer les réglages"
            } else {
                "Afficher les réglages"
            };
            if parts::icon_button(ui, p, arrow, id.with("reglages"), &look, label).clicked() {
                self.open = if open { None } else { Some(index) };
            }
        }
        let key_look = if soon {
            KeyLook::Off
        } else if active {
            KeyLook::OnGreen
        } else {
            KeyLook::Normal
        };
        let key_width = parts::keycap_width(p, &item.shortcut, key_look);
        let key = Rect::from_min_size(
            pos2(arrow.left() - 8.0 - key_width, (middle - 12.0).round()),
            vec2(key_width, KEY_HEIGHT),
        );
        parts::keycap(p, key, &item.shortcut, key_look);

        // Name and state line.
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
        let text_top =
            content.top() + ((content.height() - (s::LINE_14 + 2.0 + s::LINE_12)) / 2.0).round();
        let name = text::line(p, &item.name, 14.0, s::SEMIBOLD, name_ink, max_width);
        text::put(p, &name, left, text_top, s::LINE_14);
        let state_top = text_top + s::LINE_14 + 2.0;
        let (icon, words, weight) = state_line(item);
        let mut x = left;
        if let Some(icon) = icon {
            let size = if icon == Icon::Warning { 11.0 } else { 9.0 };
            let rect = Rect::from_center_size(
                pos2(x + 4.5, state_top + s::LINE_12 / 2.0),
                vec2(size, size),
            );
            icons::paint(
                p,
                icon,
                rect,
                if icon == Icon::Warning {
                    s::WARN
                } else {
                    state_ink
                },
            );
            x += 9.0 + 4.0;
        }
        let state = text::line(p, &words, 12.0, weight, state_ink, max_width - (x - left));
        text::put(p, &state, x, state_top, s::LINE_12);
    }

    /// Settings of macro `index`, under its line.
    #[expect(
        clippy::too_many_arguments,
        reason = "painting step with its layout inputs"
    )]
    fn paint_options(
        &self,
        ui: &Ui,
        p: &Painter,
        inner: Rect,
        top: f32,
        id: Id,
        view: &HudView,
        index: usize,
        last: bool,
        actions: &mut Vec<HudAction>,
    ) {
        let item = &view.macros[index];
        let height = self.options_height(p, view, index);
        let block = Rect::from_min_size(pos2(inner.left(), top), vec2(inner.width(), height));
        parts::block(p, block, s::OPTIONS, s::ROW_LINE, last);
        let left = inner.left() + 12.0;
        let width = content_width();
        let mut y = top + 8.0;

        if let MacroOptions::HeldButton(button) = item.options {
            text::draw(
                p,
                "Bouton maintenu",
                12.0,
                s::SEMIBOLD,
                s::TEXT_2,
                left,
                y,
                s::LINE_12,
            );
            y += s::LINE_12 + 4.0;
            let rect = Rect::from_min_size(pos2(left, y), vec2(width, parts::SELECTOR_HEIGHT));
            if let Some(chosen) = parts::selector(ui, p, rect, id.with("bouton"), button) {
                actions.push(HudAction::SetButton(index, chosen));
            }
            y += parts::SELECTOR_HEIGHT + 10.0;
        }

        if view.capture == Some(index) {
            let rect = Rect::from_min_size(pos2(left, y), vec2(width, parts::CAPTURE_HEIGHT));
            if parts::capture_zone(ui, p, rect, id.with("capture")) {
                actions.push(HudAction::CancelCapture);
            }
            y += parts::CAPTURE_HEIGHT;
        } else {
            text::draw(p, "Raccourci", 12.0, s::SEMIBOLD, s::TEXT_2, left, y, 26.0);
            let button_width = parts::small_button_width(p, "Changer");
            let button = Rect::from_min_size(
                pos2(left + width - button_width, y),
                vec2(button_width, 26.0),
            );
            if parts::small_button(ui, p, button, id.with("changer"), "Changer").clicked() {
                actions.push(HudAction::StartCapture(index));
            }
            let key_width = parts::keycap_width(p, &item.shortcut, KeyLook::Normal);
            let key = Rect::from_min_size(
                pos2(button.left() - 6.0 - key_width, y + 1.0),
                vec2(key_width, KEY_HEIGHT),
            );
            parts::keycap(p, key, &item.shortcut, KeyLook::Normal);
            y += 26.0;
        }

        if let Some(message) = attached_message(view, index) {
            y += 8.0;
            let with_action = message.level == Level::Warning;
            let height = parts::message_height(p, message, width, with_action);
            let rect = Rect::from_min_size(pos2(left, y), vec2(width, height));
            if parts::message(ui, p, rect, id.with("message"), message, with_action) {
                actions.push(HudAction::StartCapture(index));
            }
        }
    }

    // Folded pill ------------------------------------------------------------------------------

    fn paint_chip(&self, ui: &Ui, rect: Rect, view: &HudView, actions: &mut Vec<HudAction>) {
        let p = ui.painter().clone();
        let body = rect.shrink(1.0);
        let radius = (s::CHIP_HEIGHT / 2.0) as u8;
        if self.shadow {
            p.add(s::SHADOW.as_shape(body, radius));
        }
        let main = main_macro(view);
        let active = main.is_some_and(|m| m.state == MacroState::Active);
        p.rect_filled(body, radius, if active { s::ON } else { s::PANEL });
        let id = ui.make_persistent_id("macrohud-pastille");
        drag_area(ui, body, id.with("glisser"));

        let ink = if active { s::ON_INK } else { s::TEXT };
        let inner = body.shrink(1.0);
        let middle = inner.center().y;
        let grip = Rect::from_min_size(pos2(inner.left() + 6.0, middle - 8.0), vec2(12.0, 16.0));
        icons::paint(
            &p,
            Icon::Grip,
            grip,
            if active { s::ON_INK } else { s::GRIP },
        );
        let mut x = grip.right() + 6.0;
        if let Some(item) = main {
            let (icon, word) = chip_state(item);
            let icon_rect = Rect::from_center_size(pos2(x + 5.0, middle), vec2(10.0, 10.0));
            icons::paint(
                &p,
                icon,
                icon_rect,
                if icon == Icon::Warning { s::WARN } else { ink },
            );
            x += 10.0 + 5.0;
            let line_top = (middle - s::LINE_12 / 2.0).round();
            x += text::draw(&p, word, 12.0, s::BOLD, ink, x, line_top, s::LINE_12) + 2.0 + 6.0;
            let look = if active {
                KeyLook::OnGreen
            } else {
                KeyLook::Normal
            };
            let key_width = parts::keycap_width(&p, &item.shortcut, look);
            let key = Rect::from_min_size(
                pos2(x, (middle - 12.0).round()),
                vec2(key_width, KEY_HEIGHT),
            );
            parts::keycap(&p, key, &item.shortcut, look);
        }
        let unfold = Rect::from_min_size(
            pos2(inner.right() - 4.0 - 24.0, (middle - 12.0).round()),
            vec2(24.0, 24.0),
        );
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
        if parts::icon_button(ui, &p, unfold, id.with("deplier"), &look, "Déplier").clicked() {
            actions.push(HudAction::SetCollapsed(false));
        }
        p.rect_stroke(body, radius, Stroke::new(1.0, s::EDGE), StrokeKind::Inside);
        p.rect_stroke(
            rect,
            radius + 1,
            Stroke::new(1.0, s::RING),
            StrokeKind::Inside,
        );
    }
}

/// Full-width blocks stacked under the title bar.
enum Block<'a> {
    /// Line of macro `i`.
    Row(usize),
    /// Why the macro above cannot run.
    Notice(Message),
    /// Settings of macro `i`.
    Options(usize),
    /// Message about no macro in particular.
    Loose(&'a Message),
}

impl Block<'_> {
    fn message(&self) -> Option<&Message> {
        match self {
            Block::Notice(message) => Some(message),
            Block::Loose(message) => Some(message),
            Block::Row(_) | Block::Options(_) => None,
        }
    }
}

fn unavailable_notice(reason: &str) -> Message {
    Message {
        level: Level::Warning,
        title: "Macro indisponible".to_owned(),
        detail: reason.to_owned(),
        macro_index: None,
    }
}

/// A message in its own block: 8 px above, 10 px and the line below.
fn framed_message_height(p: &Painter, message: &Message) -> f32 {
    8.0 + parts::message_height(p, message, content_width(), false) + 10.0 + 1.0
}

/// Width available inside a settings block (12 px left padding, 8 px right).
fn content_width() -> f32 {
    s::PANEL_WIDTH - 2.0 - 12.0 - 8.0
}

fn attached_message(view: &HudView, index: usize) -> Option<&Message> {
    view.message
        .as_ref()
        .filter(|m| m.macro_index == Some(index))
}

/// Area that moves the window when dragged.
fn drag_area(ui: &Ui, rect: Rect, id: Id) {
    let response = ui.interact(rect, id, Sense::click_and_drag());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// Icon, words and weight of the state line under the name.
fn state_line(item: &MacroView) -> (Option<Icon>, String, f32) {
    let option = match &item.options {
        MacroOptions::HeldButton(button) => Some(button.word().to_owned()),
        MacroOptions::Summary(summary) => Some(summary.clone()),
        MacroOptions::None => None,
    };
    if !item.available {
        let words = option.map_or_else(|| "À venir".to_owned(), |o| format!("À venir · {o}"));
        return (None, words, s::REGULAR);
    }
    match (&item.state, &item.options) {
        (MacroState::Unavailable { .. }, _) => {
            (Some(Icon::Warning), "Indisponible".to_owned(), s::SEMIBOLD)
        }
        (MacroState::Active, MacroOptions::HeldButton(button)) => (
            Some(Icon::Dot),
            format!("Active · {} enfoncé", button.word()),
            s::SEMIBOLD,
        ),
        (MacroState::Active, _) => (Some(Icon::Dot), with_option("Active", option), s::SEMIBOLD),
        (MacroState::Stopped, MacroOptions::HeldButton(button)) => (
            Some(Icon::Stop),
            format!("Arrêtée · bouton {}", button.word()),
            s::REGULAR,
        ),
        (MacroState::Stopped, _) => (Some(Icon::Stop), with_option("Arrêtée", option), s::REGULAR),
    }
}

fn with_option(state: &str, option: Option<String>) -> String {
    option.map_or_else(|| state.to_owned(), |o| format!("{state} · {o}"))
}

/// The macro the folded pill shows: the first active one, else the first available one.
fn main_macro(view: &HudView) -> Option<&MacroView> {
    view.macros
        .iter()
        .find(|m| m.state == MacroState::Active)
        .or_else(|| view.macros.iter().find(|m| m.available))
        .or_else(|| view.macros.first())
}

fn chip_state(item: &MacroView) -> (Icon, &'static str) {
    match item.state {
        MacroState::Active => (Icon::Dot, "Active"),
        MacroState::Stopped => (Icon::Stop, "Arrêtée"),
        MacroState::Unavailable { .. } => (Icon::Warning, "Indisponible"),
    }
}

/// Width of the folded pill, its 1 px border included.
fn chip_width(p: &Painter, view: &HudView) -> f32 {
    let mut width = 1.0 + 6.0 + 12.0 + 6.0;
    if let Some(item) = main_macro(view) {
        let (_, word) = chip_state(item);
        let look = if item.state == MacroState::Active {
            KeyLook::OnGreen
        } else {
            KeyLook::Normal
        };
        width += 10.0 + 5.0 + text::width(p, word, 12.0, s::BOLD) + 2.0 + 6.0;
        width += parts::keycap_width(p, &item.shortcut, look) + 6.0;
    }
    (width + 24.0 + 4.0 + 1.0).ceil()
}
