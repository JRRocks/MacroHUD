//! The eframe application: the HUD's window (always on top, borderless, movable, position
//! remembered), the glue between the HUD and the engine, the messages and the settings.
//!
//! Every frame: read the engine's snapshot, build the [`HudState`], let the HUD draw it
//! (`hud::draw`), send its actions on, resize the window to `hud::window_size`, save the settings
//! when they changed. The engine thread asks for a frame whenever a macro changes (a hotkey pressed
//! while the HUD has no focus, a real click...), so the HUD is always up to date.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::contract::{
    ExternalLine, ExternalMacroView, HudAction, HudState, MacroId, MacroStatus, MacroView, Message, MessageLevel,
};
use crate::engine::{
    Command, Event, ExternalSetup, ExternalSnapshot, KeySource, MacroSetup, MacroSnapshot, Notice, Snapshot,
};
use crate::external::{self, Declaration};
use crate::hotkey::Hotkey;
use crate::hud;
use crate::macros::{CATALOG, MacroDef};
use crate::platform::{self, HudWindow};
use crate::runner::EngineHandle;
use crate::settings::{self, Settings};

/// A moved window's position is saved once it has stayed still this long.
const POSITION_SAVE_DELAY: Duration = Duration::from_secs(1);
/// How long the HUD's own key capture waits for the engine to confirm the capture it asked for.
const CAPTURE_CONFIRM_WAIT: Duration = Duration::from_secs(2);

/// What `main` hands over.
pub struct Startup {
    /// The settings folder (`%APPDATA%\MacroHUD`).
    pub dir: PathBuf,
    pub settings: Settings,
    /// Warnings to show first (damaged settings, abrupt end last time).
    pub messages: Vec<Message>,
    /// The declarations of the external macros, in the order of their ids.
    pub externals: Vec<Declaration>,
}

/// An external macro as the app knows it: its id, made once for the whole run, and its declaration.
pub type External = (MacroId, Declaration);

/// Opens the HUD and runs until it is closed.
pub fn run(startup: Startup) -> Result<(), String> {
    let externals: Vec<External> =
        startup.externals.iter().map(|d| (MacroId(external::lasting(&d.id)), d.clone())).collect();
    let first = initial_state(&startup.settings, &externals);
    let viewport = egui::ViewportBuilder::default()
        .with_title(crate::NAME)
        .with_app_id("macrohud")
        .with_inner_size(hud::window_size(&first))
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top()
        .with_transparent(hud::TRANSPARENT)
        .with_icon(hud::window_icon());
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        persist_window: false,
        centered: false,
        ..Default::default()
    };
    eframe::run_native(
        crate::NAME,
        options,
        Box::new(move |cc| Ok(Box::new(HudApp::new(cc, startup, externals, hud::window_size(&first))?))),
    )
    .map_err(|e| e.to_string())
}

/// The application.
struct HudApp {
    engine: EngineHandle,
    window: HudWindow,
    dir: PathBuf,
    /// The settings as last written (or read).
    saved: Settings,
    externals: Vec<External>,
    collapsed: bool,
    position: PositionTracker,
    messages: Messages,
    snapshot: Snapshot,
    /// The window size last asked for.
    size: egui::Vec2,
    placed: bool,
    check_on_screen: bool,
    save_failed: bool,
    shut_down: bool,
    /// The HUD's own key capture runs (started with the engine's): since when, and whether the
    /// engine has shown its capture since (then its end ends this one too).
    window_capture: Option<(Instant, bool)>,
}

impl HudApp {
    fn new(
        cc: &eframe::CreationContext<'_>,
        startup: Startup,
        externals: Vec<External>,
        size: egui::Vec2,
    ) -> Result<Self, String> {
        hud::setup(&cc.egui_ctx);
        let window = HudWindow::from_creation_context(cc);
        let ctx = cc.egui_ctx.clone();
        let engine = platform::spawn_engine(
            macro_setup(&startup.settings, CATALOG),
            external_setup(&startup.settings, &externals),
            window.raw(),
            Box::new(move || ctx.request_repaint()),
        )?;
        let now = Instant::now();
        let mut messages = Messages::default();
        for message in startup.messages {
            messages.push_later(message, now);
        }
        Ok(HudApp {
            engine,
            window,
            dir: startup.dir,
            collapsed: startup.settings.collapsed,
            position: PositionTracker::new(startup.settings.position),
            saved: startup.settings,
            externals,
            messages,
            snapshot: Snapshot::default(),
            size,
            placed: false,
            check_on_screen: false,
            save_failed: false,
            shut_down: false,
            window_capture: None,
        })
    }

