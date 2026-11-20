//! Settings, in `%APPDATA%\MacroHUD\reglages.json`: each macro's hotkey and options, the HUD's
//! position and whether it is collapsed.
//!
//! A missing file gives the default values. A damaged one too, without crashing: it is kept aside
//! under another name, and only the damaged values are replaced when the file is merely partly wrong.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::contract::{MacroId, MacroOptions, MouseButton};
use crate::hotkey::{self, Hotkey, Modifiers, Vk};
use crate::macros::MacroDef;

/// Name of the settings file, in the settings folder.
pub const FILE_NAME: &str = "reglages.json";
/// Version of the file layout, written in the file.
const FORMAT: u64 = 1;

/// All settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Top-left corner of the HUD window, in screen pixels; `None`: the default place.
    pub position: Option<(i32, i32)>,
    pub collapsed: bool,
    /// One entry per macro of the catalog, in catalog order.
    pub macros: Vec<MacroSettings>,
    /// Entries of macros this version does not know (written by a newer one): kept untouched.
    pub unknown_macros: Map<String, Value>,
}

/// The settings of one macro.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacroSettings {
    pub id: MacroId,
    pub hotkey: Hotkey,
    pub options: MacroOptions,
}

impl Settings {
    pub fn defaults(catalog: &[MacroDef]) -> Self {
        Settings {
            position: None,
            collapsed: false,
            macros: catalog
                .iter()
                .map(|def| MacroSettings { id: def.id, hotkey: def.default_hotkey, options: def.default_options })
                .collect(),
            unknown_macros: Map::new(),
        }
    }

    pub fn macro_settings(&self, id: MacroId) -> Option<&MacroSettings> {
        self.macros.iter().find(|m| m.id == id)
    }
}

/// The result of reading the settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    pub settings: Settings,
    /// What went wrong, for the HUD (French); `None` when the file was fine or absent.
    pub warning: Option<String>,
    /// The file was damaged: write the corrected settings at once, so the warning does not come
    /// back at every start. (Not when it could not be read at all: it might be fine, only locked.)
    pub rewrite: bool,
}

/// Reads the settings from `dir`. Never fails: defaults replace whatever cannot be read.
/// `stamp` dates the copy of a damaged file (« 2026-11-20_14-35-12 »).
pub fn load(dir: &Path, catalog: &[MacroDef], stamp: &str) -> Loaded {
    let path = dir.join(FILE_NAME);
    let defaults = || Settings::defaults(catalog);
    match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Loaded { settings: defaults(), warning: None, rewrite: false }
        }
        Err(e) => Loaded {
            settings: defaults(),
            warning: Some(format!("Réglages illisibles ({e}) : valeurs par défaut pour cette fois.")),
            rewrite: false,
        },
        Ok(bytes) => match parse(&bytes, catalog) {
            Ok((settings, true)) => Loaded { settings, warning: None, rewrite: false },
            Ok((settings, false)) => Loaded {
                settings,
                warning: Some("Certains réglages étaient abîmés : ils reprennent leur valeur par défaut.".into()),
                rewrite: true,
            },
            Err(()) => {
                let warning = match keep_aside(&path, stamp) {
                    Ok(name) => {
                        format!("Réglages abîmés : valeurs par défaut. L'ancien fichier est gardé sous le nom {name}.")
                    }
                    Err(_) => "Réglages abîmés : valeurs par défaut.".to_string(),
                };
                Loaded { settings: defaults(), warning: Some(warning), rewrite: true }
            }
        },
    }
}

/// Renames a damaged settings file so that the next save does not overwrite it.
fn keep_aside(path: &Path, stamp: &str) -> std::io::Result<String> {
    let name = format!("reglages.abime-{stamp}.json");
    std::fs::rename(path, path.with_file_name(&name))?;
    Ok(name)
}

