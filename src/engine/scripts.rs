//! The engine's side of the external macros (`docs/MACROS-EXTERNES.md`): their scripts run only
//! while the macro is active; MacroHUD gives them their settings, tells them to pause while the
//! mouse is on the HUD (D-22), asks them to stop, stops them by force after
//! [`SCRIPT_STOP_WAIT`], and releases their declared buttons when they end badly.

use std::time::{Duration, Instant};

use super::{Backend, Cause, Engine, ExternalSnapshot, Launch, ScriptOutput};
use crate::contract::{Alert, ExternalLine, MacroStatus, MouseButton};
use crate::external::{self, Declaration, Said};
use crate::guard::ButtonSet;

/// How long a script asked to stop (`arret`) has to quit before it is stopped by force.
pub const SCRIPT_STOP_WAIT: Duration = Duration::from_secs(2);
/// How often MacroHUD looks at a running script: has it ended, is the mouse on the HUD.
pub const SCRIPT_CHECK: Duration = Duration::from_millis(50);
/// After the end of its process, how long the last lines of a script are waited for.
const OUTPUT_GRACE: Duration = Duration::from_millis(500);

/// An external macro, in the engine.
#[derive(Debug)]
pub(super) struct External {
    pub(super) declaration: Declaration,
    /// The values of its settings, in declaration order.
    pub(super) values: Vec<i64>,
    phase: Phase,
    /// What the script of the current (or last) run said.
    said: Said,
    /// MacroHUD's own amber box (« ne peut pas démarrer », « s'est arrêtée seule »).
    alert: Option<Alert>,
    /// The last run ended with `fin`: « ✓ Terminée · texte », until the next start.
    finished: Option<String>,
    /// The last start failed: « ■ Arrêtée » alone.
    failed: bool,
}

#[derive(Debug)]
enum Phase {
    Stopped,
    Running(Script),
    /// `arret` written: the script has until the deadline to quit.
    Stopping(Script, Instant, Cause),
}

/// One run of a script.
#[derive(Debug)]
struct Script {
    run: u64,
    since: Instant,
    /// `pause` was written (the mouse is on the HUD).
    paused: bool,
    /// Its output ended.
    closed: bool,
    /// Its process ended (with its code), and when MacroHUD saw it.
    ended: Option<(Option<i32>, Instant)>,
    next_check: Instant,
}

/// How a run ended.
enum End {
    /// The process quit, with this code.
    Exited(Option<i32>),
    /// MacroHUD stopped it by force, and released these declared buttons.
    Killed(ButtonSet),
}

impl External {
    pub(super) fn new(declaration: Declaration, values: Vec<i64>) -> Self {
        External {
            declaration,
            values,
            phase: Phase::Stopped,
            said: Said::default(),
            alert: None,
            finished: None,
            failed: false,
        }
    }

    /// Its script runs (or is being stopped).
    pub(super) fn running(&self) -> bool {
        !matches!(self.phase, Phase::Stopped)
    }

    fn script_mut(&mut self) -> Option<&mut Script> {
        match &mut self.phase {
            Phase::Running(script) | Phase::Stopping(script, ..) => Some(script),
            Phase::Stopped => None,
        }
    }

    /// When the engine must look at it again.
    pub(super) fn deadline(&self) -> Option<Instant> {
        let (script, stop) = match &self.phase {
            Phase::Stopped => return None,
            Phase::Running(script) => (script, None),
            Phase::Stopping(script, deadline, _) => (script, Some(*deadline)),
        };
        let grace = script.ended.map(|(_, at)| at + OUTPUT_GRACE);
        [Some(script.next_check), stop, grace].into_iter().flatten().min()
    }
}

fn names(buttons: ButtonSet) -> String {
    let names: Vec<&str> = buttons.iter().map(MouseButton::phrase).collect();
    if names.is_empty() { "aucun".to_string() } else { names.join(", ") }
}

fn code_text(code: Option<i32>) -> String {
    code.map_or_else(|| "sans code".to_string(), |c| format!("code {c}"))
}

impl<B: Backend> Engine<B> {
    fn external(&self, i: usize) -> &External {
        self.slots[i].external.as_deref().expect("external macro")
    }

    fn external_mut(&mut self, i: usize) -> &mut External {
        self.slots[i].external.as_deref_mut().expect("external macro")
    }