    fn state(&self) -> HudState {
        hud_state(&self.snapshot, CATALOG, &self.externals, self.collapsed, self.messages.current())
    }

    fn apply(&mut self, ctx: &egui::Context, action: HudAction) {
        match action {
            HudAction::Start(id) => self.engine.send(Command::Start(id)),
            HudAction::Stop(id) => self.engine.send(Command::Stop(id)),
            HudAction::StartCapture(id) => {
                self.engine.send(Command::StartCapture(id));
                // While the HUD has the focus, the keys go to its own thread: listen there too.
                let report = self.engine.reporter();
                self.window.start_key_capture(Box::new(move |outcome| {
                    report(Event::Captured { outcome, source: KeySource::HudWindow })
                }));
                self.window_capture = Some((Instant::now(), false));
            }
            HudAction::CancelCapture => {
                self.engine.send(Command::CancelCapture);
                self.stop_window_capture();
            }
            HudAction::SetOptions(id, options) => self.engine.send(Command::SetOptions(id, options)),
            HudAction::SetSetting(id, key, value) => self.engine.send(Command::SetSetting(id, key, value)),
            HudAction::SetCollapsed(collapsed) => self.collapsed = collapsed,
            HudAction::StartDrag => ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag),
            HudAction::Quit => {
                self.shutdown();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// The settings as they are now.
    fn current_settings(&self) -> Settings {
        let mut current = self.saved.clone();
        current.collapsed = self.collapsed;
        current.position = self.position.settled();
        for m in &self.snapshot.macros {
            if let Some(entry) = current.macros.iter_mut().find(|e| e.id == m.id) {
                entry.hotkey = m.hotkey;
                entry.options = m.options;
            }
        }
        for x in &self.snapshot.externals {
            if let Some((_, declaration)) = self.externals.iter().find(|(id, _)| *id == x.id) {
                settings::set_external(&mut current, declaration, x.hotkey, &x.values, &|_| x.hotkey_label.clone());
            }
        }
        current
    }

    fn save_if_changed(&mut self) {
        let current = self.current_settings();
        if current == self.saved {
            return;
        }
        let snapshot = &self.snapshot.macros;
        let label = |h: &Hotkey| {
            snapshot.iter().find(|m| m.hotkey == *h).map_or_else(|| h.label(&|_| None), |m| m.hotkey_label.clone())
        };
        match settings::save(&self.dir, &current, &label) {
            Ok(_) => self.save_failed = false,
            Err(e) => {
                crate::log::write(format!("Réglages non enregistrés : {e}"));
                if !self.save_failed {
                    self.save_failed = true;
                    let text = format!("Impossible d'enregistrer les réglages ({e}).");
                    self.messages.push_now(Message::new(MessageLevel::Warning, text), Instant::now(), false);
                }
            }
        }
        // Even after a failure: no new attempt at every frame, only at the next change.
        self.saved = current;
    }

    fn stop_window_capture(&mut self) {
        self.window.stop_key_capture();
        self.window_capture = None;
    }

    /// The HUD's own key capture follows the engine's: once the engine's capture is over (a key
    /// came by any way, Échap, too long, refused at start), this one stops too.
    fn follow_capture(&mut self, now: Instant) {
        if let Some((since, seen)) = &mut self.window_capture {
            if self.snapshot.capturing.is_some() {
                *seen = true;
            } else if *seen || now.saturating_duration_since(*since) >= CAPTURE_CONFIRM_WAIT {
                self.stop_window_capture();
            }
        }
        self.window.key_capture_tick();
    }

    /// Releases everything and saves, once: before the window closes, whatever closes it.
    fn shutdown(&mut self) {
        if self.shut_down {
            return;
        }
        self.shut_down = true;
        self.stop_window_capture();
        self.engine.shutdown();
        let released = platform::emergency_release();
        if !released.is_empty() {
            crate::log::write(format!("Relâché à la fermeture : {released:?}"));
        }
        self.snapshot = self.engine.snapshot();
        if let Some(position) = self.window.position() {
            self.position.observe(position, Instant::now());
        }
        self.position.settle_now();
        self.save_if_changed();
    }
}

impl eframe::App for HudApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        if !self.placed {
            // The window is still hidden: it appears directly at its place.
            self.window.place(self.saved.position);
            self.placed = true;
        }
        if self.check_on_screen {
            self.window.keep_on_screen();
            self.check_on_screen = false;
        }
        self.snapshot = self.engine.snapshot();
        self.follow_capture(now);
        self.messages.sync_engine(self.snapshot.notice.as_ref(), now);
        self.messages.expire(now);
        if let Some(position) = self.window.position() {
            self.position.observe(position, now);
        }
        self.position.settle(now);
        if !self.shut_down {
            self.save_if_changed();
        }
        let capture_check = self.window_capture.filter(|(_, seen)| !seen).map(|_| Duration::from_millis(250));
        let waits = [self.messages.next_change(now), self.position.next_check(now), capture_check];
        if let Some(wait) = waits.into_iter().flatten().min() {
            ctx.request_repaint_after(wait);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let state = self.state();
        for action in hud::draw(ui, &state) {
            self.apply(ui.ctx(), action);
        }
        let size = hud::window_size(&self.state());
        if size != self.size {
            self.size = size;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            self.check_on_screen = true;
            ui.ctx().request_repaint();
        }
    }

    /// Clears the window to transparent, so that the HUD draws its own shape when `hud::TRANSPARENT`
    /// is on (eframe's default is a translucent dark grey).
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shutdown();
    }
}

