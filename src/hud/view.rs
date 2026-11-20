//! What the HUD shows and what it can ask for, mirroring `docs/CONTRAT-HUD.md`.
//!
//! These are plain view types: the integration fills a [`HudView`] from the program state each
//! frame and executes the [`HudAction`]s returned by [`super::Hud::show`]. Nothing here talks to the
//! engine.

/// Everything the HUD receives (contract: « Ce que le HUD reçoit »).
#[derive(Clone, Debug, PartialEq)]
pub struct HudView {
    /// Version shown next to the name, e.g. `"0.1.7"`.
    pub version: String,
    /// The macros, in display order.
    pub macros: Vec<MacroView>,
    /// Index of the macro waiting for a new shortcut, if any.
    pub capture: Option<usize>,
    /// One-shot message (shortcut saved, refused, error).
    pub message: Option<Message>,
    /// Folded into the small pill.
    pub collapsed: bool,
}

/// One macro as the HUD shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct MacroView {
    /// « Maintenir le clic ».
    pub name: String,
    /// One-line description. The compact row has no room for it; kept for accessibility labels.
    pub description: String,
    pub state: MacroState,
    /// Readable shortcut: « F8 », « Ctrl+Alt+M ».
    pub shortcut: String,
    pub options: MacroOptions,
    /// `false` for a macro « À venir »: shown greyed, nothing can be clicked.
    pub available: bool,
    /// Drawing shown instead of the ▶ button while the macro is « À venir ».
    pub icon: MacroIcon,
}

/// Kind of macro, for its small drawing. The integration maps each macro of the engine to one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroIcon {
    Mouse,
    RepeatedClick,
    Key,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacroState {
    Stopped,
    Active,
    /// Cannot run; the reason fits on one line.
    Unavailable {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacroOptions {
    /// « Maintenir le clic »: which mouse button is held.
    HeldButton(MouseButton),
    /// Options of a future macro, summarised in a few words (« 100 ms »).
    Summary(String),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    /// Order of the selector in the HUD (as in the mock-up).
    pub const ALL: [Self; 3] = [Self::Left, Self::Right, Self::Middle];

    /// Label of the selector segment.
    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Gauche",
            Self::Right => "Droit",
            Self::Middle => "Milieu",
        }
    }

    /// Same word inside a sentence (« bouton gauche »).
    pub fn word(self) -> &'static str {
        match self {
            Self::Left => "gauche",
            Self::Right => "droit",
            Self::Middle => "milieu",
        }
    }
}

/// A one-shot message with its level.
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub level: Level,
    /// First line, in bold: « Raccourci refusé ».
    pub title: String,
    /// Explanation; `\n` starts a new line. Long lines wrap.
    pub detail: String,
    /// Macro the message is about (a refused shortcut): the message then sits under that macro
    /// and offers « Choisir une autre touche ».
    pub macro_index: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

/// What the HUD can ask for (contract: « Ce que le HUD peut demander »).
///
/// Moving the window is not an action: the HUD starts the system drag itself, the integration
/// only remembers where the window ends up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudAction {
    /// Start or stop the macro.
    Toggle(usize),
    /// Wait for a new shortcut for the macro.
    StartCapture(usize),
    /// Stop waiting for a shortcut (« Annuler », same as Échap).
    CancelCapture,
    /// Change the held mouse button of the macro.
    SetButton(usize, MouseButton),
    /// Fold (`true`) or unfold (`false`) the HUD.
    SetCollapsed(bool),
    /// Quit MacroHUD (the integration releases everything first).
    Quit,
}
