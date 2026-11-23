//! External macros (D-32, D-36; the rule: `docs/MACROS-EXTERNES.md`): scripts of the PC that
//! MacroHUD starts, stops and shows like its own macros.
//!
//! This module reads their declarations (`%APPDATA%\MacroHUD\macros-externes\<id>.json`) and the
//! lines their scripts write, and keeps what a running script said, for its line in the HUD. The
//! engine runs them (`engine.rs`); `script.rs` starts their processes.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::contract::{Alert, Badge, BadgeColor, IntSetting, MouseButton};
use crate::guard::ButtonSet;
use crate::hotkey::{self, Hotkey};

/// Turns a text read once at start (from a declaration) into one that lives as long as MacroHUD, as
/// the names and ids of its own macros do.
pub fn lasting(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}

/// Folder of the declarations, in the settings folder.
pub const FOLDER: &str = "macros-externes";
/// A line of a script longer than this, in characters, is cut.
pub const LINE_LIMIT: usize = 200;

/// MacroHUD to the script: pause (the mouse is on the HUD, D-22).
pub const PAUSE: &str = "pause";
/// MacroHUD to the script: go on (the mouse left the HUD).
pub const RESUME: &str = "reprise";
/// MacroHUD to the script: finish your gesture, release what you hold, quit.
pub const STOP: &str = "arret";

/// The line that gives a setting's value to the script.
pub fn setting_line(key: &str, value: i64) -> String {
    format!("reglage {key} {value}")
}

// --- Declarations --------------------------------------------------------------------------------

/// An integer setting, as declared.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingDecl {
    pub key: String,
    pub name: String,
    pub unit: String,
    pub min: i64,
    pub max: i64,
    pub step: i64,
    pub default: i64,
}

impl SettingDecl {
    /// The setting at `value`, brought onto its steps, as the HUD shows it.
    pub fn at(&self, value: i64) -> IntSetting {
        let mut setting = IntSetting {
            key: self.key.clone(),
            name: self.name.clone(),
            unit: self.unit.clone(),
            min: self.min,
            max: self.max,
            step: self.step,
            value: self.default,
        };
        setting.value = setting.snap(value);
        setting
    }
}

/// One external macro, as declared.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    /// The name of its file, without `.json`.
    pub id: String,
    pub name: String,
    pub description: String,
    pub command: String,
    pub arguments: Vec<String>,
    /// Working folder of the script.
    pub folder: Option<PathBuf>,
    /// The hotkey it proposes (`raccourci`); the user may choose another one in the HUD.
    pub hotkey: Option<Hotkey>,
    /// The mouse buttons its script may press: released by MacroHUD if the script must be stopped
    /// by force or ends badly.
    pub buttons: ButtonSet,
    pub settings: Vec<SettingDecl>,
    /// `etat_arretee`: the text after « ■ Arrêtée · », where `{cle}` becomes the setting's value.
    pub stopped_text: Option<String>,
    /// The help line of its settings.
    pub info: Option<String>,
}

impl Declaration {
    /// The text after « ■ Arrêtée · » with these values of the settings (in declaration order).
    pub fn stopped_line(&self, values: &[i64]) -> Option<String> {
        let mut text = self.stopped_text.clone()?;
        for (setting, value) in self.settings.iter().zip(values) {
            text = text.replace(&format!("{{{}}}", setting.key), &value.to_string());
        }
        Some(text)
    }

    /// The settings at their default values.
    pub fn defaults(&self) -> Vec<i64> {
        self.settings.iter().map(|s| s.default).collect()
    }
}

fn button_from_word(word: &str) -> Option<MouseButton> {
    match word {
        "gauche" => Some(MouseButton::Left),
        "droit" => Some(MouseButton::Right),
        "milieu" => Some(MouseButton::Middle),
        _ => None,
    }
}

/// A text field: absent (`None`), or a string; anything else is an error.
fn text_field(object: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("« {key} » doit être un texte")),
    }
}

fn integer_field(object: &Map<String, Value>, key: &str) -> Result<Option<i64>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_i64().map(Some).ok_or_else(|| format!("« {key} » doit être un nombre entier")),
    }
}

