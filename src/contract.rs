//! The HUD contract (`docs/CONTRAT-HUD.md`, D-11) as Rust types: what the HUD receives
//! ([`HudState`], rebuilt for every frame) and what it may ask for ([`HudAction`]).
//!
//! The HUD (`src/hud/`) draws only from these types and acts only through these actions. If it
//! needs something that is not here, `docs/CONTRAT-HUD.md` changes first (the master session
//! decides), then this file.

/// Stable identifier of a macro; also its key in the settings file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MacroId(pub &'static str);

impl MacroId {
    /// « Maintenir le clic » (D-04).
    pub const HOLD_CLICK: MacroId = MacroId("maintenir-clic");
    /// « Multi-clic » (D-18).
    pub const MULTI_CLICK: MacroId = MacroId("multi-clic");
}

/// A mouse button that a macro can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    /// In the order the HUD offers them.
    pub const ALL: [MouseButton; 3] = [MouseButton::Left, MouseButton::Right, MouseButton::Middle];

    /// Name shown in the HUD: « Gauche », « Droit », « Milieu ».
    pub fn label(self) -> &'static str {
        match self {
            MouseButton::Left => "Gauche",
            MouseButton::Right => "Droit",
            MouseButton::Middle => "Milieu",
        }
    }

    /// The button inside a sentence: « le bouton gauche ».
    pub fn phrase(self) -> &'static str {
        match self {
            MouseButton::Left => "le bouton gauche",
            MouseButton::Right => "le bouton droit",
            MouseButton::Middle => "le bouton du milieu",
        }
    }
}

/// The speed of « Multi-clic » (D-22): a whole number of clicks per second, from [`ClickRate::MIN`]
/// to [`ClickRate::MAX`]. A value outside cannot be made: [`ClickRate::new`] brings it back inside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClickRate(u8);

impl ClickRate {
    /// 1 click per second.
    pub const MIN: ClickRate = ClickRate(1);
    /// 15 clicks per second: about the most a fast hand does.
    pub const MAX: ClickRate = ClickRate(15);
    /// 10 clicks per second.
    pub const DEFAULT: ClickRate = ClickRate(10);

    /// `clicks` per second, brought back between the bounds (for the HUD's speed control, P2.3).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(clicks: u32) -> ClickRate {
        let clicks = clicks.clamp(u32::from(Self::MIN.0), u32::from(Self::MAX.0));
        ClickRate(u8::try_from(clicks).unwrap_or(Self::MAX.0))
    }

    /// `clicks` per second if it lies between the bounds, else `None`.
    pub fn exact(clicks: u64) -> Option<ClickRate> {
        let clicks = u8::try_from(clicks).ok()?;
        (Self::MIN.0..=Self::MAX.0).contains(&clicks).then_some(ClickRate(clicks))
    }

    /// Clicks per second.
    pub fn per_second(self) -> u32 {
        u32::from(self.0)
    }

    /// The speed as the HUD writes it: « 10 clics/s », « 1 clic/s ».
    pub fn label(self) -> String {
        match self.0 {
            1 => "1 clic/s".to_string(),
            n => format!("{n} clics/s"),
        }
    }

    /// Time from one press to the next.
    pub fn period(self) -> std::time::Duration {
        std::time::Duration::from_secs(1) / self.per_second()
    }
}

/// The options of a macro, by kind of macro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroOptions {
    /// « Maintenir le clic »: the held button.
    HoldClick { button: MouseButton },
    /// « Multi-clic »: the clicked button and the speed. A new speed while the macro runs counts
    /// from the next click; a new button while it runs stops it, as for « Maintenir le clic ».
    MultiClick { button: MouseButton, rate: ClickRate },
}

impl MacroOptions {
    /// The button the macro holds or clicks.
    pub fn button(self) -> MouseButton {
        match self {
            MacroOptions::HoldClick { button } | MacroOptions::MultiClick { button, .. } => button,
        }
    }

    /// The same options with another button.
    pub fn with_button(self, button: MouseButton) -> Self {
        match self {
            MacroOptions::HoldClick { .. } => MacroOptions::HoldClick { button },
            MacroOptions::MultiClick { rate, .. } => MacroOptions::MultiClick { button, rate },
        }
    }

