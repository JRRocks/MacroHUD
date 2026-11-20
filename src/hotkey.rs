//! Hotkeys: a key plus modifiers, their French names (« F8 », « Ctrl+Alt+M », « Arrêt défil. »)
//! and the rules a new hotkey must follow.
//!
//! Keys are Windows virtual-key codes (VK_*), defined here so that this logic is tested on Linux.

/// A Windows virtual-key code.
pub type Vk = u16;

/// The virtual-key codes MacroHUD names or treats specially (values from WinUser.h).
#[allow(dead_code)] // a table: not every code is used by name
pub mod vk {
    use super::Vk;

    pub const LBUTTON: Vk = 0x01;
    pub const RBUTTON: Vk = 0x02;
    pub const CANCEL: Vk = 0x03;
    pub const MBUTTON: Vk = 0x04;
    pub const XBUTTON2: Vk = 0x06;
    pub const BACK: Vk = 0x08;
    pub const TAB: Vk = 0x09;
    pub const CLEAR: Vk = 0x0C;
    pub const RETURN: Vk = 0x0D;
    pub const SHIFT: Vk = 0x10;
    pub const CONTROL: Vk = 0x11;
    pub const MENU: Vk = 0x12;
    pub const PAUSE: Vk = 0x13;
    pub const CAPITAL: Vk = 0x14;
    pub const ESCAPE: Vk = 0x1B;
    pub const SPACE: Vk = 0x20;
    pub const PRIOR: Vk = 0x21;
    pub const NEXT: Vk = 0x22;
    pub const END: Vk = 0x23;
    pub const HOME: Vk = 0x24;
    pub const LEFT: Vk = 0x25;
    pub const UP: Vk = 0x26;
    pub const RIGHT: Vk = 0x27;
    pub const DOWN: Vk = 0x28;
    pub const SELECT: Vk = 0x29;
    pub const PRINT: Vk = 0x2A;
    pub const EXECUTE: Vk = 0x2B;
    pub const SNAPSHOT: Vk = 0x2C;
    pub const INSERT: Vk = 0x2D;
    pub const DELETE: Vk = 0x2E;
    pub const HELP: Vk = 0x2F;
    pub const KEY_0: Vk = 0x30;
    pub const KEY_9: Vk = 0x39;
    pub const KEY_A: Vk = 0x41;
    pub const KEY_B: Vk = 0x42;
    pub const KEY_C: Vk = 0x43;
    pub const KEY_E: Vk = 0x45;
    pub const KEY_M: Vk = 0x4D;
    pub const KEY_S: Vk = 0x53;
    pub const KEY_V: Vk = 0x56;
    pub const KEY_X: Vk = 0x58;
    pub const KEY_Y: Vk = 0x59;
    pub const KEY_Z: Vk = 0x5A;
    pub const LWIN: Vk = 0x5B;
    pub const RWIN: Vk = 0x5C;
    pub const APPS: Vk = 0x5D;
    pub const SLEEP: Vk = 0x5F;
    pub const NUMPAD0: Vk = 0x60;
    pub const NUMPAD9: Vk = 0x69;
    pub const MULTIPLY: Vk = 0x6A;
    pub const ADD: Vk = 0x6B;
    pub const SEPARATOR: Vk = 0x6C;
    pub const SUBTRACT: Vk = 0x6D;
    pub const DECIMAL: Vk = 0x6E;
    pub const DIVIDE: Vk = 0x6F;
    pub const F1: Vk = 0x70;
    pub const F2: Vk = 0x71;
    pub const F3: Vk = 0x72;
    pub const F4: Vk = 0x73;
    pub const F5: Vk = 0x74;
    pub const F6: Vk = 0x75;
    pub const F7: Vk = 0x76;
    pub const F8: Vk = 0x77;
    pub const F9: Vk = 0x78;
    pub const F10: Vk = 0x79;
    pub const F11: Vk = 0x7A;
    pub const F12: Vk = 0x7B;
    pub const F24: Vk = 0x87;
    pub const NUMLOCK: Vk = 0x90;
    pub const SCROLL: Vk = 0x91;
    pub const LSHIFT: Vk = 0xA0;
    pub const RSHIFT: Vk = 0xA1;
    pub const LCONTROL: Vk = 0xA2;
    pub const RCONTROL: Vk = 0xA3;
    pub const LMENU: Vk = 0xA4;
    pub const RMENU: Vk = 0xA5;
    pub const BROWSER_BACK: Vk = 0xA6;
    pub const BROWSER_FORWARD: Vk = 0xA7;
    pub const BROWSER_REFRESH: Vk = 0xA8;
    pub const BROWSER_STOP: Vk = 0xA9;
    pub const BROWSER_SEARCH: Vk = 0xAA;
    pub const BROWSER_FAVORITES: Vk = 0xAB;
    pub const BROWSER_HOME: Vk = 0xAC;
    pub const VOLUME_MUTE: Vk = 0xAD;
    pub const VOLUME_DOWN: Vk = 0xAE;
    pub const VOLUME_UP: Vk = 0xAF;
    pub const MEDIA_NEXT_TRACK: Vk = 0xB0;
    pub const MEDIA_PREV_TRACK: Vk = 0xB1;
    pub const MEDIA_STOP: Vk = 0xB2;
    pub const MEDIA_PLAY_PAUSE: Vk = 0xB3;
    pub const LAUNCH_MAIL: Vk = 0xB4;
    pub const LAUNCH_MEDIA_SELECT: Vk = 0xB5;
    pub const LAUNCH_APP1: Vk = 0xB6;
    pub const LAUNCH_APP2: Vk = 0xB7;
    pub const OEM_1: Vk = 0xBA;
    pub const OEM_3: Vk = 0xC0;
    pub const OEM_4: Vk = 0xDB;
    pub const OEM_8: Vk = 0xDF;
    pub const OEM_102: Vk = 0xE2;
    pub const PROCESSKEY: Vk = 0xE5;
    pub const PACKET: Vk = 0xE7;
}