/// Reads settings from the file's bytes. `Err` when it is not a JSON object at all; otherwise the
/// settings, and whether every value could be read (`false`: some took their default value).
pub fn parse(bytes: &[u8], catalog: &[MacroDef]) -> Result<(Settings, bool), ()> {
    let root: Value = serde_json::from_slice(bytes).map_err(|_| ())?;
    let root = root.as_object().ok_or(())?;
    let mut settings = Settings::defaults(catalog);
    let mut intact = true;

    if let Some(hud) = root.get("hud") {
        match hud.as_object() {
            None => intact = false,
            Some(hud) => {
                match hud.get("position") {
                    None | Some(Value::Null) => {}
                    Some(p) => match parse_position(p) {
                        Some(pos) => settings.position = Some(pos),
                        None => intact = false,
                    },
                }
                match hud.get("collapsed") {
                    None => {}
                    Some(Value::Bool(b)) => settings.collapsed = *b,
                    Some(_) => intact = false,
                }
            }
        }
    }

    let empty = Map::new();
    let macros = match root.get("macros") {
        None => &empty,
        Some(Value::Object(m)) => m,
        Some(_) => {
            intact = false;
            &empty
        }
    };
    for (def, entry) in catalog.iter().zip(settings.macros.iter_mut()) {
        let Some(value) = macros.get(def.id.0) else { continue };
        let Some(obj) = value.as_object() else {
            intact = false;
            continue;
        };
        if let Some(h) = obj.get("hotkey") {
            match parse_hotkey(h) {
                Some(hotkey) => entry.hotkey = hotkey,
                None => intact = false,
            }
        }
        match (def.default_options, obj.get("button")) {
            (_, None) => {}
            (MacroOptions::HoldClick { .. }, Some(b)) => match b.as_str().and_then(button_from_key) {
                Some(button) => entry.options = MacroOptions::HoldClick { button },
                None => intact = false,
            },
        }
    }
    // Two macros cannot share a hotkey: the later one gets its default back.
    for (i, def) in catalog.iter().enumerate().skip(1) {
        let (earlier, later) = settings.macros.split_at_mut(i);
        if earlier.iter().any(|m| m.hotkey == later[0].hotkey) {
            later[0].hotkey = def.default_hotkey;
            intact = false;
        }
    }
    for (key, value) in macros {
        if !catalog.iter().any(|def| def.id.0 == key) {
            settings.unknown_macros.insert(key.clone(), value.clone());
        }
    }
    Ok((settings, intact))
}

fn parse_position(value: &Value) -> Option<(i32, i32)> {
    let x = i32::try_from(value.get("x")?.as_i64()?).ok()?;
    let y = i32::try_from(value.get("y")?.as_i64()?).ok()?;
    Some((x, y))
}

fn parse_hotkey(value: &Value) -> Option<Hotkey> {
    let obj = value.as_object()?;
    let key = Vk::try_from(obj.get("key")?.as_u64()?).ok()?;
    let flag = |name: &str| match obj.get(name) {
        None => Some(false),
        Some(v) => v.as_bool(),
    };
    let modifiers = Modifiers { ctrl: flag("ctrl")?, alt: flag("alt")?, shift: flag("shift")?, win: flag("win")? };
    let hotkey = Hotkey::new(modifiers, key);
    // A hotkey the HUD would refuse (edited by hand, or rules changed since) is not used.
    hotkey::refusal(&hotkey, "").is_none().then_some(hotkey)
}

fn button_key(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "left",
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
    }
}

fn button_from_key(key: &str) -> Option<MouseButton> {
    MouseButton::ALL.into_iter().find(|b| button_key(*b) == key)
}

/// The file's text. `label` names a hotkey (written next to it, for whoever opens the file).
pub fn to_json(settings: &Settings, label: &dyn Fn(&Hotkey) -> String) -> String {
    let mut macros = settings.unknown_macros.clone();
    for m in &settings.macros {
        let h = m.hotkey;
        let mut entry = Map::new();
        entry.insert(
            "hotkey".into(),
            json!({
                "key": h.key,
                "ctrl": h.modifiers.ctrl,
                "alt": h.modifiers.alt,
                "shift": h.modifiers.shift,
                "win": h.modifiers.win,
                "label": label(&h),
            }),
        );
        match m.options {
            MacroOptions::HoldClick { button } => {
                entry.insert("button".into(), json!(button_key(button)));
            }
        }
        macros.insert(m.id.0.to_string(), Value::Object(entry));
    }
    let position = match settings.position {
        Some((x, y)) => json!({ "x": x, "y": y }),
        None => Value::Null,
    };
    let root = json!({
        "format": FORMAT,
        "hud": { "position": position, "collapsed": settings.collapsed },
        "macros": macros,
    });
    let mut text = serde_json::to_string_pretty(&root).unwrap_or_default();
    text.push('\n');
    text
}