    /// The same options with another speed (« Multi-clic » only: other options stay as they are).
    pub fn with_rate(self, rate: ClickRate) -> Self {
        match self {
            MacroOptions::MultiClick { button, .. } => MacroOptions::MultiClick { button, rate },
            other => other,
        }
    }
}

/// What a macro is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacroStatus {
    /// « arrêtée ».
    Stopped,
    /// « active »: running (for « Maintenir le clic »: the button is held, or about to be).
    Active,
    /// « indisponible »: cannot run right now; the reason fits on one line.
    Unavailable(String),
}

/// One macro, as the HUD shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct MacroView {
    pub id: MacroId,
    /// « Maintenir le clic ».
    pub name: &'static str,
    /// One line.
    pub description: &'static str,
    pub status: MacroStatus,
    /// Its hotkey as readable text: « F8 », « Ctrl+Alt+M ».
    pub hotkey: String,
    pub options: MacroOptions,
    /// False for a macro « à venir »: shown greyed out, nothing can be asked of it.
    pub available: bool,
    /// An active « Multi-clic » whose clicks wait because the mouse is on the HUD, or a press on the
    /// HUD is not released yet (D-22); they go on once it leaves. Meanwhile a click on the HUD acts
    /// there at once (D-27). Always false otherwise.
    pub suspended: bool,
}

/// Colour of the step badge of an external macro (`pastille <couleur> <texte>` of its script).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeColor {
    /// `ambre`: the HUD's warning colour.
    Amber,
    /// `verte`: the green of an active macro.
    Green,
    /// `bleue`: the HUD's information colour.
    Blue,
    /// `grise`: secondary text.
    Grey,
}

/// The step badge of an active external macro, between its state text and its hotkey: « Attente »,
/// « Clic ! », « Pause 8 s ».
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    pub color: BadgeColor,
    pub text: String,
}

/// The amber box under the line of an external macro, like « Raccourci refusé »: what its script
/// said (`alerte <titre> | <texte>`), or MacroHUD's own word (« « Exemple » ne peut pas démarrer. »).
/// It stays until the macro's next start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alert {
    pub title: String,
    pub text: String,
}

/// An integer setting of an external macro, shown as a slider like the speed of « Multi-clic ».
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntSetting {
    /// Its key, in the declaration and for the script (`reglage <cle> <valeur>`).
    pub key: String,
    /// « Limite de clics ».
    pub name: String,
    /// « clics », written after the value (« 100 clics »); may be empty.
    pub unit: String,
    pub min: i64,
    pub max: i64,
    /// The slider moves by this much (at least 1).
    pub step: i64,
    /// The current value: between `min` and `max`, on a step from `min`.
    pub value: i64,
}

// (Used by the HUD, P4.5, and by the engine, in the next commits of P4.4.)
#[cfg_attr(not(test), allow(dead_code))]
impl IntSetting {
    /// `value` brought between the bounds, onto the nearest step from `min` (what
    /// [`HudAction::SetSetting`] keeps).
    pub fn snap(&self, value: i64) -> i64 {
        let step = self.step.max(1);
        let top = self.min + (self.max - self.min).max(0) / step * step;
        let steps = (value.clamp(self.min, top) - self.min + step / 2) / step;
        (self.min + steps * step).min(top)
    }

    /// The value and its unit: « 100 clics », or « 100 » without a unit.
    pub fn label(&self) -> String {
        if self.unit.is_empty() { self.value.to_string() } else { format!("{} {}", self.value, self.unit) }
    }
}

/// What the line of an external macro says, after its state word (`docs/MACROS-EXTERNES.md`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalLine {
    /// « ■ Arrêtée · texte »: `etat_arretee` of its declaration, the settings' values written in;
    /// `None`: « ■ Arrêtée » alone (no such text, or the macro could not start).
    Stopped(Option<String>),
    /// « ● Active · texte »: the last `etat` of its script; `None` before the first one.
    Active(Option<String>),
    /// « ❚❚ En pause · texte »: the script waits for a condition of its own (`attente`), until it
    /// says `reprise`.
    Waiting(String),
    /// « ✓ Terminée · texte »: the script stopped by itself (`fin`); blue border, until the next
    /// start.
    Finished(String),
}

