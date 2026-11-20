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

/// The options of a macro, by kind of macro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroOptions {
    /// « Maintenir le clic »: the held button.
    HoldClick { button: MouseButton },
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
    /// Start a macro. « Maintenir le clic » started from the HUD presses its button after a short
    /// delay (the time to move the mouse away from the HUD), its hotkey presses it at once.
    Start(MacroId),
    /// Stop a macro (releases what it holds).
    Stop(MacroId),
    /// Wait for the next key or combination, which becomes the macro's hotkey.
    StartCapture(MacroId),
    /// Stop waiting (Échap does the same, from the keyboard).
    CancelCapture,
    /// Change the options of a macro (the held button). Changing them while the macro runs stops it.
    SetOptions(MacroId, MacroOptions),
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
    };
    let base = HudState {
        app_name: "MacroHUD",
        version: "0.1.7",
        macros: vec![hold_click.clone()],
        capturing: None,
        message: None,
        collapsed: false,
    };
    let with = |change: &dyn Fn(&mut HudState)| {
        let mut state = base.clone();
        change(&mut state);
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
                })
            }),
        ),
    ]
}