/// Writes the settings into `dir` (created if needed). The file is replaced in one step, so that
/// a crash in the middle never leaves it half written.
pub fn save(dir: &Path, settings: &Settings, label: &dyn Fn(&Hotkey) -> String) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE_NAME);
    let temporary = dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(&temporary, to_json(settings, label))?;
    std::fs::rename(&temporary, &path)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::{Modifier, vk};
    use crate::macros::{CATALOG, HOLD_CLICK};

    fn label(h: &Hotkey) -> String {
        h.label(&|_| None)
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("macrohud-tests-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn two_macros() -> Vec<MacroDef> {
        let mut second = HOLD_CLICK;
        second.id = MacroId("deuxieme");
        second.default_hotkey = Hotkey::new(Modifiers::NONE, vk::F9 + 1);
        vec![HOLD_CLICK, second]
    }

    #[test]
    fn defaults_follow_the_catalog() {
        let s = Settings::defaults(CATALOG);
        assert_eq!(s.position, None);
        assert!(!s.collapsed);
        let m = s.macro_settings(MacroId::HOLD_CLICK).unwrap();
        assert_eq!(m.hotkey, Hotkey::F8);
        assert_eq!(m.options, MacroOptions::HoldClick { button: MouseButton::Left });
    }

    #[test]
    fn a_missing_file_gives_defaults_silently() {
        let dir = temp_dir("absent");
        let loaded = load(&dir, CATALOG, "x");
        assert_eq!(loaded, Loaded { settings: Settings::defaults(CATALOG), warning: None, rewrite: false });
    }

    #[test]
    fn saved_settings_read_back_identical() {
        let dir = temp_dir("aller-retour");
        let mut s = Settings::defaults(CATALOG);
        s.position = Some((-1200, 35));
        s.collapsed = true;
        s.macros[0].hotkey = Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Alt), vk::KEY_M);
        s.macros[0].options = MacroOptions::HoldClick { button: MouseButton::Middle };
        let path = save(&dir, &s, &label).unwrap();
        assert!(path.ends_with(FILE_NAME));
        assert!(!dir.join("reglages.json.tmp").exists());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"label\": \"Ctrl+Alt+M\""), "{text}");
        let loaded = load(&dir, CATALOG, "x");
        assert_eq!(loaded, Loaded { settings: s, warning: None, rewrite: false });
    }

    #[test]
    fn a_damaged_file_gives_defaults_and_is_kept_aside() {
        let dir = temp_dir("abime");
        std::fs::write(dir.join(FILE_NAME), b"{ \"hud\": { \"collapsed\": tr").unwrap();
        let loaded = load(&dir, CATALOG, "2026-11-20_14-35-12");
        assert_eq!(loaded.settings, Settings::defaults(CATALOG));
        let warning = loaded.warning.unwrap();
        assert!(warning.contains("reglages.abime-2026-11-20_14-35-12.json"), "{warning}");
        assert!(dir.join("reglages.abime-2026-11-20_14-35-12.json").exists());
        assert!(!dir.join(FILE_NAME).exists());
        assert!(loaded.rewrite);
    }

    #[test]
    fn non_object_files_are_damaged() {
        for text in ["", "[]", "42", "null", "\u{0}\u{1}"] {
            assert!(parse(text.as_bytes(), CATALOG).is_err(), "{text:?}");
        }
    }

    #[test]
    fn damaged_values_take_their_default_and_the_rest_is_kept() {
        let text = r#"{
            "hud": { "position": { "x": "loin", "y": 3 }, "collapsed": true },
            "macros": { "maintenir-clic": { "hotkey": { "key": 999 }, "button": "right" } }
        }"#;
        let (s, intact) = parse(text.as_bytes(), CATALOG).unwrap();
        assert!(!intact);
        assert_eq!(s.position, None);
        assert!(s.collapsed);
        assert_eq!(s.macros[0].hotkey, Hotkey::F8);
        assert_eq!(s.macros[0].options, MacroOptions::HoldClick { button: MouseButton::Right });

        let text = r#"{ "hud": 5, "macros": { "maintenir-clic": { "button": "pouce" } } }"#;
        let (s, intact) = parse(text.as_bytes(), CATALOG).unwrap();
        assert!(!intact);
        assert_eq!(s, Settings::defaults(CATALOG));
    }

    #[test]
    fn hotkeys_the_hud_would_refuse_are_not_loaded() {
        // "M" alone, written by hand.
        let text = r#"{ "macros": { "maintenir-clic": { "hotkey": { "key": 77 } } } }"#;
        let (s, intact) = parse(text.as_bytes(), CATALOG).unwrap();
        assert!(!intact);
        assert_eq!(s.macros[0].hotkey, Hotkey::F8);
        // Missing modifier flags mean "not held".
        let text = r#"{ "macros": { "maintenir-clic": { "hotkey": { "key": 120, "ctrl": true } } } }"#;
        let (s, intact) = parse(text.as_bytes(), CATALOG).unwrap();
        assert!(intact);
        assert_eq!(s.macros[0].hotkey, Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl), vk::F9));
    }

    #[test]
    fn unknown_fields_and_macros_are_tolerated_and_kept() {
        let text = r#"{
            "format": 7, "theme": "sombre",
            "hud": { "collapsed": false, "opacity": 0.5 },
            "macros": { "future-macro": { "speed": 3 } }
        }"#;
        let (s, intact) = parse(text.as_bytes(), CATALOG).unwrap();
        assert!(intact);
        assert_eq!(s.unknown_macros.get("future-macro"), Some(&json!({ "speed": 3 })));
        let written = to_json(&s, &label);
        assert!(written.contains("future-macro"), "{written}");
    }

    #[test]
    fn two_macros_never_share_a_hotkey() {
        let catalog = two_macros();
        let text = r#"{ "macros": {
            "maintenir-clic": { "hotkey": { "key": 120 } },
            "deuxieme": { "hotkey": { "key": 120 } }
        } }"#;
        let (s, intact) = parse(text.as_bytes(), &catalog).unwrap();
        assert!(!intact);
        assert_eq!(s.macros[0].hotkey.key, vk::F9);
        assert_eq!(s.macros[1].hotkey, catalog[1].default_hotkey);
        assert_eq!(s.macros[1].hotkey.key, vk::F9 + 1);
    }
}