/// A modifier key, whichever side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Win,
}

/// The modifier a key is, if it is one.
pub fn modifier_of(key: Vk) -> Option<Modifier> {
    match key {
        vk::CONTROL | vk::LCONTROL | vk::RCONTROL => Some(Modifier::Ctrl),
        vk::MENU | vk::LMENU | vk::RMENU => Some(Modifier::Alt),
        vk::SHIFT | vk::LSHIFT | vk::RSHIFT => Some(Modifier::Shift),
        vk::LWIN | vk::RWIN => Some(Modifier::Win),
        _ => None,
    }
}

/// The modifiers of a hotkey.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers { ctrl: false, alt: false, shift: false, win: false };

    pub fn set(&mut self, modifier: Modifier, on: bool) {
        match modifier {
            Modifier::Ctrl => self.ctrl = on,
            Modifier::Alt => self.alt = on,
            Modifier::Shift => self.shift = on,
            Modifier::Win => self.win = on,
        }
    }

    pub fn with(mut self, modifier: Modifier) -> Self {
        self.set(modifier, true);
        self
    }

    /// Ctrl, Alt or Win: with one of them, a key no longer types text.
    pub fn has_command(self) -> bool {
        self.ctrl || self.alt || self.win
    }
}

/// A key and the modifiers held with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub key: Vk,
}

impl Hotkey {
    /// The default hotkey of the first macro (D-05).
    pub const F8: Hotkey = Hotkey { modifiers: Modifiers::NONE, key: vk::F8 };

    pub const fn new(modifiers: Modifiers, key: Vk) -> Self {
        Hotkey { modifiers, key }
    }