/// What each macro starts with: its definition and its saved settings.
pub fn macro_setup(settings: &Settings, catalog: &[MacroDef]) -> Vec<MacroSetup> {
    catalog
        .iter()
        .map(|def| {
            let saved = settings.macro_settings(def.id);
            MacroSetup {
                def: *def,
                hotkey: saved.map_or(def.default_hotkey, |s| s.hotkey),
                options: saved.map_or(def.default_options, |s| s.options),
            }
        })
        .collect()
}

/// What each external macro starts with: its declaration and its saved settings.
pub fn external_setup(settings: &Settings, externals: &[External]) -> Vec<ExternalSetup> {
    externals
        .iter()
        .map(|(_, declaration)| {
            let (hotkey, values) = settings::external_saved(settings, declaration);
            ExternalSetup { declaration: declaration.clone(), hotkey, values }
        })
        .collect()
}

/// The state shown before the engine's first snapshot (it only sizes the window).
fn initial_state(settings: &Settings, externals: &[External]) -> HudState {
    let snapshot = Snapshot {
        macros: macro_setup(settings, CATALOG)
            .iter()
            .map(|s| MacroSnapshot {
                id: s.def.id,
                status: MacroStatus::Stopped,
                hotkey: s.hotkey,
                hotkey_label: s.hotkey.label(&|_| None),
                options: s.options,
                suspended: false,
            })
            .collect(),
        externals: externals
            .iter()
            .zip(external_setup(settings, externals))
            .map(|((id, _), x)| ExternalSnapshot {
                id: *id,
                status: MacroStatus::Stopped,
                hotkey: x.hotkey,
                hotkey_label: x.hotkey.map(|h| h.label(&|_| None)).unwrap_or_default(),
                line: ExternalLine::Stopped(x.declaration.stopped_line(&x.values)),
                values: x.values,
                badge: None,
                suspended: false,
                alert: None,
            })
            .collect(),
        capturing: None,
        notice: None,
    };
    hud_state(&snapshot, CATALOG, externals, settings.collapsed, None)
}

/// Builds what the HUD receives.
pub fn hud_state(
    snapshot: &Snapshot,
    catalog: &[MacroDef],
    externals: &[External],
    collapsed: bool,
    message: Option<&Message>,
) -> HudState {
    HudState {
        app_name: crate::NAME,
        version: crate::VERSION,
        macros: catalog
            .iter()
            .map(|def| {
                let live = snapshot.macros.iter().find(|m| m.id == def.id);
                MacroView {
                    id: def.id,
                    name: def.name,
                    description: def.description,
                    status: live.map_or(MacroStatus::Stopped, |m| m.status.clone()),
                    hotkey: live.map_or_else(|| def.default_hotkey.label(&|_| None), |m| m.hotkey_label.clone()),
                    options: live.map_or(def.default_options, |m| m.options),
                    available: def.available,
                    suspended: live.is_some_and(|m| m.suspended),
                }
            })
            .collect(),
        external_macros: externals
            .iter()
            .map(|(id, declaration)| external_view(*id, declaration, snapshot.externals.iter().find(|x| x.id == *id)))
            .collect(),
        capturing: snapshot.capturing,
        message: message.cloned(),
        collapsed,
    }
}