fn parse_setting(value: &Value, index: usize) -> Result<SettingDecl, String> {
    let object = value.as_object().ok_or_else(|| format!("le réglage n° {} n'est pas un objet", index + 1))?;
    let key = text_field(object, "cle")?.unwrap_or_default();
    if key.is_empty() || key.chars().any(char::is_whitespace) {
        return Err(format!("le réglage n° {} n'a pas de « cle » sans espace", index + 1));
    }
    let name = text_field(object, "nom")?.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| key.clone());
    let unit = text_field(object, "unite")?.unwrap_or_default();
    let min = integer_field(object, "min")?.ok_or_else(|| format!("le réglage « {key} » n'a pas de « min »"))?;
    let max = integer_field(object, "max")?.ok_or_else(|| format!("le réglage « {key} » n'a pas de « max »"))?;
    let step = integer_field(object, "pas")?.unwrap_or(1);
    if min > max {
        return Err(format!("le réglage « {key} » a un « min » plus grand que son « max »"));
    }
    if step < 1 {
        return Err(format!("le réglage « {key} » a un « pas » inférieur à 1"));
    }
    let mut setting = SettingDecl { key, name, unit, min, max, step, default: min };
    let default = integer_field(object, "defaut")?.unwrap_or(min);
    setting.default = setting.at(default).value;
    Ok(setting)
}

/// Extensions of the arguments that name a script (or a program) to run.
const SCRIPT_EXTENSIONS: &[&str] = &["ps1", "psm1", "py", "pyw", "cmd", "bat", "vbs", "js", "exe", "ahk"];

/// Why the declaration cannot start on this PC, in the words of the HUD's box, if a file it needs is
/// missing: its working folder, or a script among its arguments (read from that folder when
/// relative). `None` when nothing is missing (the command itself is looked for at the start).
pub fn missing(declaration: &Declaration) -> Option<String> {
    if let Some(folder) = &declaration.folder
        && !folder.is_dir()
    {
        return Some("Son dossier de travail est introuvable sur ce PC.".into());
    }
    let script = declaration.arguments.iter().map(PathBuf::from).find(|path| {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| SCRIPT_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
    })?;
    let path = match &declaration.folder {
        Some(folder) if script.is_relative() => folder.join(&script),
        _ => script,
    };
    (!path.is_file()).then(|| "Son script est introuvable sur ce PC.".into())
}

/// Reads the declaration of the external macro `id` (its file name without `.json`). `Err`: why it
/// cannot be used. `Ok` also gives what was ignored in it, for the run log.
pub fn parse(id: &str, bytes: &[u8]) -> Result<(Declaration, Vec<String>), String> {
    let text = String::from_utf8_lossy(bytes);
    let root: Value =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("JSON illisible ({e})"))?;
    let object = root.as_object().ok_or("le fichier n'est pas un objet JSON")?;
    let mut ignored = Vec::new();

    let name = text_field(object, "nom")?.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let name = name.ok_or("« nom » manque")?;
    let command = text_field(object, "commande")?.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    let command = command.ok_or("« commande » manque")?;
    let arguments = match object.get("arguments") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()
            .ok_or("« arguments » doit être une liste de textes")?,
        Some(_) => return Err("« arguments » doit être une liste de textes".into()),
    };
    let folder = text_field(object, "dossier")?.filter(|f| !f.trim().is_empty()).map(PathBuf::from);
    let mut buttons = ButtonSet::EMPTY;
    match object.get("boutons") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for item in items {
                let button = item.as_str().and_then(button_from_word).ok_or_else(|| {
                    format!("« boutons » ne connaît que « gauche », « droit » et « milieu » (lu : {item})")
                })?;
                buttons = buttons.with(button);
            }
        }
        Some(_) => return Err("« boutons » doit être une liste".into()),
    }
    let mut settings: Vec<SettingDecl> = Vec::new();
    match object.get("reglages") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for (index, item) in items.iter().enumerate() {
                let setting = parse_setting(item, index)?;
                if settings.iter().any(|s| s.key == setting.key) {
                    return Err(format!("deux réglages ont la clé « {} »", setting.key));
                }
                settings.push(setting);
            }
        }
        Some(_) => return Err("« reglages » doit être une liste".into()),
    }
    // What only shows: a wrong one is left out, the macro still works.
    let mut optional_text = |key: &str| match text_field(object, key) {
        Ok(text) => text.filter(|t| !t.trim().is_empty()),
        Err(e) => {
            ignored.push(format!("{e} : ignoré"));
            None
        }
    };
    let description = optional_text("description").unwrap_or_default();
    let stopped_text = optional_text("etat_arretee");
    let info = optional_text("info");
    let hotkey_text = optional_text("raccourci");
    let hotkey = hotkey_text.and_then(|text| match Hotkey::parse(&text) {
        Some(hotkey) => match hotkey::refusal(&hotkey, text.trim()) {
            None => Some(hotkey),
            Some(why) => {
                ignored.push(format!("raccourci « {text} » ignoré : {why}"));
                None
            }
        },
        None => {
            ignored.push(format!("raccourci « {text} » ignoré : touche inconnue"));
            None
        }
    });
    let declaration = Declaration {
        id: id.to_string(),
        name,
        description,
        command,
        arguments,
        folder,
        hotkey,
        buttons,
        settings,
        stopped_text,
        info,
    };
    Ok((declaration, ignored))
}