/// A macro external to MacroHUD (D-32, D-36): a script of the PC, declared outside the repository,
/// that MacroHUD starts, stops and shows after its own macros. Its line has the same shape as the
/// others (D-37).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalMacroView {
    /// Its id: the name of its declaration file, without `.json`.
    pub id: MacroId,
    /// « Exemple ».
    pub name: String,
    /// One line, from its declaration (may be empty).
    pub description: String,
    /// Stopped; active (its script runs, or is being stopped); or unavailable, with the reason (its
    /// hotkey was taken at start), as for the other macros.
    pub status: MacroStatus,
    /// Its hotkey as readable text, « F10 »; empty when it has none (« Changer » gives one).
    pub hotkey: String,
    /// The text after the state word. Active status: `Active` or `Waiting`; otherwise `Stopped` or
    /// `Finished`.
    pub line: ExternalLine,
    /// The step badge (`pastille`), while active only.
    pub badge: Option<Badge>,
    /// Active, and the mouse on the HUD (or a press on the HUD not released yet): MacroHUD told the
    /// script to pause (`pause`, D-22) until the mouse leaves. A click on the HUD acts there.
    pub suspended: bool,
    /// The amber box under the line.
    pub alert: Option<Alert>,
    /// Its integer settings, in the order of its declaration (slider, value, unit).
    pub settings: Vec<IntSetting>,
    /// The help line of its settings (`info`).
    pub info: Option<String>,
}

/// Level of a one-off message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
}

/// The sort of a one-off message, for the HUD to show it the right way (D-16).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageKind {
    /// Any message: shown with a title from its level.
    General,
    /// « Raccourci refusé »: `key` (« Alt+Tab ») was refused as the new hotkey of the macro the
    /// message concerns, whose hotkey does not change (the text says so). Always a warning. The
    /// HUD offers « Choisir une autre touche », that is [`HudAction::StartCapture`] for that macro.
    HotkeyRefused { key: String },
}

/// A one-off message (hotkey saved, hotkey refused, error...), in French.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub level: MessageLevel,
    pub text: String,
    /// The macro the message concerns, if it concerns one.
    pub about: Option<MacroId>,
    pub kind: MessageKind,
}

impl Message {
    /// A general message, about no macro in particular.
    pub fn new(level: MessageLevel, text: impl Into<String>) -> Self {
        Message { level, text: text.into(), about: None, kind: MessageKind::General }
    }

    /// The same message, about macro `id`.
    pub fn concerning(mut self, id: MacroId) -> Self {
        self.about = Some(id);
        self
    }

    /// « Raccourci refusé » for macro `id`: `key` was refused; `text` says why, shortly, and which
    /// hotkey stays (« Alt+Tab est déjà pris par Windows. F8 reste actif. »).
    pub fn hotkey_refused(id: MacroId, key: impl Into<String>, text: impl Into<String>) -> Self {
        Message {
            level: MessageLevel::Warning,
            text: text.into(),
            about: Some(id),
            kind: MessageKind::HotkeyRefused { key: key.into() },
        }
    }
}

/// Everything the HUD receives.
#[derive(Clone, Debug, PartialEq)]
pub struct HudState {
    /// « MacroHUD ».
    pub app_name: &'static str,
    /// « 0.1.7 » (« 0.1.0-dev » for a local build).
    pub version: &'static str,
    /// The macros, in display order.
    pub macros: Vec<MacroView>,
    /// The external macros (D-32), shown after `macros`, in the alphabetical order of their ids.
    pub external_macros: Vec<ExternalMacroView>,
    /// The macro waiting for its new hotkey (the next key or combination), or none.
    pub capturing: Option<MacroId>,
    /// The current one-off message, if any (it disappears by itself after a few seconds).
    pub message: Option<Message>,
    /// The HUD is collapsed (true) or expanded (false).
    pub collapsed: bool,
}

