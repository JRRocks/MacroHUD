//! The example external macro (`exemple.ps1`, next to this file), run for real by MacroHUD's engine
//! and its process layer, as on the PC, with a pretend mouse and the HUD's requests as commands: every
//! case of `docs/MACROS-EXTERNES.md` (« Tests »), start, settings, badge, `attente`, `fin`, requested
//! stop, forced stop after 2 s, exit in error, closed input; and an alert followed by an exit.
//!
//! Compiled into `cargo test` by `src/main.rs`. It needs PowerShell (Windows PowerShell 5.1 and
//! PowerShell 7 on Windows, `pwsh` elsewhere): without it the cases are skipped, except in the CI
//! (`CI` set), where they fail.

use std::path::PathBuf;
use std::process::{Command as Process, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use crate::contract::{Alert, BadgeColor, ExternalLine, MacroId, MacroStatus, MouseButton};
use crate::engine::{
    Backend, Command, Engine, Event, ExternalSetup, ExternalSnapshot, HotkeyId, Launch, RegisterError,
    SCRIPT_STOP_WAIT, ScriptOutput,
};
use crate::external::{self, Declaration};
use crate::guard::ButtonSet;
use crate::hotkey::{Hotkey, Modifiers, Vk, vk};
use crate::script::{self, Report, Scripts};

const EXAMPLE: MacroId = MacroId("exemple");
/// PowerShell may take a few seconds to start on a busy machine.
const PATIENCE: Duration = Duration::from_secs(30);
/// How often the engine looks around while a case waits (as the engine thread does, at most).
const TICK: Duration = Duration::from_millis(20);

/// The folder of the example.
fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("macro-exemple")
}