/// Reads every declaration of `dir` (`*.json`), in the alphabetical order of their ids. A file that
/// cannot be read, or whose id is one of MacroHUD's own macros (`taken`), is left out. Returns the
/// declarations and the lines for the run log.
pub fn read_all(dir: &Path, taken: &[&str]) -> (Vec<Declaration>, Vec<String>) {
    let mut log = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return (Vec::new(), log) };
    let mut files: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")))
        .filter_map(|path| Some((path.file_stem()?.to_string_lossy().trim().to_string(), path)))
        .collect();
    files.sort();
    let mut declarations: Vec<Declaration> = Vec::new();
    for (id, path) in files {
        let result = if id.is_empty() {
            Err("nom de fichier vide".to_string())
        } else if taken.contains(&id.as_str()) {
            Err("ce nom est celui d'une macro de MacroHUD".to_string())
        } else if declarations.iter().any(|d| d.id.eq_ignore_ascii_case(&id)) {
            Err("une autre déclaration a ce nom".to_string())
        } else {
            std::fs::read(&path).map_err(|e| format!("illisible ({e})")).and_then(|bytes| parse(&id, &bytes))
        };
        match result {
            Ok((declaration, ignored)) => {
                log.push(format!("Macro externe « {id} » : déclaration lue (« {} »).", declaration.name));
                log.extend(ignored.into_iter().map(|what| format!("Macro externe « {id} » : {what}.")));
                declarations.push(declaration);
            }
            Err(why) => log.push(format!("Macro externe « {id} » laissée de côté : {why}.")),
        }
    }
    (declarations, log)
}

// --- Lines of the script -------------------------------------------------------------------------

/// What a script says, one line at a time (`docs/MACROS-EXTERNES.md`, « Échanges »).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FromScript {
    /// `etat <texte>`: « ● Active · texte ».
    State(String),
    /// `pastille <couleur> <texte>`, or `pastille` alone (`None`: no more badge).
    Badge(Option<Badge>),
    /// `attente <texte>`: « ❚❚ En pause · texte ».
    Waiting(String),
    /// `reprise`: back to « ● Active · … ».
    Resume,
    /// `alerte <titre> | <texte>`.
    Alert(Alert),
    /// `fin <texte>`: it stops by itself, « ✓ Terminée · texte ».
    Finished(String),
    /// `journal <texte>`: a line of the run log.
    Log(String),
    /// Anything else: to the run log as well.
    Other(String),
}

/// A line of the script's output as MacroHUD reads it: UTF-8 (invalid bytes replaced), without a
/// byte order mark nor the end of line, cut at [`LINE_LIMIT`] characters.
pub fn read_line(raw: &[u8]) -> String {
    let text = String::from_utf8_lossy(raw);
    let text = text.trim_start_matches('\u{feff}').trim_end_matches(['\r', '\n']);
    text.chars().take(LINE_LIMIT).collect()
}