    /// Starts the script of external macro `i` (the other macros are already stopped).
    pub(super) fn start_external(&mut self, i: usize, by: &str, now: Instant) {
        let name = self.slots[i].def.name;
        let x = self.external_mut(i);
        x.said = Said::default();
        x.alert = None;
        x.finished = None;
        x.failed = false;
        if let Some(why) = external::missing(&x.declaration) {
            crate::log::write(format!("« {name} » ne démarre pas : {why} ({:?})", x.declaration.arguments));
            self.cannot_start(i, &why);
            return;
        }
        // The guard first: while the script runs, no injected click reaches the HUD (D-22).
        if let Err(e) = self.backend.guard_hud(true) {
            self.cannot_start(i, &format!("Impossible de surveiller la souris ({e})."));
            return;
        }
        let declaration = &self.external(i).declaration;
        let launch = Launch {
            name: name.to_string(),
            command: declaration.command.clone(),
            arguments: declaration.arguments.clone(),
            folder: declaration.folder.clone(),
            buttons: declaration.buttons,
        };
        match self.backend.launch(&launch) {
            Ok(run) => {
                crate::log::write(format!("« {name} » démarre, {by} : {} {:?}.", launch.command, launch.arguments));
                let script = Script {
                    run,
                    since: now,
                    paused: false,
                    closed: false,
                    ended: None,
                    next_check: now + SCRIPT_CHECK,
                };
                self.external_mut(i).phase = Phase::Running(script);
                let x = self.external(i);
                let lines: Vec<String> = x
                    .declaration
                    .settings
                    .iter()
                    .zip(&x.values)
                    .map(|(setting, value)| external::setting_line(&setting.key, *value))
                    .collect();
                for line in lines {
                    self.tell(i, &line);
                }
                if self.backend.pointer_on_hud() {
                    self.tell(i, external::PAUSE);
                    if let Some(script) = self.external_mut(i).script_mut() {
                        script.paused = true;
                    }
                }
            }
            Err(why) => {
                let _ = self.backend.guard_hud(false);
                crate::log::write(format!("« {name} » ne démarre pas : {why}"));
                self.cannot_start(i, &why);
            }
        }
    }

    fn cannot_start(&mut self, i: usize, why: &str) {
        let name = self.slots[i].def.name;
        let x = self.external_mut(i);
        x.failed = true;
        x.alert = Some(Alert {
            title: format!("« {name} » ne peut pas démarrer."),
            text: format!("{why} Les autres macros marchent."),
        });
    }

    /// Writes a line to the running script of `i` (a refusal goes to the run log: the end of the
    /// script shows soon).
    fn tell(&mut self, i: usize, line: &str) {
        let Some(run) = self.external_mut(i).script_mut().map(|s| s.run) else { return };
        if let Err(e) = self.backend.tell(run, line) {
            crate::log::write(format!("« {} » : « {line} » non transmis ({e}).", self.slots[i].def.name));
        }
    }

    /// Asks the script of `i` to stop; it is stopped by force after [`SCRIPT_STOP_WAIT`].
    pub(super) fn stop_external(&mut self, i: usize, cause: Cause, now: Instant) {
        let x = self.external_mut(i);
        let Phase::Running(_) = x.phase else { return };
        let Phase::Running(script) = std::mem::replace(&mut x.phase, Phase::Stopped) else { unreachable!() };
        x.phase = Phase::Stopping(script, now + SCRIPT_STOP_WAIT, cause);
        let why = self.cause_text(i, cause);
        crate::log::write(format!("« {} » : arrêt demandé à son script ({why}).", self.slots[i].def.name));
        self.tell(i, external::STOP);
    }

    /// A line or the end of the output of a script.
    pub(super) fn on_script(&mut self, run: u64, output: ScriptOutput, now: Instant) {
        let found = (0..self.slots.len()).find(|&i| {
            self.slots[i].external.as_deref_mut().and_then(External::script_mut).is_some_and(|s| s.run == run)
        });
        let Some(i) = found else { return };
        let name = self.slots[i].def.name;
        match output {
            ScriptOutput::Line(line) => {
                let message = external::parse_line(&external::read_line(line.as_bytes()));
                if let Some(log) = self.external_mut(i).said.take(message, name) {
                    crate::log::write(log);
                }
            }
            ScriptOutput::Closed => {
                if let Some(script) = self.external_mut(i).script_mut() {
                    script.closed = true;
                    script.next_check = now;
                }
                self.external_tick(i, now);
            }
        }
    }