impl HudState {
    /// « MacroHUD 0.1.7 ».
    pub fn title(&self) -> String {
        format!("{} {}", self.app_name, self.version)
    }
}

/// Everything the HUD may ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum HudAction {
    /// Start a macro, after stopping the one that runs, if any (one at a time, D-20). Started from
    /// the HUD, a macro of MacroHUD acts after a short delay (the time to move the mouse away from
    /// the HUD); its hotkey starts it at once. An external macro starts at once: its script is told
    /// to pause while the mouse is on the HUD.
    Start(MacroId),
    /// Stop a macro (releases what it holds; « Multi-clic » sends no more clicks; an external
    /// macro's script is asked to stop, then stopped by force after 2 s).
    Stop(MacroId),
    /// Wait for the next key or combination, which becomes the macro's hotkey.
    StartCapture(MacroId),
    /// Stop waiting (Échap does the same, from the keyboard).
    CancelCapture,
    /// Change the options of a macro (the button; the speed of « Multi-clic »). A new button while
    /// the macro runs stops it; a new speed while « Multi-clic » runs counts from the next click.
    SetOptions(MacroId, MacroOptions),
    /// Change an integer setting of an external macro: its `key`, the new value (brought within its
    /// bounds and onto its step, see [`IntSetting::snap`]). The value goes to the script at once if
    /// it runs (D-34), and is kept in the settings. (Asked by the HUD of P4.5.)
    #[cfg_attr(not(test), allow(dead_code))]
    SetSetting(MacroId, String, i64),
    /// Collapse (true) or expand (false) the HUD.
    SetCollapsed(bool),
    /// Move the window with the mouse: send it when a drag starts on the HUD (the pointer is still
    /// down). The position is remembered.
    StartDrag,
    /// Quit MacroHUD (everything held is released first).
    Quit,
}