    /// The text the user sees: « F8 », « Ctrl+Alt+M », « Win+Maj+S », « Arrêt défil. ».
    /// `layout_char` gives the character printed on keys that depend on the keyboard layout
    /// (punctuation); without it, those keys get a generic name.
    pub fn label(&self, layout_char: &dyn Fn(Vk) -> Option<char>) -> String {
        let m = self.modifiers;
        let mut parts: Vec<String> = Vec::new();
        for (on, name) in [(m.win, "Win"), (m.ctrl, "Ctrl"), (m.alt, "Alt"), (m.shift, "Maj")] {
            if on {
                parts.push(name.to_string());
            }
        }
        parts.push(key_name(self.key, layout_char));
        parts.join("+")
    }
}

/// Keys whose printed character depends on the keyboard layout (punctuation, « OEM » keys).
pub fn is_layout_key(key: Vk) -> bool {
    matches!(key, vk::OEM_1..=vk::OEM_3 | vk::OEM_4..=vk::OEM_8 | vk::OEM_102)
}

/// French names of the keys that have one (names printed on French keyboards where possible).
const NAMED_KEYS: &[(Vk, &str)] = &[
    (vk::CANCEL, "Attn"),
    (vk::BACK, "Retour arrière"),
    (vk::TAB, "Tab"),
    (vk::CLEAR, "Effacer"),
    (vk::RETURN, "Entrée"),
    (vk::SHIFT, "Maj"),
    (vk::CONTROL, "Ctrl"),
    (vk::MENU, "Alt"),
    (vk::PAUSE, "Pause"),
    (vk::CAPITAL, "Verr. maj."),
    (vk::ESCAPE, "Échap"),
    (vk::SPACE, "Espace"),
    (vk::PRIOR, "Page préc."),
    (vk::NEXT, "Page suiv."),
    (vk::END, "Fin"),
    (vk::HOME, "Origine"),
    (vk::LEFT, "Flèche gauche"),
    (vk::UP, "Flèche haut"),
    (vk::RIGHT, "Flèche droite"),
    (vk::DOWN, "Flèche bas"),
    (vk::SELECT, "Sélection"),
    (vk::PRINT, "Imprimer"),
    (vk::EXECUTE, "Exécuter"),
    (vk::SNAPSHOT, "Impr. écran"),
    (vk::INSERT, "Inser"),
    (vk::DELETE, "Suppr"),
    (vk::HELP, "Aide"),
    (vk::LWIN, "Win"),
    (vk::RWIN, "Win"),
    (vk::APPS, "Menu"),
    (vk::SLEEP, "Veille"),
    (vk::MULTIPLY, "Pavé num. *"),
    (vk::ADD, "Pavé num. +"),
    (vk::SEPARATOR, "Pavé num. séparateur"),
    (vk::SUBTRACT, "Pavé num. -"),
    (vk::DECIMAL, "Pavé num. ."),
    (vk::DIVIDE, "Pavé num. /"),
    (vk::NUMLOCK, "Verr. num."),
    (vk::SCROLL, "Arrêt défil."),
    (vk::LSHIFT, "Maj"),
    (vk::RSHIFT, "Maj"),
    (vk::LCONTROL, "Ctrl"),
    (vk::RCONTROL, "Ctrl"),
    (vk::LMENU, "Alt"),
    (vk::RMENU, "Alt"),
    (vk::BROWSER_BACK, "Navigateur : précédent"),
    (vk::BROWSER_FORWARD, "Navigateur : suivant"),
    (vk::BROWSER_REFRESH, "Navigateur : actualiser"),
    (vk::BROWSER_STOP, "Navigateur : arrêter"),
    (vk::BROWSER_SEARCH, "Navigateur : rechercher"),
    (vk::BROWSER_FAVORITES, "Navigateur : favoris"),
    (vk::BROWSER_HOME, "Navigateur : accueil"),
    (vk::VOLUME_MUTE, "Muet"),
    (vk::VOLUME_DOWN, "Volume -"),
    (vk::VOLUME_UP, "Volume +"),
    (vk::MEDIA_NEXT_TRACK, "Piste suivante"),
    (vk::MEDIA_PREV_TRACK, "Piste précédente"),
    (vk::MEDIA_STOP, "Arrêt média"),
    (vk::MEDIA_PLAY_PAUSE, "Lecture/Pause"),
    (vk::LAUNCH_MAIL, "Courrier"),
    (vk::LAUNCH_MEDIA_SELECT, "Média"),
    (vk::LAUNCH_APP1, "Application 1"),
    (vk::LAUNCH_APP2, "Application 2"),
];