    /// What is due for the script of `i` at `now`: has it ended, is the mouse on the HUD, is its time
    /// to stop over.
    pub(super) fn external_tick(&mut self, i: usize, now: Instant) {
        let Some(run) = self.external_mut(i).script_mut().map(|s| s.run) else { return };
        let stopping = matches!(self.external(i).phase, Phase::Stopping(..));
        let due = self.external_mut(i).script_mut().is_some_and(|s| now >= s.next_check);
        if due {
            let ended = self.backend.script_ended(run);
            let on_hud = !stopping && self.backend.pointer_on_hud();
            let name = self.slots[i].def.name;
            let script = self.external_mut(i).script_mut().expect("a script");
            script.next_check = now + SCRIPT_CHECK;
            if script.ended.is_none()
                && let Some(code) = ended
            {
                script.ended = Some((code, now));
            }
            if !stopping && script.ended.is_none() && on_hud != script.paused {
                script.paused = on_hud;
                let line = if on_hud { external::PAUSE } else { external::RESUME };
                crate::log::write(format!(
                    "« {name} » : « {line} » (souris {} le HUD).",
                    if on_hud { "sur" } else { "hors du" }
                ));
                self.tell(i, line);
            }
        }
        let script = self.external_mut(i).script_mut().expect("a script");
        if let Some((code, at)) = script.ended
            && (script.closed || now >= at + OUTPUT_GRACE)
        {
            self.finish_external(i, End::Exited(code), now);
            return;
        }
        if let Phase::Stopping(_, deadline, _) = self.external(i).phase
            && now >= deadline
        {
            self.backend.kill(run);
            let buttons = self.external(i).declaration.buttons;
            let released = self.backend.release_down(buttons);
            self.finish_external(i, End::Killed(released), now);
        }
    }

    fn finish_external(&mut self, i: usize, end: End, now: Instant) {
        let name = self.slots[i].def.name;
        let phase = std::mem::replace(&mut self.external_mut(i).phase, Phase::Stopped);
        let (script, cause) = match phase {
            Phase::Running(script) => (script, None),
            Phase::Stopping(script, _, cause) => (script, Some(cause)),
            Phase::Stopped => return,
        };
        // Whatever the script left running ends with it; then no shield for the HUD is needed.
        self.backend.script_done(script.run);
        let _ = self.backend.guard_hud(false);
        let seconds = super::seconds_text(now.saturating_duration_since(script.since));
        match (end, cause) {
            (End::Killed(released), cause) => {
                let why = cause.map_or_else(String::new, |c| format!(" ({})", self.cause_text(i, c)));
                crate::log::write(format!(
                    "« {name} » arrêtée de force{why} : son script ne s'est pas arrêté dans les {} s, après {seconds} s de marche ; boutons déclarés relâchés : {}.",
                    SCRIPT_STOP_WAIT.as_secs(),
                    names(released)
                ));
            }
            (End::Exited(code), Some(cause)) => {
                let why = self.cause_text(i, cause);
                let released = self.release_after(i, code);
                crate::log::write(format!(
                    "« {name} » arrêtée ({why}) : son script a quitté ({}) après {seconds} s{released}.",
                    code_text(code)
                ));
            }
            (End::Exited(code), None) => {
                let x = self.external_mut(i);
                if let Some(text) = x.said.finished.clone() {
                    x.finished = Some(text.clone());
                    let released = self.release_after(i, code);
                    crate::log::write(format!(
                        "« {name} » terminée ({}) après {seconds} s : {text}{released}",
                        code_text(code)
                    ));
                } else {
                    // Ended badly: what it may still hold is released.
                    let buttons = x.declaration.buttons;
                    let released = self.backend.release_down(buttons);
                    let x = self.external_mut(i);
                    if x.said.alert.is_none() {
                        x.alert = Some(Alert {
                            title: format!("« {name} » s'est arrêtée seule."),
                            text: match code {
                                Some(code) => format!("Code de sortie : {code}."),
                                None => "Son script a quitté sans code de sortie.".to_string(),
                            },
                        });
                    }
                    crate::log::write(format!(
                        "« {name} » s'est arrêtée seule ({}) après {seconds} s, sans « fin »{} ; boutons déclarés relâchés : {}.",
                        code_text(code),
                        if x.said.alert.is_some() { ", après son alerte" } else { "" },
                        names(released)
                    ));
                }
            }
        }
        // A macro started meanwhile waited for this one (one at a time, D-20).
        if let Some((j, trigger)) = self.pending.take() {
            self.start_macro(j, trigger, now);
        }
    }