/// The PowerShells to run the example with: Windows PowerShell 5.1 and PowerShell 7 on Windows,
/// PowerShell 7 elsewhere; those missing are left out.
fn shells() -> &'static [&'static str] {
    static SHELLS: OnceLock<Vec<&'static str>> = OnceLock::new();
    SHELLS.get_or_init(|| {
        let candidates: &[&'static str] = if cfg!(windows) { &["powershell.exe", "pwsh.exe"] } else { &["pwsh"] };
        let found: Vec<&'static str> = candidates
            .iter()
            .copied()
            .filter(|shell| {
                Process::new(shell)
                    .args(["-NoProfile", "-NonInteractive", "-Command", "exit 0"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|status| status.success())
            })
            .collect();
        if found.is_empty() {
            assert!(
                std::env::var_os("CI").is_none(),
                "PowerShell est introuvable : la macro d'exemple n'est pas essayée."
            );
            eprintln!("PowerShell introuvable : essais de la macro d'exemple sautés.");
        }
        found
    })
}

/// Runs `case` with each PowerShell found, one script at a time (see `script::ONE_AT_A_TIME`).
fn each_shell(case: impl Fn(&str)) {
    for shell in shells() {
        let _one = script::ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
        eprintln!("Macro d'exemple avec {shell}.");
        case(shell);
    }
}

/// The declaration of the example, as on the PC (`exemple.json`), with `shell` and its `arguments`.
fn declaration(shell: &str, arguments: &[&str]) -> Declaration {
    let mut all = vec!["-NoProfile", "-NonInteractive"];
    if cfg!(windows) {
        all.extend(["-ExecutionPolicy", "Bypass"]);
    }
    all.extend(["-File", "exemple.ps1"]);
    all.extend(arguments);
    let text = serde_json::json!({
        "nom": "Exemple",
        "commande": shell,
        "arguments": all,
        "dossier": folder(),
        "raccourci": "F10",
        "boutons": ["droit"],
        "reglages": [
            { "cle": "limite", "nom": "Limite de clics", "unite": "clics", "min": 10, "max": 500, "pas": 10, "defaut": 100 }
        ],
        "etat_arretee": "limite {limite} clics",
    });
    external::parse("exemple", text.to_string().as_bytes()).expect("the declaration of the example").0
}

/// What the cases see of the pretend system.
#[derive(Default)]
struct Seen {
    /// The buttons down (a case says which); `release_down` releases them.
    down: ButtonSet,
    /// What `release_down` released, each time it was called.
    released: Vec<ButtonSet>,
}

/// A pretend mouse and keyboard; the scripts are real.
struct Bench {
    scripts: Scripts,
    report: Report,
    on_hud: Arc<AtomicBool>,
    seen: Arc<Mutex<Seen>>,
    next_id: HotkeyId,
}

impl Backend for Bench {
    fn watch(&mut self, _held: ButtonSet, _clicked: ButtonSet) -> Result<(), String> {
        Ok(())
    }
    fn is_down(&self, _button: MouseButton) -> bool {
        false
    }
    fn press(&mut self, _button: MouseButton) -> Result<(), String> {
        Ok(())
    }
    fn adopt(&mut self, _button: MouseButton) {}
    fn release(&mut self, _button: MouseButton) -> Result<(), String> {
        Ok(())
    }
    fn click_down(&mut self, _button: MouseButton) -> Result<(), String> {
        Ok(())
    }
    fn click_up(&mut self, _button: MouseButton) -> Result<bool, String> {
        Ok(false)
    }
    fn pointer_on_hud(&self) -> bool {
        self.on_hud.load(Ordering::SeqCst)
    }
    fn register_hotkey(&mut self, _hotkey: Hotkey) -> Result<HotkeyId, RegisterError> {
        self.next_id += 1;
        Ok(self.next_id)
    }
    fn unregister_hotkey(&mut self, _id: HotkeyId) {}
    fn start_capture(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn stop_capture(&mut self) {}
    fn altgr_char(&self, _hotkey: Hotkey) -> Option<char> {
        None
    }
    fn layout_char(&self, _key: Vk) -> Option<char> {
        None
    }
    fn guard_hud(&mut self, _on: bool) -> Result<(), String> {
        Ok(())
    }
    fn launch(&mut self, launch: &Launch) -> Result<u64, String> {
        self.scripts.launch(launch, self.report.clone())
    }
    fn tell(&mut self, run: u64, line: &str) -> Result<(), String> {
        self.scripts.tell(run, line)
    }
    fn script_ended(&mut self, run: u64) -> Option<Option<i32>> {
        self.scripts.ended(run)
    }
    fn kill(&mut self, run: u64) {
        self.scripts.kill(run);
    }
    fn script_done(&mut self, run: u64) {
        self.scripts.done(run);
    }
    fn release_down(&mut self, buttons: ButtonSet) -> ButtonSet {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        let down = ButtonSet::from_bits(seen.down.bits() & buttons.bits());
        seen.down = ButtonSet::from_bits(seen.down.bits() & !buttons.bits());
        seen.released.push(down);
        down
    }
}

/// One run of the example: the engine, the output of its script, what the pretend system saw.
struct Case {
    engine: Engine<Bench>,
    events: Receiver<Event>,
    on_hud: Arc<AtomicBool>,
    seen: Arc<Mutex<Seen>>,
}

impl Case {
    fn new(shell: &str, arguments: &[&str]) -> Case {
        Case::with(declaration(shell, arguments))
    }

    fn with(declaration: Declaration) -> Case {
        let (sender, events) = mpsc::channel();
        let report: Report = Arc::new(move |run, output| {
            let _ = sender.send(Event::Script { run, output });
        });
        let on_hud = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(Mutex::new(Seen::default()));
        let bench =
            Bench { scripts: Scripts::default(), report, on_hud: on_hud.clone(), seen: seen.clone(), next_id: 0 };
        let setup = ExternalSetup { hotkey: declaration.hotkey, values: declaration.defaults(), declaration };
        let mut engine = Engine::new(bench, &[], vec![setup]);
        engine.start();
        Case { engine, events, on_hud, seen }
    }

    fn view(&self) -> ExternalSnapshot {
        self.engine.snapshot().externals[0].clone()
    }

    fn ask(&mut self, command: Command) {
        self.engine.command(command, Instant::now());
    }

    fn set_limit(&mut self, value: i64) {
        self.ask(Command::SetSetting(EXAMPLE, "limite".into(), value));
    }

    /// Its declared button is down, as if its script held it.
    fn hold_right(&self) {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner).down = ButtonSet::only(MouseButton::Right);
    }

    fn released(&self) -> Vec<ButtonSet> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner).released.clone()
    }

    /// One turn of the engine thread: the next output of the script, or the next deadline.
    fn turn(&mut self) {
        let wait =
            self.engine.next_deadline().map_or(TICK, |at| at.saturating_duration_since(Instant::now()).min(TICK));
        if let Ok(event) = self.events.recv_timeout(wait) {
            self.engine.event(event, Instant::now());
        }
        self.engine.tick(Instant::now());
    }

    /// Runs the engine until `done` holds for the line of the example.
    fn until(&mut self, what: &str, done: impl Fn(&ExternalSnapshot) -> bool) -> ExternalSnapshot {
        let start = Instant::now();
        loop {
            let view = self.view();
            if done(&view) {
                return view;
            }
            assert!(start.elapsed() < PATIENCE, "{what} : {view:?}");
            self.turn();
        }
    }

    /// Runs the engine for `duration`.
    fn run_for(&mut self, duration: Duration) {
        let end = Instant::now() + duration;
        while Instant::now() < end {
            self.turn();
        }
    }

    /// Quits as MacroHUD does; returns how long it took.
    fn quit(&mut self) -> Duration {
        let start = Instant::now();
        let mut next = self.engine.shutdown(Instant::now());
        while let Some(at) = next {
            if start.elapsed() > PATIENCE {
                break;
            }
            if let Ok(event) = self.events.recv_timeout(at.saturating_duration_since(Instant::now())) {
                self.engine.event(event, Instant::now());
            }
            next = self.engine.shutdown(Instant::now());
        }
        start.elapsed()
    }
}