/// Representative states, one per situation the HUD must handle: for previews and tests.
#[cfg_attr(not(test), allow(dead_code))]
pub fn samples() -> Vec<(&'static str, HudState)> {
    let hold_click = MacroView {
        id: MacroId::HOLD_CLICK,
        name: "Maintenir le clic",
        description: "Garde un bouton de la souris enfoncé ; le raccourci ou un vrai clic le relâche.",
        status: MacroStatus::Stopped,
        hotkey: "F8".into(),
        options: MacroOptions::HoldClick { button: MouseButton::Left },
        available: true,
        suspended: false,
    };
    let multi_click = MacroView {
        id: MacroId::MULTI_CLICK,
        name: "Multi-clic",
        description: "Clique encore et encore, à la rapidité choisie ; le raccourci ou un vrai clic l'arrête.",
        status: MacroStatus::Stopped,
        hotkey: "F9".into(),
        options: MacroOptions::MultiClick { button: MouseButton::Left, rate: ClickRate::DEFAULT },
        available: true,
        suspended: false,
    };
    // The external macro of the mock-up (`maquettes/macro-externe/apercu.html`), stopped.
    let example = ExternalMacroView {
        id: MacroId("exemple"),
        name: "Exemple".into(),
        description: "Clique toutes les deux secondes".into(),
        status: MacroStatus::Stopped,
        hotkey: "F10".into(),
        line: ExternalLine::Stopped(Some("limite 100 clics".into())),
        badge: None,
        suspended: false,
        alert: None,
        settings: vec![IntSetting {
            key: "limite".into(),
            name: "Limite de clics".into(),
            unit: "clics".into(),
            min: 10,
            max: 500,
            step: 10,
            value: 100,
        }],
        info: Some(
            "Garde la fenêtre visée au premier plan. Un vrai clic n'arrête pas cette macro : F10 ou ▶ l'arrêtent."
                .into(),
        ),
    };
    let base = HudState {
        app_name: "MacroHUD",
        version: "0.1.7",
        macros: vec![hold_click.clone()],
        external_macros: Vec::new(),
        capturing: None,
        message: None,
        collapsed: false,
    };
    let with = |change: &dyn Fn(&mut HudState)| {
        let mut state = base.clone();
        change(&mut state);
        state
    };
    let three = |change: &dyn Fn(&mut ExternalMacroView)| {
        let mut state = base.clone();
        state.macros.push(multi_click.clone());
        let mut external = example.clone();
        change(&mut external);
        state.external_macros.push(external);
        state
    };
    vec![
        ("arrêtée", base.clone()),
        (
            "active, bouton droit",
            with(&|s| {
                s.macros[0].status = MacroStatus::Active;
                s.macros[0].options = MacroOptions::HoldClick { button: MouseButton::Right };
            }),
        ),
        (
            "indisponible",
            with(&|s| {
                s.macros[0].status = MacroStatus::Unavailable(
                    "F8 est déjà pris par un autre programme : choisis un autre raccourci.".into(),
                )
            }),
        ),
        ("capture d'un raccourci", with(&|s| s.capturing = Some(MacroId::HOLD_CLICK))),
        ("raccourci long", with(&|s| s.macros[0].hotkey = "Ctrl+Alt+Maj+Pavé num. séparateur".into())),
        (
            "message d'information",
            with(&|s| {
                s.message = Some(
                    Message::new(MessageLevel::Info, "Nouveau raccourci de « Maintenir le clic » : Ctrl+Alt+M.")
                        .concerning(MacroId::HOLD_CLICK),
                )
            }),
        ),
        (
            "avertissement",
            with(&|s| {
                s.message = Some(
                    Message::new(MessageLevel::Warning, "Sors la souris du HUD avant d'appuyer sur F8.")
                        .concerning(MacroId::HOLD_CLICK),
                )
            }),
        ),
        (
            "raccourci refusé",
            with(&|s| {
                s.message = Some(Message::hotkey_refused(
                    MacroId::HOLD_CLICK,
                    "Alt+Tab",
                    "Alt+Tab est déjà pris par Windows. F8 reste actif.",
                ))
            }),
        ),
        (
            "erreur",
            with(&|s| {
                s.message = Some(Message::new(
                    MessageLevel::Error,
                    "Impossible de surveiller la souris (SetWindowsHookEx, erreur 5) : rien n'a été enfoncé.",
                ))
            }),
        ),
        (
            "raccourci refusé, replié",
            with(&|s| {
                s.collapsed = true;
                s.message = Some(Message::hotkey_refused(
                    MacroId::HOLD_CLICK,
                    "Ctrl+C",
                    "Ctrl+C sert à copier. F8 reste actif.",
                ))
            }),
        ),
        ("replié", with(&|s| s.collapsed = true)),
        (
            "replié, active",
            with(&|s| {
                s.collapsed = true;
                s.macros[0].status = MacroStatus::Active;
            }),
        ),
        (
            "plusieurs macros, dont une à venir",
            with(&|s| {
                s.macros.push(MacroView {
                    id: MacroId("exemple-a-venir"),
                    name: "Macro à venir",
                    description: "Exemple de macro pas encore disponible.",
                    status: MacroStatus::Stopped,
                    hotkey: "F9".into(),
                    options: MacroOptions::HoldClick { button: MouseButton::Left },
                    available: false,
                    suspended: false,
                })
            }),
        ),
        ("deux macros", with(&|s| s.macros.push(multi_click.clone()))),
        (
            "multi-clic actif",
            with(&|s| {
                s.macros.push(multi_click.clone());
                s.macros[1].status = MacroStatus::Active;
            }),
        ),
        (
            "multi-clic suspendu (souris sur le HUD)",
            with(&|s| {
                s.macros.push(multi_click.clone());
                s.macros[1].status = MacroStatus::Active;
                s.macros[1].suspended = true;
            }),
        ),
        (
            "multi-clic aux bornes de la rapidité, bouton droit",
            with(&|s| {
                s.macros.push(multi_click.clone());
                s.macros[1].options = MacroOptions::MultiClick { button: MouseButton::Right, rate: ClickRate::MAX };
            }),
        ),
        (
            "multi-clic indisponible",
            with(&|s| {
                s.macros.push(multi_click.clone());
                s.macros[0].hotkey = "F9".into();
                s.macros[1].status = MacroStatus::Unavailable(
                    "F9 est déjà le raccourci de « Maintenir le clic » : choisis-en un autre.".into(),
                );
            }),
        ),
        (
            "replié, multi-clic actif",
            with(&|s| {
                s.collapsed = true;
                s.macros.push(multi_click.clone());
                s.macros[1].status = MacroStatus::Active;
            }),
        ),
        // The views of the external macro's mock-up (D-37), with the three macros.
        ("trois macros, macro externe arrêtée", three(&|_| {})),
        (
            "macro externe active, pastille ambre",
            three(&|e| {
                e.status = MacroStatus::Active;
                e.line = ExternalLine::Active(Some("12/100".into()));
                e.badge = Some(Badge { color: BadgeColor::Amber, text: "Attente".into() });
            }),
        ),
        (
            "macro externe active, pastille verte",
            three(&|e| {
                e.status = MacroStatus::Active;
                e.line = ExternalLine::Active(Some("12/100".into()));
                e.badge = Some(Badge { color: BadgeColor::Green, text: "Clic !".into() });
            }),
        ),
        (
            "macro externe active, pastille grise",
            three(&|e| {
                e.status = MacroStatus::Active;
                e.line = ExternalLine::Active(Some("13/100".into()));
                e.badge = Some(Badge { color: BadgeColor::Grey, text: "Pause 8 s".into() });
            }),
        ),
        (
            "macro externe active, pastille bleue",
            three(&|e| {
                e.status = MacroStatus::Active;
                e.line = ExternalLine::Active(Some("13/100".into()));
                e.badge = Some(Badge { color: BadgeColor::Blue, text: "Relance".into() });
            }),
        ),
        (
            "macro externe active, avant son premier état",
            three(&|e| {
                e.status = MacroStatus::Active;
                e.line = ExternalLine::Active(None);
            }),
        ),
        (
            "macro externe en attente",
            three(&|e| {
                e.status = MacroStatus::Active;
                e.line = ExternalLine::Waiting("attend sa fenêtre".into());
            }),
        ),
        (
            "macro externe suspendue (souris sur le HUD)",
            three(&|e| {
                e.status = MacroStatus::Active;
                e.line = ExternalLine::Active(Some("12/100".into()));
                e.badge = Some(Badge { color: BadgeColor::Amber, text: "Attente".into() });
                e.suspended = true;
            }),
        ),
        ("macro externe terminée", three(&|e| e.line = ExternalLine::Finished("100/100 clics".into()))),
        (
            "replié, macro externe active",
            three(&|e| {
                e.status = MacroStatus::Active;
                e.line = ExternalLine::Active(Some("12/100".into()));
                e.badge = Some(Badge { color: BadgeColor::Amber, text: "Attente".into() });
            })
            .folded(),
        ),
        (
            "macro externe, alerte de son script",
            three(&|e| {
                e.alert = Some(Alert {
                    title: "La fenêtre visée est en plein écran.".into(),
                    text: "Passe-la en fenêtre : en plein écran, la macro ne voit pas l'écran.".into(),
                })
            }),
        ),
        (
            "macro externe qui ne peut pas démarrer",
            three(&|e| {
                e.line = ExternalLine::Stopped(None);
                e.alert = Some(Alert {
                    title: "« Exemple » ne peut pas démarrer.".into(),
                    text: "Son script est introuvable sur ce PC. Les autres macros marchent.".into(),
                });
            }),
        ),
        (
            "macro externe sans raccourci ni réglage",
            three(&|e| {
                e.hotkey = String::new();
                e.settings.clear();
                e.info = None;
                e.line = ExternalLine::Stopped(None);
            }),
        ),
    ]
}