    /// After a stop it was asked for, or its `fin`: a script that quits with code 0 has released what
    /// it held (protocol); with any other end, its declared buttons still down are released (D-47).
    /// Returns the end of the line for the run log.
    fn release_after(&mut self, i: usize, code: Option<i32>) -> String {
        if code == Some(0) {
            return String::new();
        }
        let released = self.backend.release_down(self.external(i).declaration.buttons);
        format!(" ; boutons déclarés relâchés : {}", names(released))
    }

    /// A new value for a setting of external macro `i`; it goes to the script at once if it runs.
    pub(super) fn set_setting(&mut self, i: usize, key: &str, value: i64) {
        let name = self.slots[i].def.name;
        let x = self.external_mut(i);
        let Some(k) = x.declaration.settings.iter().position(|s| s.key == key) else { return };
        let value = x.declaration.settings[k].at(value).value;
        if x.values[k] == value {
            return;
        }
        x.values[k] = value;
        let running = matches!(x.phase, Phase::Running(_));
        crate::log::write(format!("« {name} » : réglage « {key} » à {value}."));
        if running {
            self.tell(i, &external::setting_line(key, value));
        }
    }

    /// External macro `i` as the HUD shows it.
    pub(super) fn external_snapshot(&self, i: usize) -> ExternalSnapshot {
        let slot = &self.slots[i];
        let x = self.external(i);
        let (status, line, badge, suspended) = match &x.phase {
            Phase::Running(script) | Phase::Stopping(script, ..) => {
                let line = match &x.said.waiting {
                    Some(text) => ExternalLine::Waiting(text.clone()),
                    None => ExternalLine::Active(x.said.state.clone()),
                };
                let badge = if x.said.waiting.is_some() { None } else { x.said.badge.clone() };
                let paused = matches!(x.phase, Phase::Running(_)) && script.paused;
                (MacroStatus::Active, line, badge, paused)
            }
            Phase::Stopped => {
                let line = match (&x.finished, x.failed) {
                    (Some(text), _) => ExternalLine::Finished(text.clone()),
                    (None, true) => ExternalLine::Stopped(None),
                    (None, false) => ExternalLine::Stopped(x.declaration.stopped_line(&x.values)),
                };
                (MacroStatus::Stopped, line, None, false)
            }
        };
        let status = match &slot.unavailable {
            Some(reason) if status == MacroStatus::Stopped => MacroStatus::Unavailable(reason.clone()),
            _ => status,
        };
        ExternalSnapshot {
            id: slot.def.id,
            status,
            hotkey: (!slot.hotkey.is_none()).then_some(slot.hotkey),
            hotkey_label: self.label(slot.hotkey),
            values: x.values.clone(),
            line,
            badge,
            suspended,
            alert: x.alert.clone().or_else(|| x.said.alert.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Call, Fake};
    use super::super::*;
    use super::*;
    use crate::contract::{Badge, BadgeColor, MacroId};
    use crate::hotkey::{Modifiers, vk};
    use crate::macros::{HOLD_CLICK, MULTI_CLICK};

    const MS: Duration = Duration::from_millis(1);
    const EXAMPLE: MacroId = MacroId("exemple");

    fn declaration() -> Declaration {
        let text = r#"{ "nom": "Exemple", "commande": "outil.exe", "arguments": ["-File", "exemple.ps1"],
            "raccourci": "F10", "boutons": ["droit"],
            "reglages": [ { "cle": "limite", "nom": "Limite de clics", "unite": "clics", "min": 10, "max": 500, "pas": 10, "defaut": 100 } ],
            "etat_arretee": "limite {limite} clics" }"#;
        external::parse("exemple", text.as_bytes()).unwrap().0
    }

    fn setup(def: crate::macros::MacroDef) -> MacroSetup {
        MacroSetup { def, hotkey: def.default_hotkey, options: def.default_options }
    }

    /// The two macros of MacroHUD and the example, whose script file is present.
    fn engine_with(fake: Fake) -> Engine<Fake> {
        let mut declaration = declaration();
        declaration.arguments.clear();
        let externals = vec![ExternalSetup { hotkey: declaration.hotkey, values: declaration.defaults(), declaration }];
        let mut engine = Engine::new(fake, &[setup(HOLD_CLICK), setup(MULTI_CLICK)], externals);
        engine.start();
        engine.backend_mut().calls.clear();
        engine
    }

    fn engine() -> Engine<Fake> {
        engine_with(Fake::default())
    }

    fn f10(engine: &mut Engine<Fake>, now: Instant) {
        let id = engine.slots[2].registered.expect("F10 registered");
        engine.event(Event::Hotkey(id), now);
    }

    fn view(engine: &Engine<Fake>) -> ExternalSnapshot {
        engine.snapshot().externals[0].clone()
    }

    fn calls(engine: &mut Engine<Fake>) -> Vec<Call> {
        std::mem::take(&mut engine.backend_mut().calls)
    }

    fn line(engine: &mut Engine<Fake>, text: &str, now: Instant) {
        let run = engine.backend().last_run;
        engine.event(Event::Script { run, output: ScriptOutput::Line(text.into()) }, now);
    }

    fn closed(engine: &mut Engine<Fake>, now: Instant) {
        let run = engine.backend().last_run;
        engine.event(Event::Script { run, output: ScriptOutput::Closed }, now);
    }

    #[test]
    fn its_hotkey_starts_the_script_and_gives_it_its_settings() {
        let mut e = engine();
        let now = Instant::now();
        assert_eq!(e.snapshot().externals[0].hotkey_label, "F10");
        assert_eq!(view(&e).line, ExternalLine::Stopped(Some("limite 100 clics".into())));
        f10(&mut e, now);
        assert_eq!(
            calls(&mut e),
            vec![Call::GuardHud(true), Call::Launch("outil.exe".into()), Call::Tell("reglage limite 100".into())]
        );
        let v = view(&e);
        assert_eq!((v.status, v.line, v.badge, v.alert), (MacroStatus::Active, ExternalLine::Active(None), None, None));
        // Its declared buttons are not watched: a real click does not stop it (D-35).
        assert_eq!(e.guarded_buttons(), ButtonSet::EMPTY);
        e.event(Event::Click { buttons: ButtonSet::only(MouseButton::Right), on_hud: false }, now);
        assert_eq!(view(&e).status, MacroStatus::Active);
    }

    #[test]
    fn what_the_script_says_shows_on_its_line() {
        let mut e = engine();
        let now = Instant::now();
        f10(&mut e, now);
        line(&mut e, "etat 12/100", now);
        line(&mut e, "pastille ambre Attente", now);
        let v = view(&e);
        assert_eq!(v.line, ExternalLine::Active(Some("12/100".into())));
        assert_eq!(v.badge, Some(Badge { color: BadgeColor::Amber, text: "Attente".into() }));
        line(&mut e, "attente attend sa fenêtre", now);
        let v = view(&e);
        assert_eq!((v.line, v.badge), (ExternalLine::Waiting("attend sa fenêtre".into()), None), "no badge meanwhile");
        line(&mut e, "reprise", now);
        assert_eq!(view(&e).badge.map(|b| b.color), Some(BadgeColor::Amber), "back to the active line");
        line(&mut e, "pastille", now);
        assert_eq!(view(&e).badge, None);
        line(&mut e, "alerte La fenêtre visée est en plein écran. | Passe-la en fenêtre.", now);
        assert_eq!(view(&e).alert.map(|a| a.title), Some("La fenêtre visée est en plein écran.".into()));
        // A line of an older run is ignored.
        let late = Event::Script { run: 999, output: ScriptOutput::Line("etat 99/100".into()) };
        e.event(late, now);
        assert_eq!(view(&e).line, ExternalLine::Active(Some("12/100".into())));
    }

    #[test]
    fn a_new_setting_goes_to_the_script_at_once_on_its_steps() {
        let mut e = engine();
        let now = Instant::now();
        e.command(Command::SetSetting(EXAMPLE, "limite".into(), 123), now);
        assert!(calls(&mut e).is_empty(), "stopped: only kept");
        assert_eq!(view(&e).values, vec![120]);
        assert_eq!(view(&e).line, ExternalLine::Stopped(Some("limite 120 clics".into())));
        f10(&mut e, now);
        assert!(calls(&mut e).contains(&Call::Tell("reglage limite 120".into())));
        e.command(Command::SetSetting(EXAMPLE, "limite".into(), 9999), now);
        assert_eq!(calls(&mut e), vec![Call::Tell("reglage limite 500".into())]);
        e.command(Command::SetSetting(EXAMPLE, "limite".into(), 500), now);
        e.command(Command::SetSetting(EXAMPLE, "inconnu".into(), 3), now);
        assert!(calls(&mut e).is_empty(), "same value, unknown key: nothing");
    }

    #[test]
    fn the_script_pauses_while_the_mouse_is_on_the_hud() {
        let mut e = engine_with(Fake { pointer_on_hud: true, ..Fake::default() });
        let now = Instant::now();
        // Started from the HUD, the mouse on it: at once, but paused.
        e.command(Command::Start(EXAMPLE), now);
        assert_eq!(calls(&mut e).last(), Some(&Call::Tell("pause".into())));
        assert!(view(&e).suspended);
        e.backend_mut().pointer_on_hud = false;
        e.tick(now + SCRIPT_CHECK);
        assert_eq!(calls(&mut e), vec![Call::Tell("reprise".into())]);
        assert!(!view(&e).suspended);
        e.backend_mut().pointer_on_hud = true;
        e.tick(now + 2 * SCRIPT_CHECK);
        assert_eq!(calls(&mut e), vec![Call::Tell("pause".into())]);
        assert_eq!(e.next_deadline(), Some(now + 3 * SCRIPT_CHECK), "it keeps looking");
    }

    #[test]
    fn a_requested_stop_lets_the_script_finish() {
        let mut e = engine();
        let now = Instant::now();
        f10(&mut e, now);
        calls(&mut e);
        f10(&mut e, now + 100 * MS);
        assert_eq!(calls(&mut e), vec![Call::Tell("arret".into())]);
        assert_eq!(view(&e).status, MacroStatus::Active, "still active while it ends");
        // It quits within the 2 s.
        e.backend_mut().ended = Some(Some(0));
        e.tick(now + 300 * MS);
        closed(&mut e, now + 300 * MS);
        assert_eq!(calls(&mut e), vec![Call::Done, Call::GuardHud(false)]);
        let v = view(&e);
        assert_eq!(
            (v.status, v.line, v.alert),
            (MacroStatus::Stopped, ExternalLine::Stopped(Some("limite 100 clics".into())), None)
        );
        assert_eq!(e.next_deadline(), None);
    }

    /// Asked to stop, or after its `fin`: with code 0 it released what it held; with another end,
    /// its declared buttons still down are released (D-47).
    #[test]
    fn a_stop_or_a_fin_with_an_error_code_releases_its_buttons() {
        for (fin, code, released) in [
            (false, Some(0), false),
            (false, Some(1), true),
            (false, None, true),
            (true, Some(0), false),
            (true, Some(2), true),
        ] {
            let mut e = engine_with(Fake { down: ButtonSet::only(MouseButton::Right), ..Fake::default() });
            let now = Instant::now();
            f10(&mut e, now);
            if fin {
                line(&mut e, "fin 100/100 clics", now);
            } else {
                f10(&mut e, now + MS);
            }
            e.backend_mut().ended = Some(code);
            closed(&mut e, now + 2 * MS);
            let made = calls(&mut e);
            let expected = Call::ReleaseDown(ButtonSet::only(MouseButton::Right));
            assert_eq!(made.contains(&expected), released, "fin {fin}, code {code:?}: {made:?}");
            assert_eq!(e.backend().down.is_empty(), released, "fin {fin}, code {code:?}");
            assert_eq!(view(&e).status, MacroStatus::Stopped);
            assert_eq!(view(&e).alert, None, "no box: it was asked to stop, or said « fin »");
        }
    }

    #[test]
    fn a_script_that_does_not_stop_is_stopped_by_force_after_2_s() {
        let mut e = engine_with(Fake { down: ButtonSet::only(MouseButton::Right), ..Fake::default() });
        let now = Instant::now();
        f10(&mut e, now);
        f10(&mut e, now + 10 * MS);
        calls(&mut e);
        e.tick(now + 10 * MS + SCRIPT_STOP_WAIT - MS);
        assert!(!calls(&mut e).contains(&Call::Kill), "not before 2 s");
        e.tick(now + 10 * MS + SCRIPT_STOP_WAIT);
        assert_eq!(
            calls(&mut e),
            vec![Call::Kill, Call::ReleaseDown(ButtonSet::only(MouseButton::Right)), Call::Done, Call::GuardHud(false)]
        );
        assert!(e.backend().down.is_empty(), "its declared button released");
        assert_eq!(view(&e).status, MacroStatus::Stopped);
    }

    #[test]
    fn a_script_that_ends_with_fin_shows_finished_until_the_next_start() {
        let mut e = engine();
        let now = Instant::now();
        f10(&mut e, now);
        line(&mut e, "fin 100/100 clics", now);
        e.backend_mut().ended = Some(Some(0));
        closed(&mut e, now + MS);
        let v = view(&e);
        assert_eq!(
            (v.status, v.line, v.alert),
            (MacroStatus::Stopped, ExternalLine::Finished("100/100 clics".into()), None)
        );
        assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::ReleaseDown(_))), "it ended well: nothing released");
        e.backend_mut().ended = None;
        f10(&mut e, now + 2 * MS);
        assert_eq!(view(&e).line, ExternalLine::Active(None), "a new start clears it");
    }

    #[test]
    fn a_script_that_ends_badly_says_so_and_its_buttons_are_released() {
        let mut e = engine_with(Fake { down: ButtonSet::only(MouseButton::Right), ..Fake::default() });
        let now = Instant::now();
        f10(&mut e, now);
        calls(&mut e);
        e.backend_mut().ended = Some(Some(3));
        e.tick(now + SCRIPT_CHECK);
        assert_eq!(view(&e).status, MacroStatus::Active, "its last lines may still come");
        e.tick(now + SCRIPT_CHECK + OUTPUT_GRACE);
        assert!(calls(&mut e).contains(&Call::ReleaseDown(ButtonSet::only(MouseButton::Right))));
        let v = view(&e);
        assert_eq!(v.status, MacroStatus::Stopped);
        assert_eq!(v.line, ExternalLine::Stopped(Some("limite 100 clics".into())));
        assert_eq!(
            v.alert,
            Some(Alert { title: "« Exemple » s'est arrêtée seule.".into(), text: "Code de sortie : 3.".into() })
        );
    }

    /// Its own alert, then an end without `fin`: its box alone (`docs/MACROS-EXTERNES.md`, mock-up
    /// view 7), whatever its exit code; its buttons released all the same.
    #[test]
    fn an_alert_then_an_end_keeps_its_box_alone() {
        for code in [Some(0), Some(1), None] {
            let mut e = engine_with(Fake { down: ButtonSet::only(MouseButton::Right), ..Fake::default() });
            let now = Instant::now();
            f10(&mut e, now);
            line(&mut e, "alerte La fenêtre visée est en plein écran. | Passe-la en fenêtre.", now);
            e.backend_mut().ended = Some(code);
            closed(&mut e, now + MS);
            assert!(calls(&mut e).contains(&Call::ReleaseDown(ButtonSet::only(MouseButton::Right))), "{code:?}");
            let v = view(&e);
            assert_eq!(v.line, ExternalLine::Stopped(Some("limite 100 clics".into())), "{code:?}");
            assert_eq!(v.alert.map(|a| a.title), Some("La fenêtre visée est en plein écran.".into()), "{code:?}");
        }
    }

    #[test]
    fn a_missing_script_or_a_refused_start_cannot_start() {
        let mut e = engine();
        let now = Instant::now();
        e.external_mut(2).declaration.arguments = vec!["-File".into(), "introuvable.ps1".into()];
        f10(&mut e, now);
        assert!(!calls(&mut e).iter().any(|c| matches!(c, Call::Launch(_))));
        let v = view(&e);
        assert_eq!((v.status, v.line), (MacroStatus::Stopped, ExternalLine::Stopped(None)));
        assert_eq!(
            v.alert,
            Some(Alert {
                title: "« Exemple » ne peut pas démarrer.".into(),
                text: "Son script est introuvable sur ce PC. Les autres macros marchent.".into()
            })
        );
        // The command itself refused.
        e.external_mut(2).declaration.arguments.clear();
        e.backend_mut().refuse_launch = Some("« outil.exe » est introuvable sur ce PC.".into());
        f10(&mut e, now);
        assert_eq!(calls(&mut e), vec![Call::GuardHud(true), Call::GuardHud(false)]);
        assert_eq!(
            view(&e).alert.map(|a| a.text),
            Some("« outil.exe » est introuvable sur ce PC. Les autres macros marchent.".into())
        );
        // The other macros still work.
        e.backend_mut().refuse_launch = None;
        let f8 = e.slots[0].registered.unwrap();
        e.event(Event::Hotkey(f8), now);
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Active);
    }

    #[test]
    fn one_macro_at_a_time_waits_for_the_script_to_end() {
        let mut e = engine();
        let now = Instant::now();
        f10(&mut e, now);
        calls(&mut e);
        // F8 during the script: `arret` first; the hold waits for the script to end.
        let f8 = e.slots[0].registered.unwrap();
        e.event(Event::Hotkey(f8), now + 10 * MS);
        assert_eq!(calls(&mut e), vec![Call::Tell("arret".into())]);
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Stopped);
        e.backend_mut().ended = Some(Some(0));
        e.tick(now + 60 * MS);
        closed(&mut e, now + 60 * MS);
        let made = calls(&mut e);
        assert_eq!(made[..2], [Call::Done, Call::GuardHud(false)]);
        assert!(made.contains(&Call::Press(MouseButton::Left)), "{made:?}");
        assert_eq!(e.snapshot().macros[0].status, MacroStatus::Active);
        // F10 during the hold: released first, then the script starts at once.
        e.backend_mut().ended = None;
        f10(&mut e, now + Duration::from_secs(1));
        let made = calls(&mut e);
        assert_eq!(made[0], Call::Release(MouseButton::Left));
        assert!(made.contains(&Call::Launch("outil.exe".into())), "{made:?}");
    }

    #[test]
    fn quitting_asks_the_script_to_stop_then_waits_for_it() {
        let mut e = engine();
        let now = Instant::now();
        f10(&mut e, now);
        calls(&mut e);
        let next = e.shutdown(now);
        assert_eq!(calls(&mut e), vec![Call::Tell("arret".into())]);
        let at = next.expect("it waits for the script");
        assert!(at <= now + SCRIPT_STOP_WAIT);
        // Nothing starts meanwhile.
        f10(&mut e, now + MS);
        assert!(calls(&mut e).is_empty());
        e.backend_mut().ended = Some(Some(0));
        closed(&mut e, now + 20 * MS);
        let next = e.shutdown(now + 20 * MS);
        assert_eq!(next, None);
        let made = calls(&mut e);
        assert!(
            made.contains(&Call::GuardHud(false)) && made.iter().any(|c| matches!(c, Call::Unregister(_))),
            "{made:?}"
        );
        // A script that never quits: stopped by force at the deadline.
        let mut e = engine();
        f10(&mut e, now);
        let mut next = e.shutdown(now);
        while let Some(at) = next {
            next = e.shutdown(at);
        }
        assert!(calls(&mut e).contains(&Call::Kill));
    }

    #[test]
    fn without_hotkey_it_starts_from_the_hud_and_gets_one_when_chosen() {
        let mut declaration = declaration();
        declaration.arguments.clear();
        declaration.hotkey = None;
        let externals = vec![ExternalSetup { hotkey: None, values: declaration.defaults(), declaration }];
        let mut e = Engine::new(Fake::default(), &[setup(HOLD_CLICK)], externals);
        e.start();
        assert!(e.slots[1].registered.is_none());
        assert_eq!((e.snapshot().externals[0].hotkey, e.snapshot().externals[0].hotkey_label.as_str()), (None, ""));
        let now = Instant::now();
        e.command(Command::StartCapture(EXAMPLE), now);
        let f11 = crate::hotkey::Hotkey::new(Modifiers::NONE, vk::F11);
        e.event(Event::Captured { outcome: crate::capture::Outcome::Chosen(f11), source: KeySource::GlobalHook }, now);
        assert_eq!(e.snapshot().externals[0].hotkey, Some(f11));
        assert_eq!(e.snapshot().notice.unwrap().message.unwrap().text, "Nouveau raccourci de « Exemple » : F11.");
    }
}