/// The line of an external macro: its declaration, and what the engine says of it (`None`: before
/// its first snapshot, as declared).
fn external_view(id: MacroId, declaration: &Declaration, live: Option<&ExternalSnapshot>) -> ExternalMacroView {
    let values = live.map_or_else(|| declaration.defaults(), |x| x.values.clone());
    ExternalMacroView {
        id,
        name: declaration.name.clone(),
        description: declaration.description.clone(),
        status: live.map_or(MacroStatus::Stopped, |x| x.status.clone()),
        hotkey: live.map_or_else(
            || declaration.hotkey.map(|h| h.label(&|_| None)).unwrap_or_default(),
            |x| x.hotkey_label.clone(),
        ),
        line: live.map_or_else(|| ExternalLine::Stopped(declaration.stopped_line(&values)), |x| x.line.clone()),
        badge: live.and_then(|x| x.badge.clone()),
        suspended: live.is_some_and(|x| x.suspended),
        alert: live.and_then(|x| x.alert.clone()),
        settings: declaration.settings.iter().zip(&values).map(|(setting, value)| setting.at(*value)).collect(),
        info: declaration.info.clone(),
    }
}

/// Moves a window of `size` at `position` as little as possible so that it lies inside `area`
/// (left, top, right, bottom); a window larger than the area keeps its top-left corner inside.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn fit_into(position: (i32, i32), size: (i32, i32), area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (left, top, right, bottom) = area;
    let x = position.0.min(right - size.0).max(left);
    let y = position.1.min(bottom - size.1).max(top);
    (x, y)
}

/// Follows the window's position; a new position counts once the window has stayed still a moment
/// (so a drag is saved once, at its end).
#[derive(Debug)]
struct PositionTracker {
    settled: Option<(i32, i32)>,
    seen: Option<((i32, i32), Instant)>,
}

impl PositionTracker {
    fn new(saved: Option<(i32, i32)>) -> Self {
        PositionTracker { settled: saved, seen: None }
    }

    fn observe(&mut self, position: (i32, i32), now: Instant) {
        if self.seen.map(|(p, _)| p) != Some(position) {
            self.seen = Some((position, now));
        }
    }

    fn settle(&mut self, now: Instant) {
        if let Some((position, since)) = self.seen
            && now.saturating_duration_since(since) >= POSITION_SAVE_DELAY
        {
            self.settled = Some(position);
        }
    }

    fn settle_now(&mut self) {
        if let Some((position, _)) = self.seen {
            self.settled = Some(position);
        }
    }

    fn settled(&self) -> Option<(i32, i32)> {
        self.settled
    }

    /// When to look again, if a new position is waiting to settle.
    fn next_check(&self, now: Instant) -> Option<Duration> {
        let (position, since) = self.seen?;
        (Some(position) != self.settled).then(|| (since + POSITION_SAVE_DELAY).saturating_duration_since(now))
    }
}

/// The one-off messages: one shown at a time, each for `hud::MESSAGE_DURATION`.
#[derive(Debug, Default)]
struct Messages {
    current: Option<Shown>,
    waiting: VecDeque<Message>,
    last_engine_number: u64,
}

/// The message on screen.
#[derive(Debug)]
struct Shown {
    message: Message,
    since: Instant,
    /// It came from the engine (which may take it back), not from the start of MacroHUD.
    from_engine: bool,
}

impl Messages {
    /// Shows `message` now. The message it replaces comes back afterwards if it was an early
    /// warning (from the start) that had no time to be read.
    fn push_now(&mut self, message: Message, now: Instant, from_engine: bool) {
        if let Some(replaced) = self.current.take()
            && !replaced.from_engine
            && replaced.message.level != MessageLevel::Info
            && now.saturating_duration_since(replaced.since) < hud::MESSAGE_DURATION / 2
        {
            self.waiting.push_front(replaced.message);
        }
        self.current = Some(Shown { message, since: now, from_engine });
    }

    /// Shows `message` (from the start of MacroHUD) after those already waiting.
    fn push_later(&mut self, message: Message, now: Instant) {
        if self.current.is_none() {
            self.current = Some(Shown { message, since: now, from_engine: false });
        } else {
            self.waiting.push_back(message);
        }
    }