impl HudState {
    /// The same state, collapsed.
    #[cfg_attr(not(test), allow(dead_code))]
    fn folded(mut self) -> Self {
        self.collapsed = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn integer_settings_stay_on_their_steps_between_their_bounds() {
        let limit = IntSetting {
            key: "limite".into(),
            name: "Limite de clics".into(),
            unit: "clics".into(),
            min: 10,
            max: 500,
            step: 10,
            value: 100,
        };
        assert_eq!(limit.label(), "100 clics");
        assert_eq!(IntSetting { unit: String::new(), ..limit.clone() }.label(), "100");
        for (asked, kept) in [(100, 100), (104, 100), (105, 110), (0, 10), (-50, 10), (499, 500), (9999, 500)] {
            assert_eq!(limit.snap(asked), kept, "{asked}");
        }
        // A top that is not on a step: the last step below it.
        let odd = IntSetting { min: 0, max: 25, step: 10, ..limit.clone() };
        assert_eq!((odd.snap(25), odd.snap(24), odd.snap(14)), (20, 20, 10));
        // A step of 0 or less counts as 1.
        let loose = IntSetting { min: 1, max: 3, step: 0, ..limit };
        assert_eq!((loose.snap(2), loose.snap(7)), (2, 3));
    }

    #[test]
    fn the_external_macro_samples_follow_the_mock_up() {
        let samples = samples();
        let find = |name: &str| samples.iter().find(|(n, _)| *n == name).map(|(_, s)| s).expect(name);
        let stopped = find("trois macros, macro externe arrêtée");
        assert_eq!(stopped.macros.len() + stopped.external_macros.len(), 3);
        let example = &stopped.external_macros[0];
        assert_eq!(example.line, ExternalLine::Stopped(Some("limite 100 clics".into())));
        assert_eq!(example.settings[0].label(), "100 clics");
        // What the slider asks for: a value on its steps.
        let asked = HudAction::SetSetting(example.id, example.settings[0].key.clone(), example.settings[0].snap(123));
        assert_eq!(asked, HudAction::SetSetting(MacroId("exemple"), "limite".into(), 120));
        assert!(find("replié, macro externe active").collapsed);
        for (name, state) in &samples {
            for e in &state.external_macros {
                let active = e.status == MacroStatus::Active;
                let active_line = matches!(e.line, ExternalLine::Active(_) | ExternalLine::Waiting(_));
                assert_eq!(active, active_line, "{name}: state and line agree");
                assert!(active || e.badge.is_none() && !e.suspended, "{name}: badge and pause while active only");
            }
        }
    }

    #[test]
    fn click_rates_stay_between_their_bounds() {
        assert_eq!(ClickRate::new(0), ClickRate::MIN);
        assert_eq!(ClickRate::new(16), ClickRate::MAX);
        assert_eq!(ClickRate::new(u32::MAX), ClickRate::MAX);
        assert_eq!(ClickRate::new(7).per_second(), 7);
        assert_eq!(ClickRate::exact(15), Some(ClickRate::MAX));
        assert_eq!(ClickRate::exact(0), None);
        assert_eq!(ClickRate::exact(16), None);
        assert_eq!(ClickRate::exact(256 + 10), None);
        assert_eq!(
            (ClickRate::MIN.per_second(), ClickRate::DEFAULT.per_second(), ClickRate::MAX.per_second()),
            (1, 10, 15)
        );
        assert_eq!(ClickRate::DEFAULT.period(), Duration::from_millis(100));
        assert_eq!(
            (ClickRate::MIN.label(), ClickRate::MAX.label()),
            ("1 clic/s".to_string(), "15 clics/s".to_string())
        );
        assert_eq!(ClickRate::MIN.period(), Duration::from_secs(1));
    }

    #[test]
    fn options_change_one_thing_at_a_time() {
        let hold = MacroOptions::HoldClick { button: MouseButton::Left };
        let multi = MacroOptions::MultiClick { button: MouseButton::Left, rate: ClickRate::DEFAULT };
        assert_eq!(hold.with_button(MouseButton::Right), MacroOptions::HoldClick { button: MouseButton::Right });
        assert_eq!(hold.with_rate(ClickRate::MAX), hold);
        assert_eq!(
            multi.with_button(MouseButton::Middle),
            MacroOptions::MultiClick { button: MouseButton::Middle, rate: ClickRate::DEFAULT }
        );
        assert_eq!(
            multi.with_rate(ClickRate::MIN),
            MacroOptions::MultiClick { button: MouseButton::Left, rate: ClickRate::MIN }
        );
        assert_eq!(multi.button(), MouseButton::Left);
    }
}
