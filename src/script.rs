//! The processes of the external macros' scripts (D-36, `docs/MACROS-EXTERNES.md`): started without
//! a console window, in their working folder, their input and output in pipes. Their output is read
//! line by line on threads of their own (a line is cut, invalid UTF-8 is replaced: nothing a script
//! writes can stop MacroHUD); what MacroHUD tells them goes through a thread too, so the engine never
//! waits for a script. On Windows each script is put in a Job Object that ends it, and everything it
//! started, when its run is over and when MacroHUD ends, however it ends (`platform::Job`).

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};

use crate::engine::{Launch, ScriptOutput};
use crate::external;
use crate::platform::Job;

/// The longest line read from a script, in bytes; the rest of the line is skipped (the engine keeps
/// [`external::LINE_LIMIT`] characters of it).
const LINE_BYTES: usize = 4096;

/// The tests that start real scripts run one at a time: on Windows, what the crash handler ends is
/// one Job Object for the whole process.
#[cfg(test)]
pub static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Where the output of a script goes: its run number and a line, or the end.
pub type Report = Arc<dyn Fn(u64, ScriptOutput) + Send + Sync>;

/// The script running, if any (one at a time, D-20).
#[derive(Default)]
pub struct Scripts {
    next_run: u64,
    current: Option<Running>,
}

struct Running {
    run: u64,
    child: Child,
    /// To the thread that writes its input; dropped: its input closes.
    input: Option<Sender<String>>,
    /// Dropped: everything still in it ends.
    job: Option<Job>,
    ended: Option<Option<i32>>,
}

/// Why `command` could not start, in French.
fn start_error(command: &str, e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => format!("« {command} » est introuvable sur ce PC."),
        std::io::ErrorKind::PermissionDenied => format!("Windows refuse de lancer « {command} »."),
        _ => format!("« {command} » ne démarre pas ({e})."),
    }
}