/// French name of a key: « F8 », « M », « Pavé num. 5 », « Arrêt défil. ».
pub fn key_name(key: Vk, layout_char: &dyn Fn(Vk) -> Option<char>) -> String {
    match key {
        vk::KEY_A..=vk::KEY_Z | vk::KEY_0..=vk::KEY_9 => char::from(key as u8).to_string(),
        vk::F1..=vk::F24 => format!("F{}", key - vk::F1 + 1),
        vk::NUMPAD0..=vk::NUMPAD9 => format!("Pavé num. {}", key - vk::NUMPAD0),
        _ if is_layout_key(key) => layout_char(key)
            .filter(|c| !c.is_control() && !c.is_whitespace())
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_else(|| generic_name(key)),
        _ => NAMED_KEYS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, name)| (*name).to_string())
            .unwrap_or_else(|| generic_name(key)),
    }
}

fn generic_name(key: Vk) -> String {
    format!("Touche {key:#04X}")
}

/// Keys that type text or move the text cursor: as a global hotkey without Ctrl, Alt or Win,
/// they would stop working everywhere while MacroHUD runs.
fn types_text(key: Vk) -> bool {
    matches!(
        key,
        vk::KEY_A..=vk::KEY_Z
            | vk::KEY_0..=vk::KEY_9
            | vk::NUMPAD0..=vk::DIVIDE
            | vk::SPACE
            | vk::RETURN
            | vk::TAB
            | vk::BACK
            | vk::DELETE
            | vk::CLEAR
            | vk::PRIOR
            | vk::NEXT
            | vk::END
            | vk::HOME
            | vk::LEFT
            | vk::UP
            | vk::RIGHT
            | vk::DOWN
    ) || is_layout_key(key)
}

/// Combinations that Windows or nearly every application already uses.
fn reserved_use(hotkey: &Hotkey) -> Option<&'static str> {
    let m = hotkey.modifiers;
    let ctrl_only = m == Modifiers::NONE.with(Modifier::Ctrl);
    let alt_only = m == Modifiers::NONE.with(Modifier::Alt);
    let alt_shift = m == Modifiers::NONE.with(Modifier::Alt).with(Modifier::Shift);
    match hotkey.key {
        vk::F4 if alt_only => Some("fermer les fenêtres"),
        vk::TAB if alt_only || alt_shift => Some("passer d'une fenêtre à l'autre"),
        vk::SPACE if alt_only => Some("ouvrir le menu des fenêtres"),
        vk::KEY_C if ctrl_only => Some("copier"),
        vk::KEY_X if ctrl_only => Some("couper"),
        vk::KEY_V if ctrl_only => Some("coller"),
        vk::KEY_Z if ctrl_only => Some("annuler"),
        vk::KEY_Y if ctrl_only => Some("rétablir"),
        vk::KEY_A if ctrl_only => Some("tout sélectionner"),
        vk::KEY_S if ctrl_only => Some("enregistrer"),
        _ => None,
    }
}