impl Drop for Case {
    /// A case that failed half way leaves no script behind.
    fn drop(&mut self) {
        self.quit();
    }
}

/// The text after « ● Active · ».
fn active(view: &ExternalSnapshot) -> Option<&str> {
    match &view.line {
        ExternalLine::Active(Some(text)) => Some(text),
        _ => None,
    }
}

#[test]
fn it_starts_with_its_settings_shows_its_line_and_stops_when_asked() {
    each_shell(|shell| {
        let mut case = Case::new(shell, &["-Interval", "100"]);
        // A setting changed while it is stopped: kept, shown, given at the start.
        case.set_limit(500);
        assert_eq!(case.view().line, ExternalLine::Stopped(Some("limite 500 clics".into())));
        case.ask(Command::Start(EXAMPLE));
        assert_eq!(case.view().status, MacroStatus::Active);
        let view = case.until("its first badge", |v| v.badge.is_some());
        assert_eq!(view.badge.map(|b| (b.color, b.text)), Some((BadgeColor::Blue, "Départ".into())));
        let view = case.until("its second click", |v| active(v).is_some_and(|t| t.starts_with("2/")));
        assert_eq!(active(&view), Some("2/500"));
        assert_eq!(view.badge.map(|b| (b.color, b.text)), Some((BadgeColor::Green, "Clics".into())));
        // A new limit while it runs: at once.
        case.set_limit(400);
        case.until("the new limit", |v| active(v).is_some_and(|t| t.ends_with("/400")));
        // The mouse on the HUD: it is told to pause, and stops counting.
        case.on_hud.store(true, Ordering::SeqCst);
        let view = case.until("its pause", |v| v.badge.as_ref().is_some_and(|b| b.color == BadgeColor::Amber));
        assert!(view.suspended);
        case.run_for(Duration::from_millis(300));
        let paused = case.view().line;
        case.run_for(Duration::from_millis(500));
        assert_eq!(case.view().line, paused, "no click while paused");
        // The mouse leaves: it goes on.
        case.on_hud.store(false, Ordering::SeqCst);
        case.until("its resumption", |v| {
            !v.suspended && v.badge.as_ref().is_some_and(|b| b.color == BadgeColor::Green)
        });
        case.until("a click after the pause", |v| v.line != paused);
        // Asked to stop: it quits by itself, well within the 2 s; nothing to release.
        case.hold_right();
        let asked = Instant::now();
        case.ask(Command::Stop(EXAMPLE));
        assert_eq!(case.view().status, MacroStatus::Active, "active until its script has quit");
        let view = case.until("its end", |v| v.status == MacroStatus::Stopped);
        assert!(asked.elapsed() < SCRIPT_STOP_WAIT, "it took {:?}", asked.elapsed());
        assert_eq!(view.line, ExternalLine::Stopped(Some("limite 400 clics".into())));
        assert_eq!(view.alert, None);
        assert!(case.released().is_empty(), "nothing released after a stop it obeyed");
    });
}

#[test]
fn at_its_limit_it_ends_with_fin() {
    each_shell(|shell| {
        let mut case = Case::new(shell, &["-Interval", "20"]);
        case.set_limit(10);
        case.ask(Command::Start(EXAMPLE));
        let view = case.until("its end", |v| v.status == MacroStatus::Stopped);
        assert_eq!((view.line, view.alert), (ExternalLine::Finished("10/10 clics".into()), None));
        assert!(case.released().is_empty(), "it ended well: nothing released");
        // Until the next start.
        case.ask(Command::Start(EXAMPLE));
        assert_eq!(case.view().line, ExternalLine::Active(None));
        case.ask(Command::Stop(EXAMPLE));
        case.until("its second end", |v| v.status == MacroStatus::Stopped);
    });
}