    /// Takes the engine's latest notice, if it is new: a message to show, or the end of its message.
    fn sync_engine(&mut self, notice: Option<&Notice>, now: Instant) {
        let Some(notice) = notice else { return };
        if notice.number <= self.last_engine_number {
            return;
        }
        self.last_engine_number = notice.number;
        match &notice.message {
            Some(message) => self.push_now(message.clone(), now, true),
            None => {
                if self.current.as_ref().is_some_and(|c| c.from_engine) {
                    self.show_next(now);
                }
            }
        }
    }

    fn show_next(&mut self, now: Instant) {
        self.current = self.waiting.pop_front().map(|message| Shown { message, since: now, from_engine: false });
    }

    fn expire(&mut self, now: Instant) {
        while let Some(shown) = &self.current
            && now.saturating_duration_since(shown.since) >= hud::MESSAGE_DURATION
        {
            self.show_next(now);
        }
    }

    fn current(&self) -> Option<&Message> {
        self.current.as_ref().map(|c| &c.message)
    }

    /// When the shown message must go.
    fn next_change(&self, now: Instant) -> Option<Duration> {
        self.current.as_ref().map(|c| (c.since + hud::MESSAGE_DURATION).saturating_duration_since(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{ClickRate, MacroId, MacroOptions, MouseButton};
    use crate::hotkey::{Modifier, Modifiers, vk};

    fn info(text: &str) -> Message {
        Message::new(MessageLevel::Info, text)
    }

    fn warning(text: &str) -> Message {
        Message::new(MessageLevel::Warning, text)
    }

    fn notice(number: u64, message: Message) -> Notice {
        Notice { number, message: Some(message) }
    }

    /// Runs a few frames of the HUD without a window, at its own window size; returns its actions.
    fn hud_frames(state: &HudState) -> Vec<HudAction> {
        let ctx = egui::Context::default();
        hud::setup(&ctx);
        let size = hud::window_size(state);
        let mut actions = Vec::new();
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| actions.extend(hud::draw(ui, state)));
            // No renderer here: the font textures egui wanted to upload are dropped on purpose.
            output.textures_delta.clear();
        }
        actions
    }

    /// Whatever HUD `src/hud/` holds (the debug one or the validated one): it draws every situation
    /// of the contract, at a usable size, and asks nothing by itself.
    #[test]
    fn the_hud_draws_every_sample_state_without_asking_anything() {
        for (name, state) in crate::contract::samples() {
            let size = hud::window_size(&state);
            assert!(size.x >= 1.0 && size.y >= 1.0, "{name}: size {size:?}");
            assert!(hud_frames(&state).is_empty(), "{name}: actions without any input");
        }
        assert!(hud::MESSAGE_DURATION >= Duration::from_secs(1));
    }

    #[test]
    fn windows_are_kept_inside_the_screen() {
        let area = (0, 0, 1920, 1040);
        assert_eq!(fit_into((100, 100), (300, 200), area), (100, 100));
        assert_eq!(fit_into((1800, 1000), (300, 200), area), (1620, 840));
        assert_eq!(fit_into((-500, -20), (300, 200), area), (0, 0));
        assert_eq!(fit_into((50, 50), (3000, 2000), area), (0, 0), "too large: top-left corner inside");
        // A screen on the left of the main one.
        assert_eq!(fit_into((-100, 10), (300, 200), (-1920, 0, 0, 1080)), (-300, 10));
    }

    #[test]
    fn a_position_is_saved_once_the_window_stays_still() {
        let start = Instant::now();
        let mut tracker = PositionTracker::new(Some((10, 10)));
        tracker.observe((10, 10), start);
        tracker.settle(start);
        assert_eq!(tracker.settled(), Some((10, 10)));
        tracker.observe((500, 300), start);
        tracker.settle(start + Duration::from_millis(200));
        assert_eq!(tracker.settled(), Some((10, 10)), "still moving");
        assert_eq!(tracker.next_check(start + Duration::from_millis(200)), Some(Duration::from_millis(800)));
        tracker.settle(start + POSITION_SAVE_DELAY);
        assert_eq!(tracker.settled(), Some((500, 300)));
        assert_eq!(tracker.next_check(start + POSITION_SAVE_DELAY), None);
        tracker.observe((600, 300), start + POSITION_SAVE_DELAY);
        tracker.settle_now();
        assert_eq!(tracker.settled(), Some((600, 300)));
    }

    #[test]
    fn messages_follow_each_other_and_expire() {
        let start = Instant::now();
        let mut m = Messages::default();
        m.push_later(warning("réglages abîmés"), start);
        m.push_later(warning("fermeture brutale"), start);
        assert_eq!(m.current(), Some(&warning("réglages abîmés")));
        assert_eq!(m.next_change(start), Some(hud::MESSAGE_DURATION));
        m.expire(start + hud::MESSAGE_DURATION);
        assert_eq!(m.current(), Some(&warning("fermeture brutale")));
        m.expire(start + hud::MESSAGE_DURATION * 2);
        assert_eq!(m.current(), None);
        assert_eq!(m.next_change(start), None);
    }

    #[test]
    fn engine_messages_show_at_once_and_only_once() {
        let start = Instant::now();
        let mut m = Messages::default();
        m.push_later(warning("au démarrage"), start);
        m.sync_engine(Some(&notice(1, info("raccourci enregistré"))), start);
        assert_eq!(m.current(), Some(&info("raccourci enregistré")));
        // The early warning was cut short: it comes back next.
        m.expire(start + hud::MESSAGE_DURATION);
        assert_eq!(m.current(), Some(&warning("au démarrage")));
        // The same engine message again (same number) is not shown again.
        m.sync_engine(Some(&notice(1, info("raccourci enregistré"))), start + hud::MESSAGE_DURATION);
        assert_eq!(m.current(), Some(&warning("au démarrage")));
        m.sync_engine(Some(&notice(2, info("autre"))), start + hud::MESSAGE_DURATION);
        assert_eq!(m.current(), Some(&info("autre")));
    }

    #[test]
    fn the_engine_can_take_its_message_back() {
        let start = Instant::now();
        let mut m = Messages::default();
        m.sync_engine(Some(&notice(1, info("Activation dans 3 s"))), start);
        m.sync_engine(Some(&Notice { number: 2, message: None }), start);
        assert_eq!(m.current(), None, "the countdown is gone at the press");
        // A withdrawal never takes away a message that is not the engine's.
        m.push_later(warning("au démarrage"), start);
        m.sync_engine(Some(&Notice { number: 3, message: None }), start);
        assert_eq!(m.current(), Some(&warning("au démarrage")));
        // A withdrawal lets the next waiting message show.
        let mut m = Messages::default();
        m.push_later(warning("premier"), start);
        m.push_later(warning("second"), start);
        let soon = start + Duration::from_secs(1);
        m.sync_engine(Some(&notice(1, info("Activation dans 3 s"))), soon);
        m.sync_engine(Some(&Notice { number: 2, message: None }), soon);
        assert_eq!(m.current(), Some(&warning("premier")), "cut short, so shown again");
        m.expire(soon + hud::MESSAGE_DURATION);
        assert_eq!(m.current(), Some(&warning("second")));
    }

    #[test]
    fn the_hud_state_follows_the_catalog_and_the_snapshot() {
        let ctrl_m = Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Alt), vk::KEY_M);
        let snapshot = Snapshot {
            macros: vec![
                MacroSnapshot {
                    id: MacroId::HOLD_CLICK,
                    status: MacroStatus::Active,
                    hotkey: ctrl_m,
                    hotkey_label: "Ctrl+Alt+M".into(),
                    options: MacroOptions::HoldClick { button: MouseButton::Right },
                    suspended: false,
                },
                MacroSnapshot {
                    id: MacroId::MULTI_CLICK,
                    status: MacroStatus::Active,
                    hotkey: Hotkey::F9,
                    hotkey_label: "F9".into(),
                    options: MacroOptions::MultiClick { button: MouseButton::Middle, rate: ClickRate::MAX },
                    suspended: true,
                },
            ],
            capturing: Some(MacroId::HOLD_CLICK),
            ..Snapshot::default()
        };
        let state = hud_state(&snapshot, CATALOG, &[], true, Some(&info("bonjour")));
        assert_eq!(state.app_name, "MacroHUD");
        assert_eq!(state.version, crate::VERSION);
        assert_eq!(state.title(), format!("MacroHUD {}", crate::VERSION));
        assert!(state.collapsed);
        assert_eq!(state.capturing, Some(MacroId::HOLD_CLICK));
        assert_eq!(state.message, Some(info("bonjour")));
        let view = &state.macros[0];
        assert_eq!(view.name, "Maintenir le clic");
        assert_eq!(view.status, MacroStatus::Active);
        assert_eq!(view.hotkey, "Ctrl+Alt+M");
        assert_eq!(view.options, MacroOptions::HoldClick { button: MouseButton::Right });
        assert!(view.available);
        assert!(!view.suspended);
        let multi = &state.macros[1];
        assert_eq!(multi.name, "Multi-clic");
        assert_eq!((multi.status.clone(), multi.suspended), (MacroStatus::Active, true));
        assert_eq!(multi.options, MacroOptions::MultiClick { button: MouseButton::Middle, rate: ClickRate::MAX });
    }

    fn example() -> Declaration {
        let text = r#"{ "nom": "Exemple", "description": "Clique toutes les deux secondes", "commande": "outil.exe",
            "raccourci": "F10", "boutons": ["droit"],
            "reglages": [ { "cle": "limite", "nom": "Limite de clics", "unite": "clics", "min": 10, "max": 500, "pas": 10, "defaut": 100 } ],
            "etat_arretee": "limite {limite} clics", "info": "Un vrai clic n'arrête pas cette macro." }"#;
        external::parse("exemple", text.as_bytes()).expect("declaration").0
    }

    #[test]
    fn external_macros_come_after_the_others_with_their_saved_settings() {
        use crate::contract::{Badge, BadgeColor};
        let externals = vec![(MacroId("exemple"), example())];
        let mut settings = Settings::defaults(CATALOG);
        let f11 = Hotkey::new(Modifiers::NONE, vk::F11);
        settings::set_external(&mut settings, &externals[0].1, Some(f11), &[200], &|_| "F11".into());
        // Before the engine's first word: as saved.
        let state = initial_state(&settings, &externals);
        assert_eq!(state.macros.len(), CATALOG.len());
        let first = &state.external_macros[0];
        assert_eq!((first.name.as_str(), first.description.as_str()), ("Exemple", "Clique toutes les deux secondes"));
        assert_eq!((first.status.clone(), first.hotkey.as_str()), (MacroStatus::Stopped, "F11"));
        assert_eq!(first.line, ExternalLine::Stopped(Some("limite 200 clics".into())));
        assert_eq!(first.settings[0].value, 200);
        assert_eq!(first.settings[0].name, "Limite de clics");
        assert_eq!(first.info.as_deref(), Some("Un vrai clic n'arrête pas cette macro."));
        // What the engine says of it.
        let snapshot = Snapshot {
            externals: vec![ExternalSnapshot {
                id: MacroId("exemple"),
                status: MacroStatus::Active,
                hotkey: Some(f11),
                hotkey_label: "F11".into(),
                values: vec![300],
                line: ExternalLine::Active(Some("12/300".into())),
                badge: Some(Badge { color: BadgeColor::Green, text: "Clic".into() }),
                suspended: true,
                alert: None,
            }],
            ..Snapshot::default()
        };
        let state = hud_state(&snapshot, CATALOG, &externals, false, None);
        let view = &state.external_macros[0];
        assert_eq!(
            (view.status.clone(), view.line.clone()),
            (MacroStatus::Active, ExternalLine::Active(Some("12/300".into())))
        );
        assert_eq!(view.badge.as_ref().map(|b| b.color), Some(BadgeColor::Green));
        assert!(view.suspended);
        assert_eq!(view.settings[0].value, 300);
        // Kept as the engine has it.
        let setup = external_setup(&settings, &externals);
        assert_eq!((setup[0].hotkey, setup[0].values.clone()), (Some(f11), vec![200]));
    }

    #[test]
    fn the_first_state_comes_from_the_settings() {
        let mut settings = Settings::defaults(CATALOG);
        settings.collapsed = true;
        settings.macros[0].options = MacroOptions::HoldClick { button: MouseButton::Middle };
        let state = initial_state(&settings, &[]);
        assert!(state.collapsed);
        assert_eq!(state.macros[0].hotkey, "F8");
        assert_eq!(state.macros[0].status, MacroStatus::Stopped);
        assert_eq!(state.macros[0].options, MacroOptions::HoldClick { button: MouseButton::Middle });
        let setup = macro_setup(&settings, CATALOG);
        assert_eq!(setup[0].hotkey, Hotkey::F8);
        // The second macro, « Multi-clic », with its defaults.
        assert_eq!(state.macros[1].hotkey, "F9");
        assert_eq!(setup[1].options, MacroOptions::MultiClick { button: MouseButton::Left, rate: ClickRate::DEFAULT });
    }
}