fn badge_color(word: &str) -> Option<BadgeColor> {
    match word {
        "ambre" => Some(BadgeColor::Amber),
        "verte" => Some(BadgeColor::Green),
        "bleue" => Some(BadgeColor::Blue),
        "grise" => Some(BadgeColor::Grey),
        _ => None,
    }
}

/// What a line of the script says.
pub fn parse_line(line: &str) -> FromScript {
    let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
    let rest = rest.trim();
    match word {
        "etat" => FromScript::State(rest.to_string()),
        "pastille" if rest.is_empty() => FromScript::Badge(None),
        "pastille" => {
            let (color, text) = rest.split_once(' ').unwrap_or((rest, ""));
            match (badge_color(color), text.trim()) {
                (Some(color), text) if !text.is_empty() => {
                    FromScript::Badge(Some(Badge { color, text: text.to_string() }))
                }
                _ => FromScript::Other(line.to_string()),
            }
        }
        "attente" => FromScript::Waiting(rest.to_string()),
        "reprise" if rest.is_empty() => FromScript::Resume,
        "alerte" => {
            let (title, text) = rest.split_once('|').unwrap_or((rest, ""));
            FromScript::Alert(Alert { title: title.trim().to_string(), text: text.trim().to_string() })
        }
        "fin" => FromScript::Finished(rest.to_string()),
        "journal" => FromScript::Log(rest.to_string()),
        _ => FromScript::Other(line.to_string()),
    }
}

/// What a running script said so far, for its line in the HUD.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Said {
    /// The last `etat` (empty: none).
    pub state: Option<String>,
    /// `attente` until `reprise`.
    pub waiting: Option<String>,
    /// The last `pastille`.
    pub badge: Option<Badge>,
    /// The last `alerte`.
    pub alert: Option<Alert>,
    /// `fin`: it stops by itself.
    pub finished: Option<String>,
}

impl Said {
    /// Takes in one line of the script of macro `name`; returns what goes into the run log.
    pub fn take(&mut self, message: FromScript, name: &str) -> Option<String> {
        match message {
            FromScript::State(text) => self.state = Some(text).filter(|t| !t.is_empty()),
            FromScript::Badge(badge) => self.badge = badge,
            FromScript::Waiting(text) => self.waiting = Some(text),
            FromScript::Resume => self.waiting = None,
            FromScript::Alert(alert) => {
                let line = format!("« {name} » : alerte « {} » {}", alert.title, alert.text);
                self.alert = Some(alert);
                return Some(line);
            }
            FromScript::Finished(text) => {
                let line = format!("« {name} » : fin « {text} »");
                self.finished = Some(text);
                return Some(line);
            }
            FromScript::Log(text) => return Some(format!("« {name} » : {text}")),
            FromScript::Other(text) => return Some(format!("« {name} » (sortie) : {text}")),
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::{Modifiers, vk};

    const EXAMPLE: &str = r#"{
  "nom": "Exemple",
  "description": "Clique toutes les deux secondes",
  "commande": "powershell.exe",
  "arguments": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "C:\\Users\\<utilisateur>\\exemple.ps1"],
  "dossier": "C:\\Users\\<utilisateur>",
  "raccourci": "F10",
  "boutons": ["droit"],
  "reglages": [
    { "cle": "limite", "nom": "Limite de clics", "unite": "clics", "min": 10, "max": 500, "pas": 10, "defaut": 100 }
  ],
  "etat_arretee": "limite {limite} clics",
  "info": "Garde la fenêtre visée au premier plan. Un vrai clic n'arrête pas cette macro : F10 ou ▶ l'arrêtent."
}"#;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("macrohud-tests-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_declaration_of_the_rule_reads_whole() {
        let (d, ignored) = parse("exemple", EXAMPLE.as_bytes()).unwrap();
        assert!(ignored.is_empty(), "{ignored:?}");
        assert_eq!((d.id.as_str(), d.name.as_str(), d.command.as_str()), ("exemple", "Exemple", "powershell.exe"));
        assert_eq!(d.arguments.len(), 5);
        assert_eq!(d.folder, Some(PathBuf::from("C:\\Users\\<utilisateur>")));
        assert_eq!(d.hotkey, Some(Hotkey::new(Modifiers::NONE, vk::F10)));
        assert_eq!(d.buttons, ButtonSet::only(MouseButton::Right));
        assert_eq!(
            d.settings,
            vec![SettingDecl {
                key: "limite".into(),
                name: "Limite de clics".into(),
                unit: "clics".into(),
                min: 10,
                max: 500,
                step: 10,
                default: 100
            }]
        );
        assert_eq!(d.stopped_line(&[120]), Some("limite 120 clics".into()));
        assert_eq!(d.defaults(), vec![100]);
        assert!(d.info.as_deref().unwrap().starts_with("Garde la fenêtre"));
    }