/// Reads one line into `line` (at most [`LINE_BYTES`] of it, the end of line left out). `false` at
/// the end of the output.
fn read_line(reader: &mut impl BufRead, line: &mut Vec<u8>) -> std::io::Result<bool> {
    line.clear();
    let mut any = false;
    loop {
        let buffer = match reader.fill_buf() {
            Ok(buffer) => buffer,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if buffer.is_empty() {
            return Ok(any);
        }
        any = true;
        let (taken, done) = match buffer.iter().position(|b| *b == b'\n') {
            Some(at) => (at + 1, true),
            None => (buffer.len(), false),
        };
        let room = LINE_BYTES.saturating_sub(line.len());
        line.extend_from_slice(&buffer[..taken.min(room)]);
        reader.consume(taken);
        if done {
            return Ok(true);
        }
    }
}

/// Reads `source` line by line until its end, giving each line to `each`.
fn read_lines(source: impl Read, mut each: impl FnMut(String)) {
    let mut reader = BufReader::new(source);
    let mut line = Vec::new();
    while let Ok(true) = read_line(&mut reader, &mut line) {
        each(external::read_line(&line));
    }
}

impl Scripts {
    /// Starts a script; its output lines, then its end, go to `report` with the run number
    /// returned. `Err`: why it cannot start, in French.
    pub fn launch(&mut self, launch: &Launch, report: Report) -> Result<u64, String> {
        // One script at a time: one still there (it should not be) goes first.
        if let Some(run) = self.current.as_ref().map(|c| c.run) {
            self.kill(run);
            self.done(run);
        }
        let mut command = Command::new(&launch.command);
        command.args(&launch.arguments).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(folder) = &launch.folder {
            command.current_dir(folder);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            /// No console window for the script (`CREATE_NO_WINDOW`).
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|e| start_error(&launch.command, &e))?;
        // Before anything else: the script ends with MacroHUD, however MacroHUD ends. (It may have
        // started a process before this: std cannot start it suspended, but an interpreter takes far
        // longer than this to run its first line.)
        let job = match Job::for_child(&child, launch.buttons) {
            Ok(job) => Some(job),
            Err(e) => {
                crate::log::write(format!("« {} » : script hors d'un Job Object ({e}).", launch.name));
                None
            }
        };
        self.next_run += 1;
        let run = self.next_run;
        if let Some(stdout) = child.stdout.take() {
            let to_engine = report.clone();
            let spawned = std::thread::Builder::new().name("macrohud-script-sortie".into()).spawn(move || {
                read_lines(stdout, |line| to_engine(run, ScriptOutput::Line(line)));
                to_engine(run, ScriptOutput::Closed);
            });
            if spawned.is_err() {
                // Its output will not be read: the engine waits for the end of the process alone.
                report(run, ScriptOutput::Closed);
            }
        }
        if let Some(stderr) = child.stderr.take() {
            let name = launch.name.clone();
            let _ = std::thread::Builder::new().name("macrohud-script-erreurs".into()).spawn(move || {
                read_lines(stderr, |line| crate::log::write(format!("« {name} » (erreur) : {line}")));
            });
        }
        let input = child.stdin.take().and_then(|mut stdin| {
            let (sender, lines) = mpsc::channel::<String>();
            let spawned = std::thread::Builder::new().name("macrohud-script-entree".into()).spawn(move || {
                // Ends when MacroHUD drops its side (the script's input closes) or the script is gone.
                for line in lines {
                    if stdin.write_all(format!("{line}\n").as_bytes()).and_then(|()| stdin.flush()).is_err() {
                        break;
                    }
                }
            });
            spawned.ok().map(|_| sender)
        });
        self.current = Some(Running { run, child, input, job, ended: None });
        Ok(run)
    }

    fn running(&mut self, run: u64) -> Option<&mut Running> {
        self.current.as_mut().filter(|c| c.run == run)
    }

    /// Writes a line to the input of the script of `run`.
    pub fn tell(&mut self, run: u64, line: &str) -> Result<(), String> {
        let running = self.running(run).ok_or("script arrêté")?;
        let input = running.input.as_ref().ok_or("son entrée est fermée")?;
        input.send(line.to_string()).map_err(|_| "son entrée est fermée".to_string())
    }

    /// Whether the script of `run` has ended, with its exit code.
    pub fn ended(&mut self, run: u64) -> Option<Option<i32>> {
        let running = self.running(run)?;
        if running.ended.is_none() {
            running.ended = match running.child.try_wait() {
                Ok(Some(status)) => Some(status.code()),
                Ok(None) => None,
                Err(_) => Some(None),
            };
            if running.ended.is_some() {
                // Nobody reads its input any more.
                running.input = None;
            }
        }
        running.ended
    }

    /// Ends the script of `run` by force, with everything it started (without waiting: its end
    /// shows in [`Scripts::ended`]).
    pub fn kill(&mut self, run: u64) {
        let Some(running) = self.running(run) else { return };
        running.input = None;
        if let Some(job) = &running.job {
            job.terminate();
        }
        // (Outside Windows, the only way; on Windows, in case it is not in a Job Object.)
        let _ = running.child.kill();
    }

    /// The run of `run` is over: whatever the script left running ends too, and it is forgotten.
    pub fn done(&mut self, run: u64) {
        let Some(mut running) = self.current.take_if(|c| c.run == run) else { return };
        if let Some(job) = &running.job {
            job.terminate();
        }
        if running.ended.is_none() {
            let _ = running.child.kill();
        }
        // Collected if it has ended, so nothing of it stays (dropping the job closes it).
        let _ = running.child.try_wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_lines_are_cut_and_ends_of_line_left_out() {
        let text = [b"etat 1\r\n".as_slice(), &[b'x'; 10_000], b"\nfin\n", b"sans fin de ligne"].concat();
        let mut lines = Vec::new();
        read_lines(text.as_slice(), |line| lines.push(line));
        assert_eq!(lines.len(), 4, "{lines:?}");
        assert_eq!(lines[0], "etat 1");
        assert_eq!(lines[1].chars().count(), external::LINE_LIMIT);
        assert_eq!((lines[2].as_str(), lines[3].as_str()), ("fin", "sans fin de ligne"));
    }

    #[test]
    fn a_command_that_cannot_start_says_why() {
        let mut scripts = Scripts::default();
        let launch = Launch {
            name: "Exemple".into(),
            command: "macrohud-commande-introuvable".into(),
            arguments: Vec::new(),
            folder: None,
            buttons: crate::guard::ButtonSet::EMPTY,
        };
        let report: Report = Arc::new(|_, _| {});
        assert_eq!(
            scripts.launch(&launch, report),
            Err("« macrohud-commande-introuvable » est introuvable sur ce PC.".into())
        );
    }

    /// The pipes, the threads and the end, with a shell script (the example script itself, in
    /// PowerShell, is tested in `engine::scripts::example`).
    #[cfg(unix)]
    #[test]
    fn a_script_talks_through_its_pipes_and_ends() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        use std::sync::Mutex;
        use std::time::{Duration, Instant};
        let heard = Arc::new(Mutex::new(Vec::new()));
        let report: Report = {
            let heard = heard.clone();
            Arc::new(move |run, output| heard.lock().unwrap().push((run, output)))
        };
        let mut scripts = Scripts::default();
        let launch = Launch {
            name: "Essai".into(),
            command: "sh".into(),
            arguments: vec![
                "-c".into(),
                "echo 'etat prêt'; read l; echo \"reçu $l\"; printf '\\377\\n'; exit 3".into(),
            ],
            folder: Some(std::env::temp_dir()),
            buttons: crate::guard::ButtonSet::EMPTY,
        };
        let run = scripts.launch(&launch, report).unwrap();
        scripts.tell(run, "reglage limite 100").unwrap();
        let start = Instant::now();
        while scripts.ended(run).is_none() || !heard.lock().unwrap().iter().any(|(_, o)| *o == ScriptOutput::Closed) {
            assert!(start.elapsed() < Duration::from_secs(5), "the script should end");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(scripts.ended(run), Some(Some(3)));
        let heard = heard.lock().unwrap();
        let lines: Vec<String> = heard
            .iter()
            .filter_map(|(r, o)| match o {
                ScriptOutput::Line(line) if *r == run => Some(line.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(lines, ["etat prêt", "reçu reglage limite 100", "\u{FFFD}"]);
        assert!(scripts.tell(run, "arret").is_err(), "its input is closed once it ended");
    }

    #[cfg(unix)]
    #[test]
    fn a_script_that_does_not_quit_is_killed() {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut scripts = Scripts::default();
        let launch = Launch {
            name: "Sourd".into(),
            command: "sh".into(),
            arguments: vec!["-c".into(), "trap '' TERM; while true; do sleep 1; done".into()],
            folder: None,
            buttons: crate::guard::ButtonSet::EMPTY,
        };
        let run = scripts.launch(&launch, Arc::new(|_, _| {})).unwrap();
        assert_eq!(scripts.ended(run), None);
        scripts.kill(run);
        let start = std::time::Instant::now();
        while scripts.ended(run).is_none() {
            assert!(start.elapsed() < std::time::Duration::from_secs(5), "the script should be killed");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        scripts.done(run);
        assert!(scripts.current.is_none());
        assert_eq!(scripts.ended(run), None, "forgotten");
    }
}