/// Why `hotkey` cannot be used as a hotkey, as a French sentence; `None` when it can.
/// `label` is its text (see [`Hotkey::label`]).
pub fn refusal(hotkey: &Hotkey, label: &str) -> Option<String> {
    let key = hotkey.key;
    if key == 0 || key > 0xFE || modifier_of(key).is_some() || (vk::LBUTTON..=vk::XBUTTON2).contains(&key) {
        return Some("Choisissez une touche, en plus de Ctrl, Alt, Maj ou Win.".into());
    }
    if key == vk::ESCAPE {
        return Some("Échap sert à annuler : choisissez une autre touche.".into());
    }
    if key == vk::PACKET || key == vk::PROCESSKEY {
        return Some("Cette touche ne peut pas servir de raccourci.".into());
    }
    if key == vk::F12 && hotkey.modifiers == Modifiers::NONE {
        return Some(
            "F12 seule est réservée par Windows : ajoutez Ctrl, Alt, Maj ou Win, ou choisissez une autre touche."
                .into(),
        );
    }
    if let Some(usage) = reserved_use(hotkey) {
        return Some(format!("{label} sert déjà à {usage} partout : choisissez-en un autre."));
    }
    if types_text(key) && !hotkey.modifiers.has_command() {
        return Some(format!(
            "{label} empêcherait d'écrire : ajoutez Ctrl, Alt ou Win, ou choisissez une touche de F1 à F24."
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_layout(_: Vk) -> Option<char> {
        None
    }

    fn azerty(key: Vk) -> Option<char> {
        match key {
            0xBA => Some('$'),
            0xDE => Some('²'),
            0xE2 => Some('<'),
            0xC0 => Some('ù'),
            _ => None,
        }
    }

    fn hk(m: Modifiers, key: Vk) -> Hotkey {
        Hotkey::new(m, key)
    }

    fn ctrl() -> Modifiers {
        Modifiers::NONE.with(Modifier::Ctrl)
    }

    #[test]
    fn labels_are_french_and_ordered() {
        assert_eq!(Hotkey::F8.label(&no_layout), "F8");
        let ctrl_alt = ctrl().with(Modifier::Alt);
        assert_eq!(hk(ctrl_alt, vk::KEY_M).label(&no_layout), "Ctrl+Alt+M");
        assert_eq!(hk(Modifiers::NONE, vk::SCROLL).label(&no_layout), "Arrêt défil.");
        let win_shift = Modifiers::NONE.with(Modifier::Shift).with(Modifier::Win);
        assert_eq!(hk(win_shift, vk::KEY_S).label(&no_layout), "Win+Maj+S");
        let all = ctrl_alt.with(Modifier::Shift).with(Modifier::Win);
        assert_eq!(hk(all, vk::F24).label(&no_layout), "Win+Ctrl+Alt+Maj+F24");
        assert_eq!(hk(ctrl(), vk::PAUSE).label(&no_layout), "Ctrl+Pause");
        assert_eq!(hk(ctrl(), vk::CANCEL).label(&no_layout), "Ctrl+Attn");
        assert_eq!(hk(Modifiers::NONE, vk::ESCAPE).label(&no_layout), "Échap");
    }

    #[test]
    fn key_names_cover_ranges_and_layout_keys() {
        assert_eq!(key_name(vk::F1, &no_layout), "F1");
        assert_eq!(key_name(vk::F24, &no_layout), "F24");
        assert_eq!(key_name(vk::KEY_0, &no_layout), "0");
        assert_eq!(key_name(vk::NUMPAD0 + 7, &no_layout), "Pavé num. 7");
        assert_eq!(key_name(vk::DIVIDE, &no_layout), "Pavé num. /");
        assert_eq!(key_name(vk::PRIOR, &no_layout), "Page préc.");
        assert_eq!(key_name(vk::SNAPSHOT, &no_layout), "Impr. écran");
        assert_eq!(key_name(0xBA, &azerty), "$");
        assert_eq!(key_name(0xC0, &azerty), "Ù");
        assert_eq!(key_name(0xBA, &no_layout), "Touche 0xBA");
        assert_eq!(key_name(0xFB, &no_layout), "Touche 0xFB");
        assert_eq!(key_name(0x0E, &no_layout), "Touche 0x0E");
    }

    #[test]
    fn every_named_key_has_a_distinct_code_and_a_name() {
        for (i, (code, name)) in NAMED_KEYS.iter().enumerate() {
            assert!(!name.is_empty());
            assert!(NAMED_KEYS[i + 1..].iter().all(|(other, _)| other != code), "{code:#x} twice");
        }
    }

    #[test]
    fn modifiers_are_recognised_on_both_sides() {
        for (key, m) in [
            (vk::LCONTROL, Modifier::Ctrl),
            (vk::RCONTROL, Modifier::Ctrl),
            (vk::CONTROL, Modifier::Ctrl),
            (vk::LMENU, Modifier::Alt),
            (vk::RMENU, Modifier::Alt),
            (vk::LSHIFT, Modifier::Shift),
            (vk::RSHIFT, Modifier::Shift),
            (vk::LWIN, Modifier::Win),
            (vk::RWIN, Modifier::Win),
        ] {
            assert_eq!(modifier_of(key), Some(m));
        }
        assert_eq!(modifier_of(vk::F8), None);
        assert_eq!(modifier_of(vk::CAPITAL), None);
    }

    fn refused(hotkey: Hotkey) -> bool {
        refusal(&hotkey, &hotkey.label(&no_layout)).is_some()
    }

    #[test]
    fn function_keys_and_free_keys_are_accepted_alone() {
        for key in [vk::F1, vk::F8, vk::F11, vk::F24, vk::SCROLL, vk::PAUSE, vk::INSERT, vk::CAPITAL] {
            assert!(!refused(hk(Modifiers::NONE, key)), "{key:#x}");
        }
        assert!(!refused(hk(Modifiers::NONE.with(Modifier::Shift), vk::F8)));
    }

    #[test]
    fn typing_keys_need_ctrl_alt_or_win() {
        let shift = Modifiers::NONE.with(Modifier::Shift);
        for key in [vk::KEY_M, vk::KEY_0, vk::SPACE, vk::RETURN, vk::LEFT, vk::NUMPAD0, 0xBA] {
            assert!(refused(hk(Modifiers::NONE, key)), "{key:#x} alone");
            assert!(refused(hk(shift, key)), "Maj+{key:#x}");
        }
        assert!(!refused(hk(ctrl().with(Modifier::Alt), vk::KEY_M)));
        assert!(!refused(hk(Modifiers::NONE.with(Modifier::Win), vk::KEY_M)));
        let text = refusal(&hk(Modifiers::NONE, vk::KEY_M), "M").unwrap();
        assert!(text.starts_with("M empêcherait d'écrire"), "{text}");
    }

    #[test]
    fn windows_reserved_combinations_are_refused() {
        let alt = Modifiers::NONE.with(Modifier::Alt);
        assert!(refused(hk(alt, vk::F4)));
        assert!(refused(hk(alt, vk::TAB)));
        assert!(refused(hk(alt.with(Modifier::Shift), vk::TAB)));
        assert!(refused(hk(alt, vk::SPACE)));
        for key in [vk::KEY_C, vk::KEY_X, vk::KEY_V, vk::KEY_Z, vk::KEY_Y, vk::KEY_A, vk::KEY_S] {
            assert!(refused(hk(ctrl(), key)), "Ctrl+{key:#x}");
            assert!(!refused(hk(ctrl().with(Modifier::Alt), key)), "Ctrl+Alt+{key:#x}");
        }
        assert!(refused(hk(Modifiers::NONE, vk::F12)));
        assert!(!refused(hk(ctrl(), vk::F12)));
        assert!(refused(hk(Modifiers::NONE, vk::ESCAPE)));
        assert!(refused(hk(ctrl(), vk::ESCAPE)));
        assert!(refused(hk(ctrl(), vk::LCONTROL)));
        assert!(refused(hk(Modifiers::NONE, 0)));
        assert!(refused(hk(Modifiers::NONE, vk::LBUTTON)));
        let text = refusal(&hk(alt, vk::F4), "Alt+F4").unwrap();
        assert_eq!(text, "Alt+F4 sert déjà à fermer les fenêtres partout : choisissez-en un autre.");
    }
}