#[test]
fn while_it_waits_its_line_says_why() {
    each_shell(|shell| {
        let mut case = Case::new(shell, &["-Mode", "wait", "-Interval", "300"]);
        case.ask(Command::Start(EXAMPLE));
        let view = case.until("its wait", |v| v.line == ExternalLine::Waiting("attend sa fenêtre".into()));
        assert_eq!((view.status, view.badge), (MacroStatus::Active, None), "no badge while it waits");
        case.until("its resumption", |v| matches!(v.line, ExternalLine::Active(_)));
        case.until("its first click", |v| active(v).is_some_and(|t| t.starts_with("1/")));
        case.ask(Command::Stop(EXAMPLE));
        case.until("its end", |v| v.status == MacroStatus::Stopped);
    });
}

#[test]
fn a_script_deaf_to_arret_is_stopped_by_force_after_2_s() {
    each_shell(|shell| {
        let mut case = Case::new(shell, &["-Mode", "deaf", "-Interval", "100"]);
        case.ask(Command::Start(EXAMPLE));
        case.until("its first click", |v| active(v).is_some_and(|t| t.starts_with("1/")));
        case.hold_right();
        let asked = Instant::now();
        case.ask(Command::Stop(EXAMPLE));
        let view = case.until("its forced end", |v| v.status == MacroStatus::Stopped);
        let took = asked.elapsed();
        assert!(took >= SCRIPT_STOP_WAIT && took < SCRIPT_STOP_WAIT + Duration::from_secs(3), "it took {took:?}");
        assert_eq!(case.released(), vec![ButtonSet::only(MouseButton::Right)], "its declared button released");
        assert_eq!(view.line, ExternalLine::Stopped(Some("limite 100 clics".into())));
    });
}

#[test]
fn a_script_that_fails_has_stopped_by_itself() {
    each_shell(|shell| {
        let mut case = Case::new(shell, &["-Mode", "error", "-Interval", "100"]);
        case.hold_right();
        case.ask(Command::Start(EXAMPLE));
        let view = case.until("its end", |v| v.status == MacroStatus::Stopped);
        assert_eq!(view.line, ExternalLine::Stopped(Some("limite 100 clics".into())));
        assert_eq!(
            view.alert,
            Some(Alert { title: "« Exemple » s'est arrêtée seule.".into(), text: "Code de sortie : 3.".into() })
        );
        assert_eq!(case.released(), vec![ButtonSet::only(MouseButton::Right)], "its declared button released");
    });
}

/// Its own alert, then an exit without `fin`, whatever its exit code: its box alone, no « s'est
/// arrêtée seule » (mock-up, view 7); its declared buttons released all the same.
#[test]
fn an_alert_then_an_exit_keeps_its_box_alone() {
    each_shell(|shell| {
        for code in ["1", "0"] {
            let mut case = Case::new(shell, &["-Mode", "alert", "-Interval", "100", "-ExitCode", code]);
            case.hold_right();
            case.ask(Command::Start(EXAMPLE));
            let view = case.until("its end", |v| v.status == MacroStatus::Stopped);
            assert_eq!(view.line, ExternalLine::Stopped(Some("limite 100 clics".into())), "code {code}");
            assert_eq!(
                view.alert,
                Some(Alert {
                    title: "La fenêtre visée est en plein écran.".into(),
                    text: "Passe-la en fenêtre : en plein écran, la macro ne voit pas l'écran.".into(),
                }),
                "code {code}"
            );
            assert_eq!(case.released(), vec![ButtonSet::only(MouseButton::Right)], "code {code}");
        }
    });
}

/// As when MacroHUD ends without a word (killed, crashed): the end of its input stops it.
#[test]
fn when_its_input_closes_it_quits() {
    each_shell(|shell| {
        for (mode, quits) in [("normal", true), ("deaf", false)] {
            let declaration = declaration(shell, &["-Mode", mode, "-Interval", "100"]);
            let (sender, lines) = mpsc::channel();
            let report: Report = Arc::new(move |_, output| {
                let _ = sender.send(output);
            });
            let mut scripts = Scripts::default();
            let launch = Launch {
                name: declaration.name.clone(),
                command: declaration.command.clone(),
                arguments: declaration.arguments.clone(),
                folder: declaration.folder.clone(),
                buttons: declaration.buttons,
            };
            let run = scripts.launch(&launch, report).expect("the example starts");
            let first = lines.recv_timeout(PATIENCE).expect("its first line");
            assert!(matches!(first, ScriptOutput::Line(_)), "{first:?}");
            scripts.close_input(run);
            let closed = Instant::now();
            while scripts.ended(run).is_none() && closed.elapsed() < SCRIPT_STOP_WAIT {
                std::thread::sleep(TICK);
            }
            if quits {
                assert_eq!(scripts.ended(run), Some(Some(0)), "{mode}: it quits at once, as with « arret »");
            } else {
                assert_eq!(scripts.ended(run), None, "{mode}: it does not");
                scripts.kill(run);
                while scripts.ended(run).is_none() {
                    assert!(closed.elapsed() < PATIENCE, "{mode}: it should be killed");
                    std::thread::sleep(TICK);
                }
            }
            scripts.done(run);
        }
    });
}

