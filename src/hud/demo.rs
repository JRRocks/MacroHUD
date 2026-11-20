//! Made-up views for the render tests: the six views of the mock-up (a–f) and the states the
//! mock-up did not show. Content follows `docs/CONTRAT-HUD.md`.

use super::view::{
    HudView, Level, MacroIcon, MacroOptions, MacroState, MacroView, Message, MouseButton,
};

/// One view to render: its letter, data, and the line whose settings are unfolded.
pub(crate) struct Demo {
    pub key: &'static str,
    pub view: HudView,
    pub open: Option<usize>,
}

pub(crate) fn hold_click(state: MacroState) -> MacroView {
    MacroView {
        name: "Maintenir le clic".to_owned(),
        description: "Garde un bouton de la souris enfoncé".to_owned(),
        state,
        shortcut: "F8".to_owned(),
        options: MacroOptions::HeldButton(MouseButton::Left),
        available: true,
        icon: MacroIcon::Mouse,
    }
}

fn coming(name: &str, shortcut: &str, summary: &str, icon: MacroIcon) -> MacroView {
    MacroView {
        name: name.to_owned(),
        description: String::new(),
        state: MacroState::Stopped,
        shortcut: shortcut.to_owned(),
        options: MacroOptions::Summary(summary.to_owned()),
        available: false,
        icon,
    }
}

fn view(macros: Vec<MacroView>) -> HudView {
    HudView {
        version: "0.1.7".to_owned(),
        macros,
        capture: None,
        message: None,
        collapsed: false,
    }
}

fn refused() -> Message {
    Message {
        level: Level::Warning,
        title: "Raccourci refusé".to_owned(),
        detail: "Alt+Tab est déjà pris par Windows.\nF8 reste actif.".to_owned(),
        macro_index: Some(0),
    }
}

/// The six views of the mock-up, in order.
pub(crate) fn mockup_views() -> Vec<Demo> {
    let stopped = || hold_click(MacroState::Stopped);
    let mut capture = view(vec![stopped()]);
    capture.capture = Some(0);
    let mut refused_view = view(vec![stopped()]);
    refused_view.message = Some(refused());
    let mut folded = view(vec![stopped()]);
    folded.collapsed = true;
    let mut folded_on = view(vec![hold_click(MacroState::Active)]);
    folded_on.collapsed = true;
    vec![
        Demo {
            key: "a",
            view: view(vec![stopped()]),
            open: Some(0),
        },
        Demo {
            key: "b",
            view: view(vec![hold_click(MacroState::Active)]),
            open: None,
        },
        Demo {
            key: "c",
            view: capture,
            open: Some(0),
        },
        Demo {
            key: "d",
            view: refused_view,
            open: Some(0),
        },
        Demo {
            key: "e",
            view: view(vec![
                stopped(),
                coming("Clic répété", "F9", "100 ms", MacroIcon::RepeatedClick),
                coming("Touche maintenue", "F10", "Espace", MacroIcon::Key),
            ]),
            open: None,
        },
        Demo {
            key: "f1",
            view: folded,
            open: None,
        },
        Demo {
            key: "f2",
            view: folded_on,
            open: None,
        },
    ]
}

/// States the mock-up did not show: drawn with the same parts, to be validated.
pub(crate) fn extra_views() -> Vec<Demo> {
    let unavailable = hold_click(MacroState::Unavailable {
        reason: "Windows a refusé le clic simulé".to_owned(),
    });
    let mut saved_macro = hold_click(MacroState::Stopped);
    saved_macro.shortcut = "F7".to_owned();
    let mut saved = view(vec![saved_macro]);
    saved.message = Some(Message {
        level: Level::Info,
        title: "Raccourci enregistré".to_owned(),
        detail: "F7 remplace F8.".to_owned(),
        macro_index: Some(0),
    });
    let mut error = view(vec![hold_click(MacroState::Stopped)]);
    error.message = Some(Message {
        level: Level::Error,
        title: "Erreur".to_owned(),
        detail: "Le clic n’a pas pu être envoyé.".to_owned(),
        macro_index: None,
    });
    vec![
        Demo {
            key: "g",
            view: view(vec![unavailable]),
            open: None,
        },
        Demo {
            key: "h",
            view: saved,
            open: Some(0),
        },
        Demo {
            key: "i",
            view: error,
            open: None,
        },
    ]
}