    #[test]
    fn only_name_and_command_are_needed() {
        let (d, ignored) = parse("mini", br#"{ "nom": "Mini", "commande": "outil.exe" }"#).unwrap();
        assert!(ignored.is_empty());
        assert_eq!((d.arguments.len(), d.folder.clone(), d.hotkey, d.buttons), (0, None, None, ButtonSet::EMPTY));
        assert!(d.settings.is_empty() && d.stopped_text.is_none() && d.info.is_none() && d.description.is_empty());
        assert_eq!(d.stopped_line(&[]), None);
        // A byte order mark (Notepad) is no problem.
        let with_bom = [b"\xEF\xBB\xBF".as_slice(), br#"{ "nom": "Mini", "commande": "outil.exe" }"#].concat();
        assert!(parse("mini", &with_bom).is_ok());
    }

    #[test]
    fn a_declaration_that_cannot_run_is_refused_with_its_reason() {
        for (text, why) in [
            ("{", "JSON illisible"),
            ("[]", "objet JSON"),
            (r#"{ "commande": "outil.exe" }"#, "« nom » manque"),
            (r#"{ "nom": "  ", "commande": "outil.exe" }"#, "« nom » manque"),
            (r#"{ "nom": "X" }"#, "« commande » manque"),
            (r#"{ "nom": "X", "commande": 3 }"#, "« commande » doit être un texte"),
            (r#"{ "nom": "X", "commande": "a", "arguments": "-x" }"#, "« arguments »"),
            (r#"{ "nom": "X", "commande": "a", "arguments": ["-x", 2] }"#, "« arguments »"),
            (r#"{ "nom": "X", "commande": "a", "boutons": ["pouce"] }"#, "« boutons »"),
            (r#"{ "nom": "X", "commande": "a", "reglages": [{ "nom": "n", "min": 1, "max": 2 }] }"#, "« cle »"),
            (r#"{ "nom": "X", "commande": "a", "reglages": [{ "cle": "a b", "min": 1, "max": 2 }] }"#, "« cle »"),
            (r#"{ "nom": "X", "commande": "a", "reglages": [{ "cle": "a", "max": 2 }] }"#, "« min »"),
            (r#"{ "nom": "X", "commande": "a", "reglages": [{ "cle": "a", "min": 3, "max": 2 }] }"#, "plus grand"),
            (
                r#"{ "nom": "X", "commande": "a", "reglages": [{ "cle": "a", "min": 1, "max": 2, "pas": 0 }] }"#,
                "« pas »",
            ),
            (r#"{ "nom": "X", "commande": "a", "reglages": [{ "cle": "a", "min": 1.5, "max": 2 }] }"#, "entier"),
            (
                r#"{ "nom": "X", "commande": "a", "reglages": [{ "cle": "a", "min": 1, "max": 2 }, { "cle": "a", "min": 1, "max": 2 }] }"#,
                "deux réglages",
            ),
        ] {
            let error = parse("x", text.as_bytes()).unwrap_err();
            assert!(error.contains(why), "{text} : {error}");
        }
    }

    #[test]
    fn what_only_shows_is_left_out_when_wrong() {
        let text = r#"{ "nom": "X", "commande": "a", "description": 2, "info": [], "raccourci": "M",
                        "etat_arretee": 4 }"#;
        let (d, ignored) = parse("x", text.as_bytes()).unwrap();
        assert_eq!(d.hotkey, None, "« M » alone would stop typing");
        assert!(d.description.is_empty() && d.info.is_none() && d.stopped_text.is_none());
        assert_eq!(ignored.len(), 4, "{ignored:?}");
        assert!(ignored.iter().any(|i| i.starts_with("raccourci « M » ignoré")), "{ignored:?}");
        let (d, ignored) = parse("x", br#"{ "nom": "X", "commande": "a", "raccourci": "Truc" }"#).unwrap();
        assert_eq!((d.hotkey, ignored.len()), (None, 1));
    }

    #[test]
    fn settings_get_sensible_defaults() {
        let text = r#"{ "nom": "X", "commande": "a", "reglages": [
            { "cle": "n", "min": 0, "max": 9 },
            { "cle": "p", "min": 0, "max": 100, "pas": 25, "defaut": 60 },
            { "cle": "q", "min": 5, "max": 6, "defaut": 99 }
        ] }"#;
        let (d, _) = parse("x", text.as_bytes()).unwrap();
        assert_eq!(
            d.settings.iter().map(|s| (s.name.as_str(), s.step, s.default)).collect::<Vec<_>>(),
            [("n", 1, 0), ("p", 25, 50), ("q", 1, 6)]
        );
        assert_eq!(d.settings[1].at(80).label(), "75");
    }

    #[test]
    fn every_declaration_of_the_folder_is_read_in_order() {
        let dir = temp_dir("declarations");
        std::fs::write(dir.join("zeta.json"), br#"{ "nom": "Zeta", "commande": "z.exe" }"#).unwrap();
        std::fs::write(dir.join("alpha.json"), br#"{ "nom": "Alpha", "commande": "a.exe" }"#).unwrap();
        std::fs::write(dir.join("casse.json"), b"{ pas du json").unwrap();
        std::fs::write(dir.join("multi-clic.json"), br#"{ "nom": "Doublon", "commande": "m.exe" }"#).unwrap();
        std::fs::write(dir.join("notes.txt"), b"rien").unwrap();
        let (declarations, log) = read_all(&dir, &["maintenir-clic", "multi-clic"]);
        let ids: Vec<&str> = declarations.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, ["alpha", "zeta"]);
        assert!(
            log.iter().any(|l| l.starts_with("Macro externe « casse » laissée de côté : JSON illisible")),
            "{log:?}"
        );
        assert!(log.iter().any(|l| l.contains("« multi-clic » laissée de côté : ce nom est celui")), "{log:?}");
        assert_eq!(log.len(), 4, "{log:?}");
        // No folder: nothing, silently.
        assert_eq!(read_all(&dir.join("absent"), &[]), (Vec::new(), Vec::new()));
    }

    #[test]
    fn a_missing_folder_or_script_is_found_before_the_start() {
        let dir = temp_dir("manquant");
        std::fs::write(dir.join("present.ps1"), b"# exemple").unwrap();
        let declare = |folder: Option<PathBuf>, arguments: &[&str]| Declaration {
            folder,
            arguments: arguments.iter().map(|a| a.to_string()).collect(),
            ..parse("x", br#"{ "nom": "X", "commande": "powershell.exe" }"#).unwrap().0
        };
        let script = dir.join("present.ps1").to_string_lossy().to_string();
        assert_eq!(missing(&declare(None, &["-File", &script])), None);
        assert_eq!(missing(&declare(Some(dir.clone()), &["-File", "present.ps1"])), None);
        assert_eq!(missing(&declare(Some(dir.clone()), &["-NoProfile"])), None, "no script among the arguments");
        let absent = dir.join("absent.PS1").to_string_lossy().to_string();
        assert_eq!(
            missing(&declare(None, &["-File", &absent])).as_deref(),
            Some("Son script est introuvable sur ce PC.")
        );
        assert_eq!(
            missing(&declare(Some(dir.join("ailleurs")), &[])).as_deref(),
            Some("Son dossier de travail est introuvable sur ce PC.")
        );
    }

    #[test]
    fn lines_of_the_script_are_read_tolerantly() {
        assert_eq!(read_line(b"etat 12/100\r\n"), "etat 12/100");
        assert_eq!(read_line("\u{feff}etat é".as_bytes()), "etat é");
        assert_eq!(read_line(b"etat \xFF\xFE ok"), "etat \u{FFFD}\u{FFFD} ok");
        let long = "x".repeat(500);
        assert_eq!(read_line(long.as_bytes()).chars().count(), LINE_LIMIT);
        assert_eq!(read_line("é".repeat(300).as_bytes()).chars().count(), LINE_LIMIT, "characters, not bytes");
    }

    #[test]
    fn every_word_of_the_protocol_is_understood() {
        let badge = |color, text: &str| FromScript::Badge(Some(Badge { color, text: text.into() }));
        for (line, expected) in [
            ("etat 12/100", FromScript::State("12/100".into())),
            ("etat", FromScript::State(String::new())),
            ("pastille ambre Attente", badge(BadgeColor::Amber, "Attente")),
            ("pastille verte Clic !", badge(BadgeColor::Green, "Clic !")),
            ("pastille bleue Relance", badge(BadgeColor::Blue, "Relance")),
            ("pastille grise Pause 8 s", badge(BadgeColor::Grey, "Pause 8 s")),
            ("pastille", FromScript::Badge(None)),
            ("pastille rose X", FromScript::Other("pastille rose X".into())),
            ("pastille ambre", FromScript::Other("pastille ambre".into())),
            ("attente attend sa fenêtre", FromScript::Waiting("attend sa fenêtre".into())),
            ("reprise", FromScript::Resume),
            (
                "alerte La fenêtre visée est en plein écran. | Passe-la en fenêtre.",
                FromScript::Alert(Alert {
                    title: "La fenêtre visée est en plein écran.".into(),
                    text: "Passe-la en fenêtre.".into(),
                }),
            ),
            ("alerte Sans texte", FromScript::Alert(Alert { title: "Sans texte".into(), text: String::new() })),
            ("fin 100/100 clics", FromScript::Finished("100/100 clics".into())),
            ("journal clic 12", FromScript::Log("clic 12".into())),
            ("Bonjour", FromScript::Other("Bonjour".into())),
            ("", FromScript::Other(String::new())),
        ] {
            assert_eq!(parse_line(line), expected, "{line:?}");
        }
        assert_eq!(setting_line("limite", 120), "reglage limite 120");
    }

    #[test]
    fn what_the_script_said_makes_its_line() {
        let mut said = Said::default();
        assert_eq!(said.take(parse_line("etat 1/10"), "Exemple"), None);
        said.take(parse_line("pastille ambre Attente"), "Exemple");
        said.take(parse_line("attente attend sa fenêtre"), "Exemple");
        assert_eq!(said.waiting.as_deref(), Some("attend sa fenêtre"));
        said.take(parse_line("reprise"), "Exemple");
        assert_eq!((said.state.as_deref(), said.waiting.clone()), (Some("1/10"), None));
        assert!(said.badge.is_some(), "the badge comes back after the wait");
        said.take(parse_line("pastille"), "Exemple");
        assert_eq!(said.badge, None);
        assert_eq!(said.take(parse_line("journal clic"), "Exemple"), Some("« Exemple » : clic".into()));
        assert_eq!(said.take(parse_line("bonjour"), "Exemple"), Some("« Exemple » (sortie) : bonjour".into()));
        assert!(said.take(parse_line("alerte T | X"), "Exemple").is_some());
        assert_eq!(said.alert, Some(Alert { title: "T".into(), text: "X".into() }));
        said.take(parse_line("fin 10/10"), "Exemple");
        assert_eq!(said.finished.as_deref(), Some("10/10"));
        said.take(parse_line("etat"), "Exemple");
        assert_eq!(said.state, None, "an empty state: « ● Active » alone");
    }
}