/// « Quitter » or Alt+F4: `arret` first, then the 2 s at most.
#[test]
fn quitting_macrohud_stops_it_first() {
    each_shell(|shell| {
        let mut case = Case::new(shell, &["-Interval", "100"]);
        case.ask(Command::Start(EXAMPLE));
        case.until("its first click", |v| active(v).is_some_and(|t| t.starts_with("1/")));
        let took = case.quit();
        assert!(took < SCRIPT_STOP_WAIT, "it quit when asked, in {took:?}");
        assert_eq!(case.view().status, MacroStatus::Stopped);
        // One that does not hear: stopped by force at the end of the 2 s.
        let mut case = Case::new(shell, &["-Mode", "deaf", "-Interval", "100"]);
        case.ask(Command::Start(EXAMPLE));
        case.until("its first click", |v| active(v).is_some_and(|t| t.starts_with("1/")));
        case.hold_right();
        let took = case.quit();
        assert!(took >= SCRIPT_STOP_WAIT && took < SCRIPT_STOP_WAIT + Duration::from_secs(3), "{took:?}");
        assert_eq!(case.released(), vec![ButtonSet::only(MouseButton::Right)]);
    });
}

/// Neither its script nor its command found: « ne peut pas démarrer », and why. (No PowerShell needed.)
#[test]
fn a_missing_script_or_command_cannot_start() {
    let mut missing = declaration("powershell.exe", &[]);
    for argument in &mut missing.arguments {
        if argument == "exemple.ps1" {
            *argument = "introuvable.ps1".into();
        }
    }
    let mut case = Case::with(missing);
    case.ask(Command::Start(EXAMPLE));
    let view = case.view();
    assert_eq!((view.status, view.line), (MacroStatus::Stopped, ExternalLine::Stopped(None)));
    assert_eq!(
        view.alert,
        Some(Alert {
            title: "« Exemple » ne peut pas démarrer.".into(),
            text: "Son script est introuvable sur ce PC. Les autres macros marchent.".into(),
        })
    );
    let mut no_command = declaration("macrohud-commande-introuvable", &[]);
    no_command.arguments.clear();
    let mut case = Case::with(no_command);
    case.ask(Command::Start(EXAMPLE));
    let view = case.view();
    assert_eq!(view.status, MacroStatus::Stopped);
    assert_eq!(
        view.alert.map(|a| a.text),
        Some("« macrohud-commande-introuvable » est introuvable sur ce PC. Les autres macros marchent.".into())
    );
}

/// The declaration to copy on the PC (`exemple.json`) is valid, and names no real place.
#[test]
fn the_declaration_given_as_example_is_valid() {
    let bytes = std::fs::read(folder().join("exemple.json")).expect("exemple.json");
    let (declaration, ignored) = external::parse("exemple", &bytes).expect("a valid declaration");
    assert!(ignored.is_empty(), "{ignored:?}");
    assert_eq!((declaration.name.as_str(), declaration.command.as_str()), ("Exemple", "powershell.exe"));
    let f10 = Hotkey::new(Modifiers::NONE, vk::F10);
    assert_eq!((declaration.hotkey, declaration.buttons), (Some(f10), ButtonSet::only(MouseButton::Right)));
    assert_eq!(declaration.defaults(), vec![100]);
    assert_eq!(declaration.stopped_line(&[100]).as_deref(), Some("limite 100 clics"));
    let text = String::from_utf8(bytes).expect("UTF-8");
    assert!(text.contains("<utilisateur>"), "a placeholder, not the path of a PC");
    // The script reads its accents right on Windows PowerShell 5.1: it starts with a BOM.
    let script = std::fs::read(folder().join("exemple.ps1")).expect("exemple.ps1");
    assert!(script.starts_with(&[0xEF, 0xBB, 0xBF]), "exemple.ps1 must be saved in UTF-8 with a BOM");
}
