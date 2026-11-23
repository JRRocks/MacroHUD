//! Everything that calls the Windows API. The only module allowed to use `unsafe`.
//!
//! Threads:
//! - the engine thread (« macrohud-moteur ») owns the global hotkeys (`RegisterHotKey` posts
//!   `WM_HOTKEY` to the thread that registered them), runs the [`Engine`] and injects the mouse
//!   buttons with `SendInput`;
//! - the hook thread (« macrohud-crochets ») only runs the low-level mouse and keyboard hooks, which
//!   must answer within a few milliseconds or Windows silently removes them. It installs them only
//!   while needed (a button is held or clicked, the script of an external macro runs, a hotkey is
//!   being captured), so the mouse of a game is not slowed down the rest of the time, and re-installs
//!   the mouse hook every few seconds while it is needed.
//!
//! Every button MacroHUD presses is recorded in [`HELD`] before it is pressed, so that
//! [`emergency_release`] (panic hook, crash filter, exit) can release it from any thread. The script
//! of an external macro runs in a Job Object ([`Job`]): it ends with MacroHUD however MacroHUD ends,
//! and [`emergency_release`] ends it first, then releases its declared buttons.
//!
//! The capture of a new hotkey listens in two ways: the low-level keyboard hook (keys typed in any
//! window), and a keyboard hook on the HUD's own thread (keys typed while the HUD has the focus: the
//! PC test showed the low-level hook did not get those). The first answer wins.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_ALREADY_EXISTS, ERROR_HOTKEY_ALREADY_REGISTERED,
    GetLastError, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, LRESULT, POINT, RECT, SYSTEMTIME, WPARAM,
};
use windows_sys::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWINDOWATTRIBUTE,
    DwmSetWindowAttribute,
};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
    MonitorFromRect, ScreenToClient,
};
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE};
use windows_sys::Win32::System::Diagnostics::Debug::{EXCEPTION_POINTERS, SetUnhandledExceptionFilter};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateMutexW, CreateWaitableTimerExW, GetCurrentProcess, GetCurrentThread,
    GetCurrentThreadId, INFINITE, SetThreadPriority, SetWaitableTimer, THREAD_PRIORITY_ABOVE_NORMAL,
    THREAD_PRIORITY_TIME_CRITICAL, TIMER_ALL_ACCESS,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, HKL, INPUT, INPUT_0, INPUT_MOUSE, MAPVK_VK_TO_CHAR, MAPVK_VK_TO_VSC, MOD_ALT,
    MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
    MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEINPUT, MapVirtualKeyExW, RegisterHotKey,
    SendInput, ToUnicodeEx, UnregisterHotKey, VK_CONTROL, VK_LBUTTON, VK_LCONTROL, VK_LWIN, VK_MBUTTON, VK_MENU,
    VK_RBUTTON, VK_RMENU, VK_RWIN, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GA_ROOT, GUITHREADINFO, GetAncestor, GetClassNameW, GetClientRect, GetCursorPos,
    GetForegroundWindow, GetGUIThreadInfo, GetMessageW, GetWindowRect, GetWindowThreadProcessId, HC_ACTION, HHOOK,
    HOOKPROC, HWND_TOPMOST, IsIconic, IsWindowVisible, KBDLLHOOKSTRUCT, KillTimer, LLKHF_UP, MB_ICONERROR,
    MB_ICONINFORMATION, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, MSG, MSLLHOOKSTRUCT, MWMO_INPUTAVAILABLE, MessageBoxW,
    MsgWaitForMultipleObjectsEx, PM_NOREMOVE, PM_REMOVE, PeekMessageW, PostMessageW, PostThreadMessageW, QS_ALLINPUT,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetTimer, SetWindowPos,
    SetWindowsHookExW, UnhookWindowsHookEx, WH_KEYBOARD, WH_KEYBOARD_LL, WH_MOUSE_LL, WINDOWS_HOOK_ID, WM_APP,
    WM_HOTKEY, WM_QUIT, WM_TIMER, WindowFromPoint,
};

use super::BoxKind;
use crate::capture::{KeyCapture, Outcome};
use crate::clock::Stamp;
use crate::contract::MouseButton;
use crate::engine::{
    Backend, Command, Engine, Event, ExternalSetup, HotkeyId, KeySource, Launch, MacroSetup, RegisterError, Snapshot,
};
use crate::guard::{self, ButtonSet, Edge, MouseGuard, ReleaseScene, Source};
use crate::hotkey::{Hotkey, Modifiers, Vk};
use crate::runner::{EngineHandle, Input, Publisher};
use crate::script::{Report, Scripts};

/// Posted to a thread to make it look at its request queue.
const WM_WAKE: u32 = WM_APP + 1;
/// While the mouse hook is needed, it is installed again this often (Windows removes a hook that
/// was once too slow to answer, without telling anyone).
const HOOK_REFRESH_MS: u32 = 5_000;
/// Mark put on the mouse events MacroHUD injects ("MHUD"), on top of Windows' "injected" flag.
const SIGNATURE: usize = 0x4D48_5544;
/// How long the engine waits for the hook thread to answer.
const HOOK_ANSWER_WAIT: Duration = Duration::from_secs(2);

/// Buttons pressed by MacroHUD and not released yet (bits of [`ButtonSet`]).
static HELD: AtomicU8 = AtomicU8::new(0);
/// For each button (left, right, middle): the window that received its press (0: none known), so
/// that it also gets the release, wherever the pointer is then (P1.8).
static PRESS_WINDOW: [AtomicIsize; 3] = [const { AtomicIsize::new(0) }; 3];
/// The HUD's window, to name it in the run log.
static HUD_WINDOW: AtomicIsize = AtomicIsize::new(0);
/// Set by the panic hook: the hooks then let everything through untouched.
static PANICKING: AtomicBool = AtomicBool::new(false);
/// A handle of the Job Object of the script that runs (0: none), for [`emergency_release`] only: it
/// takes it out (and ends the job) or [`Job`] does when the run is over, never both.
static SCRIPT_JOB: AtomicIsize = AtomicIsize::new(0);
/// The declared buttons of that script, released by [`emergency_release`] if they are down.
static SCRIPT_BUTTONS: AtomicU8 = AtomicU8::new(0);
/// Keyboard events seen during the current capture, by the low-level hook and by the HUD's thread
/// (written in the run log at the end of a capture, to understand it afterwards).
static GLOBAL_KEYS_SEEN: AtomicU32 = AtomicU32::new(0);
static WINDOW_KEYS_SEEN: AtomicU32 = AtomicU32::new(0);

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain(std::iter::once(0)).collect()
}

fn last_error() -> u32 {
    unsafe { GetLastError() }
}

fn post(thread_id: u32, message: u32) {
    unsafe {
        PostThreadMessageW(thread_id, message, 0, 0);
    }
}

/// Makes sure the calling thread has a message queue (before anyone posts to it).
fn ensure_message_queue() {
    let mut msg = MSG::default();
    unsafe {
        PeekMessageW(&mut msg, null_mut(), WM_APP, WM_APP, PM_NOREMOVE);
    }
}

// --- Mouse buttons ---------------------------------------------------------------------------------

fn button_flags(button: MouseButton, down: bool) -> u32 {
    match (button, down) {
        (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
        (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
        (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
        (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
        (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
        (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
    }
}

fn button_vk(button: MouseButton) -> i32 {
    i32::from(match button {
        MouseButton::Left => VK_LBUTTON,
        MouseButton::Right => VK_RBUTTON,
        MouseButton::Middle => VK_MBUTTON,
    })
}

/// Injects a press or a release where the pointer is (no move).
fn inject(button: MouseButton, down: bool) -> Result<(), String> {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: button_flags(button, down),
                time: 0,
                dwExtraInfo: SIGNATURE,
            },
        },
    };
    let sent = unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
    if sent == 1 { Ok(()) } else { Err(format!("SendInput, erreur {}", last_error())) }
}

fn button_index(button: MouseButton) -> usize {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
    }
}

/// Presses the button; `log`: with a line in the run log (not for each click of « Multi-clic »).
fn press(button: MouseButton, log: bool) -> Result<(), String> {
    let bit = ButtonSet::only(button).bits();
    // Recorded before the press: whatever happens next, the emergency release knows about it.
    HELD.fetch_or(bit, Ordering::SeqCst);
    // The press goes to the window under the pointer (it is never the HUD: refused before).
    let target = window_under_pointer();
    PRESS_WINDOW[button_index(button)].store(target, Ordering::SeqCst);
    let result = inject(button, true);
    if result.is_err() {
        HELD.fetch_and(!bit, Ordering::SeqCst);
        PRESS_WINDOW[button_index(button)].store(0, Ordering::SeqCst);
    } else if log {
        crate::log::write(format!("Appui du {} : fenêtre sous le curseur {}.", button_name(button), describe(target)));
    }
    result
}

fn adopt(button: MouseButton) {
    HELD.fetch_or(ButtonSet::only(button).bits(), Ordering::SeqCst);
    // The user's own press went to the window that holds the capture now, or else to the one under
    // the pointer.
    let capture = capture_of(0);
    let target = if capture != 0 { capture } else { window_under_pointer() };
    PRESS_WINDOW[button_index(button)].store(target, Ordering::SeqCst);
    crate::log::write(format!(
        "{} déjà tenu à la main, repris : fenêtre de l'appui {}.",
        capitalized_name(button),
        describe(target)
    ));
}

/// Releases the button, and sends the release to the window of the press when Windows would not
/// bring it there (P1.8). `log`: with a line in the run log. `Ok(true)`: it had to be sent.
fn release(button: MouseButton, log: bool) -> Result<bool, String> {
    let index = button_index(button);
    // Looked at before the release: the application lets the capture go once it gets it.
    let scene = Scene::now(PRESS_WINDOW[index].load(Ordering::SeqCst));
    inject(button, false)?;
    HELD.fetch_and(!ButtonSet::only(button).bits(), Ordering::SeqCst);
    PRESS_WINDOW[index].store(0, Ordering::SeqCst);
    let delivery = scene.deliver(button);
    if log {
        crate::log::write(scene.line(button, &delivery));
    }
    Ok(delivery == Delivery::Sent)
}

/// Releases every button MacroHUD holds, from any thread; returns those it released. The script of
/// an external macro that runs is ended first (with everything it started), then those of its
/// declared buttons that are down are released too.
pub fn emergency_release() -> ButtonSet {
    let job = SCRIPT_JOB.swap(0, Ordering::SeqCst);
    if job != 0 {
        // It can press nothing any more. (Its handle stays open: MacroHUD ends anyway.)
        unsafe { TerminateJobObject(job as HANDLE, 1) };
    }
    let declared = ButtonSet::from_bits(SCRIPT_BUTTONS.swap(0, Ordering::SeqCst));
    let held = ButtonSet::from_bits(HELD.swap(0, Ordering::SeqCst));
    // Here nothing comes before the releases, not even a look at the windows: it follows them, when
    // the application has not had the time to handle its release yet (at worst it would get it
    // twice). The run log only gets the line if writing needs no waiting (panic, crash).
    for button in held.iter() {
        let _ = inject(button, false);
    }
    let mut left_by_script = ButtonSet::EMPTY;
    for button in declared.iter().filter(|b| !held.contains(*b) && key_down(button_vk(*b) as u16)) {
        if inject(button, false).is_ok() {
            left_by_script = left_by_script.with(button);
        }
    }
    for button in held.iter() {
        let scene = Scene::now(PRESS_WINDOW[button_index(button)].swap(0, Ordering::SeqCst));
        let delivery = scene.deliver(button);
        crate::log::write_now(scene.line(button, &delivery));
    }
    if job != 0 {
        let names: Vec<&str> = left_by_script.iter().map(MouseButton::phrase).collect();
        crate::log::write_now(format!(
            "Script de la macro externe arrêté de force ; boutons déclarés relâchés : {}.",
            if names.is_empty() { "aucun".to_string() } else { names.join(", ") }
        ));
    }
    held.union(left_by_script)
}

/// The Job Object of the script of an external macro: the script and every process it starts are
/// in it, and end when it closes, which Windows does when MacroHUD ends, however it ends
/// (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`). Dropped (the run is over): what is left in it ends.
pub struct Job {
    handle: isize,
}

impl Job {
    /// Puts the script just started in a new Job Object; until the job is dropped, a crash ends it
    /// and releases those of `buttons` that are down.
    pub fn for_child(child: &std::process::Child, buttons: ButtonSet) -> Result<Job, String> {
        use std::os::windows::io::AsRawHandle;
        let handle = unsafe { CreateJobObjectW(null(), null()) };
        if handle.is_null() {
            return Err(format!("CreateJobObject, erreur {}", last_error()));
        }
        // Closed by its drop, whatever fails next.
        let job = Job { handle: handle as isize };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        let info = (&raw const limits).cast();
        if unsafe { SetInformationJobObject(handle, JobObjectExtendedLimitInformation, info, size) } == 0 {
            return Err(format!("SetInformationJobObject, erreur {}", last_error()));
        }
        if unsafe { AssignProcessToJobObject(handle, child.as_raw_handle() as HANDLE) } == 0 {
            return Err(format!("AssignProcessToJobObject, erreur {}", last_error()));
        }
        // A handle of its own for the crash handler (a closed handle could be reused meanwhile).
        let mut spare: HANDLE = null_mut();
        let process = unsafe { GetCurrentProcess() };
        if unsafe { DuplicateHandle(process, handle, process, &mut spare, 0, 0, DUPLICATE_SAME_ACCESS) } != 0 {
            SCRIPT_BUTTONS.store(buttons.bits(), Ordering::SeqCst);
            let older = SCRIPT_JOB.swap(spare as isize, Ordering::SeqCst);
            if older != 0 {
                unsafe { CloseHandle(older as HANDLE) };
            }
        }
        Ok(job)
    }

    /// Ends every process in the job.
    pub fn terminate(&self) {
        unsafe { TerminateJobObject(self.handle as HANDLE, 1) };
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // The crash handler has nothing more to end (unless it took its handle already).
        SCRIPT_BUTTONS.store(0, Ordering::SeqCst);
        let spare = SCRIPT_JOB.swap(0, Ordering::SeqCst);
        unsafe {
            if spare != 0 {
                CloseHandle(spare as HANDLE);
            }
            CloseHandle(self.handle as HANDLE);
        }
    }
}

/// Sends `button`'s release message to `window`, at the pointer's place brought back inside it.
fn post_release(window: isize, button: MouseButton) -> Result<(), u32> {
    let hwnd = window as HWND;
    let mut point = POINT::default();
    let mut client = RECT::default();
    unsafe {
        GetCursorPos(&mut point);
        ScreenToClient(hwnd, &mut point);
        GetClientRect(hwnd, &mut client);
    }
    let (x, y) = guard::point_inside(point.x, point.y, client.right, client.bottom);
    // The buttons still down, without the one released (Windows may not have handled it yet).
    let down = MouseButton::ALL
        .into_iter()
        .filter(|b| *b != button && key_down(button_vk(*b) as u16))
        .fold(ButtonSet::EMPTY, ButtonSet::with);
    let flags = guard::key_flags(down, key_down(VK_SHIFT), key_down(VK_CONTROL));
    if unsafe { PostMessageW(hwnd, guard::release_message(button), flags, guard::pack_point(x, y)) } != 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

/// What became of a release, for the window of the press.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Delivery {
    /// No press window known.
    Unknown,
    /// Windows brings the release there by itself.
    Reaches,
    /// The window closed meanwhile.
    Closed,
    /// Sent to it as well.
    Sent,
    /// Sending it was refused (error code).
    Refused(u32),
}

/// The windows involved in a release, for the decision and for the run log.
struct Scene {
    press: isize,
    press_thread: u32,
    pointer: isize,
    capture: isize,
    press_in_foreground: bool,
    reaches: bool,
}

impl Scene {
    fn now(press: isize) -> Scene {
        let pointer = window_under_pointer();
        let press_thread = thread_of(press);
        let capture = if press_thread == 0 { 0 } else { capture_of(press_thread) };
        let foreground = unsafe { GetForegroundWindow() } as isize;
        let press_in_foreground = press_thread != 0 && thread_of(foreground) == press_thread;
        // (The foreground is only written in the run log: it does not change where Windows sends the
        // release, see `guard::release_reaches`.)
        let reaches = guard::release_reaches(&ReleaseScene {
            press_thread,
            pointer_thread: thread_of(pointer),
            press_thread_captures: capture != 0,
        });
        Scene { press, press_thread, pointer, capture, press_in_foreground, reaches }
    }

    /// Once the button is released: sends the release to the press window when Windows does not
    /// bring it there (P1.8, see [`guard::release_reaches`]).
    fn deliver(&self, button: MouseButton) -> Delivery {
        if self.press == 0 {
            Delivery::Unknown
        } else if self.reaches {
            Delivery::Reaches
        } else if thread_of(self.press) != self.press_thread {
            // Closed meanwhile (its handle may even name another window now).
            Delivery::Closed
        } else {
            match post_release(self.press, button) {
                Ok(()) => Delivery::Sent,
                Err(code) => Delivery::Refused(code),
            }
        }
    }

    /// The run log's line about a release and its delivery.
    fn line(&self, button: MouseButton, delivery: &Delivery) -> String {
        let outcome = match delivery {
            Delivery::Unknown => "fenêtre de l'appui inconnue".to_string(),
            Delivery::Reaches => "le relâchement lui parvient".to_string(),
            Delivery::Closed => "fenêtre de l'appui fermée, rien à lui envoyer".to_string(),
            Delivery::Sent => "relâchement envoyé aussi à la fenêtre de l'appui".to_string(),
            Delivery::Refused(code) => format!("envoi à la fenêtre de l'appui refusé (erreur {code})"),
        };
        format!(
            "Relâchement du {} : sous le curseur {} ; fenêtre de l'appui {}, capture de son fil : {}, au premier plan : {} → {outcome}.",
            button_name(button),
            describe(self.pointer),
            describe(self.press),
            describe(self.capture),
            if self.press_in_foreground { "oui" } else { "non" },
        )
    }
}

/// The window under the pointer (the deepest child), 0 when there is none.
fn window_under_pointer() -> isize {
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return 0;
    }
    unsafe { WindowFromPoint(point) as isize }
}

fn thread_of(window: isize) -> u32 {
    if window == 0 { 0 } else { unsafe { GetWindowThreadProcessId(window as HWND, null_mut()) } }
}

/// The window holding the mouse capture in a thread (0: the foreground thread); 0 when none.
fn capture_of(thread: u32) -> isize {
    let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    if unsafe { GetGUIThreadInfo(thread, &mut info) } == 0 {
        return 0;
    }
    info.hwndCapture as isize
}

/// A window for the run log: handle, class (never its title, which may be private), thread.
fn describe(window: isize) -> String {
    if window == 0 {
        return "aucune".to_string();
    }
    let mut class = [0u16; 128];
    let length = unsafe { GetClassNameW(window as HWND, class.as_mut_ptr(), class.len() as i32) };
    let class = String::from_utf16_lossy(&class[..usize::try_from(length).unwrap_or(0)]);
    let root = unsafe { GetAncestor(window as HWND, GA_ROOT) } as isize;
    let hud = HUD_WINDOW.load(Ordering::Relaxed);
    let mark = if hud != 0 && root == hud { " (le HUD)" } else { "" };
    format!("{window:#x} « {class} »{mark}, fil {}", thread_of(window))
}

fn button_name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "bouton gauche",
        MouseButton::Right => "bouton droit",
        MouseButton::Middle => "bouton du milieu",
    }
}

fn capitalized_name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "Bouton gauche",
        MouseButton::Right => "Bouton droit",
        MouseButton::Middle => "Bouton du milieu",
    }
}

/// Called by the panic hook: the hooks stop interfering with the mouse and keyboard.
pub fn set_panicking() {
    PANICKING.store(true, Ordering::SeqCst);
}

unsafe extern "system" fn on_native_crash(_info: *const EXCEPTION_POINTERS) -> i32 {
    // A crash Rust cannot catch (in a driver, for instance): release the button, then let Windows
    // end the process as usual.
    emergency_release();
    0 // EXCEPTION_CONTINUE_SEARCH
}

/// Releases the held button even on a crash outside Rust's panics (access violation...).
pub fn install_native_crash_handler() {
    unsafe {
        SetUnhandledExceptionFilter(Some(on_native_crash));
    }
}

// --- Keyboard layout --------------------------------------------------------------------------------

/// The keyboard layout of the window the user types in.
fn current_layout() -> HKL {
    unsafe {
        let foreground = GetForegroundWindow();
        let thread = if foreground.is_null() { 0 } else { GetWindowThreadProcessId(foreground, null_mut()) };
        GetKeyboardLayout(thread)
    }
}

fn layout_char(key: Vk) -> Option<char> {
    let mapped = unsafe { MapVirtualKeyExW(u32::from(key), MAPVK_VK_TO_CHAR, current_layout()) };
    // Dead keys have the top bit set; the character is in the low word.
    char::from_u32(mapped & 0xFFFF).filter(|c| *c != '\0')
}

fn altgr_char(hotkey: Hotkey) -> Option<char> {
    let m = hotkey.modifiers;
    if !(m.ctrl && m.alt) || m.win {
        return None;
    }
    let layout = current_layout();
    let mut state = [0u8; 256];
    for key in [VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RMENU] {
        state[usize::from(key)] = 0x80;
    }
    if m.shift {
        state[usize::from(VK_SHIFT)] = 0x80;
    }
    let key = u32::from(hotkey.key);
    let mut buffer = [0u16; 8];
    let count = unsafe {
        let scan = MapVirtualKeyExW(key, MAPVK_VK_TO_VSC, layout);
        // Flag 4: leave the keyboard state (dead keys) untouched.
        ToUnicodeEx(key, scan, state.as_ptr(), buffer.as_mut_ptr(), buffer.len() as i32, 4, layout)
    };
    // Negative: a dead key, whose character is in the buffer.
    let length = if count < 0 { 1 } else { count as usize };
    String::from_utf16_lossy(&buffer[..length.min(buffer.len())]).chars().find(|c| !c.is_control())
}

fn key_down(key: u16) -> bool {
    (unsafe { GetAsyncKeyState(i32::from(key)) } as u16) & 0x8000 != 0
}

fn held_modifiers() -> Modifiers {
    Modifiers {
        ctrl: key_down(VK_CONTROL),
        alt: key_down(VK_MENU),
        shift: key_down(VK_SHIFT),
        win: key_down(VK_LWIN) || key_down(VK_RWIN),
    }
}

fn hotkey_modifiers(m: Modifiers) -> u32 {
    let mut flags = MOD_NOREPEAT;
    for (on, flag) in [(m.ctrl, MOD_CONTROL), (m.alt, MOD_ALT), (m.shift, MOD_SHIFT), (m.win, MOD_WIN)] {
        if on {
            flags |= flag;
        }
    }
    flags
}

// --- Hook thread ------------------------------------------------------------------------------------

/// Where the hook thread sends what it sees: the engine's queue, then a wake-up message.
#[derive(Clone)]
struct EngineWaker {
    thread_id: u32,
    sender: Sender<Input>,
}

impl EngineWaker {
    fn send(&self, event: Event) {
        if self.sender.send(Input::Event(event)).is_ok() {
            post(self.thread_id, WM_WAKE);
        }
    }
}

enum HookRequest {
    /// Buttons held, buttons clicked (see `MouseGuard::watch`).
    Watch(ButtonSet, ButtonSet, SyncSender<Result<(), String>>),
    /// The script of an external macro runs, or no longer (see `MouseGuard::shield_hud`).
    GuardHud(bool, SyncSender<Result<(), String>>),
    StartCapture(Modifiers, SyncSender<Result<(), String>>),
    StopCapture,
    Quit,
}

/// The state of the hook thread, reached by the hook procedures (which run on that thread).
struct HookState {
    guard: MouseGuard,
    capture: Option<KeyCapture>,
    mouse_hook: HHOOK,
    keyboard_hook: HHOOK,
    engine: EngineWaker,
    thread_id: u32,
    /// The HUD's window: a click on it during a hold stops the hold.
    hud: Option<isize>,
    /// The hooks must be installed or removed: done by the thread's loop, not inside a hook procedure.
    resync_posted: bool,
}

thread_local! {
    static HOOKS: RefCell<Option<HookState>> = const { RefCell::new(None) };
}

fn install_hook(id: WINDOWS_HOOK_ID, procedure: HOOKPROC) -> Result<HHOOK, String> {
    let hook = unsafe { SetWindowsHookExW(id, procedure, GetModuleHandleW(null()), 0) };
    if hook.is_null() { Err(format!("SetWindowsHookEx, erreur {}", last_error())) } else { Ok(hook) }
}

fn remove_hook(hook: &mut HHOOK) {
    if !hook.is_null() {
        unsafe {
            UnhookWindowsHookEx(*hook);
        }
        *hook = null_mut();
    }
}

impl HookState {
    fn sync_mouse(&mut self) -> Result<(), String> {
        if self.guard.needs_hook() {
            if self.mouse_hook.is_null() {
                self.mouse_hook = install_hook(WH_MOUSE_LL, Some(mouse_procedure))?;
            }
        } else {
            remove_hook(&mut self.mouse_hook);
        }
        Ok(())
    }

    fn sync_keyboard(&mut self) -> Result<(), String> {
        if self.capture.as_ref().is_some_and(KeyCapture::needs_events) {
            if self.keyboard_hook.is_null() {
                self.keyboard_hook = install_hook(WH_KEYBOARD_LL, Some(keyboard_procedure))?;
            }
        } else {
            self.capture = None;
            remove_hook(&mut self.keyboard_hook);
        }
        Ok(())
    }

    /// Re-installs the mouse hook (the new one first, so there is no gap).
    fn refresh_mouse(&mut self) {
        if !self.mouse_hook.is_null()
            && let Ok(fresh) = install_hook(WH_MOUSE_LL, Some(mouse_procedure))
        {
            remove_hook(&mut self.mouse_hook);
            self.mouse_hook = fresh;
        }
    }

    /// Asks the thread's loop to install or remove hooks after the current hook procedure returns.
    fn post_resync(&mut self) {
        if !self.resync_posted {
            self.resync_posted = true;
            post(self.thread_id, WM_WAKE);
        }
    }
}

/// Runs `f` on the hook state, unless it is already in use (a hook procedure called re-entrantly).
fn with_hooks<R>(f: impl FnOnce(&mut HookState) -> R) -> Option<R> {
    HOOKS.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        slot.as_mut().map(f)
    })
}

unsafe extern "system" fn mouse_procedure(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && !PANICKING.load(Ordering::Relaxed) {
        // SAFETY: for HC_ACTION, `lparam` points to an MSLLHOOKSTRUCT (WH_MOUSE_LL contract).
        let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        let injected = guard::is_injected(info.flags);
        // MacroHUD's own presses and releases are none of the guard's business.
        let ours = injected && info.dwExtraInfo == SIGNATURE;
        if !ours && let Some((button, edge)) = guard::button_event(wparam as u32) {
            let source = if injected { Source::OtherProgram } else { Source::Hand };
            let swallow = with_hooks(|state| {
                let on_hud = edge == Edge::Down
                    && state.guard.needs_position()
                    && state.hud.is_some_and(|hud| point_in_window(hud, info.pt));
                let verdict = state.guard.on_event(button, edge, source, on_hud, Instant::now());
                if !verdict.stop.is_empty() {
                    state.engine.send(Event::Click { buttons: verdict.stop, on_hud: verdict.on_hud });
                }
                if !state.guard.needs_hook() {
                    state.post_resync();
                }
                verdict.swallow
            });
            if swallow == Some(true) {
                return 1;
            }
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn keyboard_procedure(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && !PANICKING.load(Ordering::Relaxed) {
        // SAFETY: for HC_ACTION, `lparam` points to a KBDLLHOOKSTRUCT (WH_KEYBOARD_LL contract).
        let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        let down = info.flags & LLKHF_UP == 0;
        let key = Vk::try_from(info.vkCode).unwrap_or(0);
        let swallow = with_hooks(|state| {
            let capture = state.capture.as_mut()?;
            GLOBAL_KEYS_SEEN.fetch_add(1, Ordering::Relaxed);
            let decision = capture.on_key(key, down);
            let finished = !capture.needs_events();
            if let Some(outcome) = decision.outcome {
                state.engine.send(Event::Captured { outcome, source: KeySource::GlobalHook });
            }
            if finished {
                state.post_resync();
            }
            Some(decision.swallow)
        })
        .flatten();
        if swallow == Some(true) {
            return 1;
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

/// The engine's handle on the hook thread.
struct HookThread {
    thread_id: u32,
    requests: Sender<HookRequest>,
    thread: Option<JoinHandle<()>>,
}

impl HookThread {
    fn spawn(engine: EngineWaker, hud: Option<isize>) -> Result<HookThread, String> {
        let (requests, receiver) = mpsc::channel();
        let (ready_sender, ready) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("macrohud-crochets".into())
            .spawn(move || hook_main(receiver, ready_sender, engine, hud))
            .map_err(|e| format!("fil des crochets : {e}"))?;
        let thread_id =
            ready.recv_timeout(HOOK_ANSWER_WAIT).map_err(|_| "le fil des crochets n'a pas démarré".to_string())?;
        Ok(HookThread { thread_id, requests, thread: Some(thread) })
    }

    /// Sends a request and waits for its answer.
    fn ask(&self, request: impl FnOnce(SyncSender<Result<(), String>>) -> HookRequest) -> Result<(), String> {
        let (answer, receiver) = mpsc::sync_channel(1);
        self.requests.send(request(answer)).map_err(|_| "le fil des crochets est arrêté".to_string())?;
        post(self.thread_id, WM_WAKE);
        receiver.recv_timeout(HOOK_ANSWER_WAIT).map_err(|_| "le fil des crochets ne répond pas".to_string())?
    }

    fn tell(&self, request: HookRequest) {
        if self.requests.send(request).is_ok() {
            post(self.thread_id, WM_WAKE);
        }
    }
}

impl Drop for HookThread {
    fn drop(&mut self) {
        self.tell(HookRequest::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn hook_main(requests: Receiver<HookRequest>, ready: SyncSender<u32>, engine: EngineWaker, hud: Option<isize>) {
    unsafe {
        // The hooks must answer at once, even when a game keeps every core busy.
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
    }
    ensure_message_queue();
    let thread_id = unsafe { GetCurrentThreadId() };
    HOOKS.with(|cell| {
        *cell.borrow_mut() = Some(HookState {
            guard: MouseGuard::default(),
            capture: None,
            mouse_hook: null_mut(),
            keyboard_hook: null_mut(),
            engine,
            thread_id,
            hud,
            resync_posted: false,
        })
    });
    let _ = ready.send(thread_id);
    let timer = unsafe { SetTimer(null_mut(), 0, HOOK_REFRESH_MS, None) };

    let mut msg = MSG::default();
    'run: loop {
        // Hook procedures run inside GetMessageW.
        if unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) } <= 0 {
            break;
        }
        match msg.message {
            WM_WAKE => loop {
                match requests.try_recv() {
                    Ok(HookRequest::Watch(held, clicked, answer)) => {
                        let result = with_hooks(|s| {
                            s.guard.watch(held, clicked);
                            s.sync_mouse()
                        });
                        let _ = answer.send(result.unwrap_or_else(|| Err("état des crochets occupé".into())));
                    }
                    Ok(HookRequest::GuardHud(on, answer)) => {
                        let result = with_hooks(|s| {
                            s.guard.shield_hud(on);
                            s.sync_mouse()
                        });
                        let _ = answer.send(result.unwrap_or_else(|| Err("état des crochets occupé".into())));
                    }
                    Ok(HookRequest::StartCapture(modifiers, answer)) => {
                        let result = with_hooks(|s| {
                            s.capture = Some(KeyCapture::new(modifiers));
                            let result = s.sync_keyboard();
                            if result.is_err() {
                                s.capture = None;
                            }
                            result
                        });
                        let _ = answer.send(result.unwrap_or_else(|| Err("état des crochets occupé".into())));
                    }
                    Ok(HookRequest::StopCapture) => {
                        with_hooks(|s| {
                            if let Some(capture) = s.capture.as_mut() {
                                capture.finish();
                            }
                            let _ = s.sync_keyboard();
                        });
                    }
                    Ok(HookRequest::Quit) | Err(TryRecvError::Disconnected) => break 'run,
                    Err(TryRecvError::Empty) => {
                        with_hooks(|s| {
                            if s.resync_posted {
                                s.resync_posted = false;
                                let _ = s.sync_mouse();
                                let _ = s.sync_keyboard();
                            }
                        });
                        break;
                    }
                }
            },
            WM_TIMER => {
                with_hooks(|s| {
                    let now = Instant::now();
                    if s.capture.as_ref().is_some_and(|c| c.expired(now)) {
                        s.capture = None;
                    }
                    s.guard.expire(now);
                    s.refresh_mouse();
                    let _ = s.sync_mouse();
                    let _ = s.sync_keyboard();
                });
            }
            _ => {}
        }
    }
    with_hooks(|s| {
        remove_hook(&mut s.mouse_hook);
        remove_hook(&mut s.keyboard_hook);
    });
    unsafe {
        KillTimer(null_mut(), timer);
    }
}

// --- Engine thread ----------------------------------------------------------------------------------

/// The real backend: Windows itself.
struct WindowsBackend {
    hooks: Option<HookThread>,
    hud: Option<isize>,
    next_id: HotkeyId,
    scripts: Scripts,
    /// Where the scripts' output goes: the engine's queue.
    report: Report,
}

impl Backend for WindowsBackend {
    fn watch(&mut self, held: ButtonSet, clicked: ButtonSet) -> Result<(), String> {
        match &self.hooks {
            Some(hooks) => hooks.ask(|answer| HookRequest::Watch(held, clicked, answer)),
            None if held.union(clicked).is_empty() => Ok(()),
            None => Err("le fil des crochets n'a pas démarré".into()),
        }
    }

    fn is_down(&self, button: MouseButton) -> bool {
        (unsafe { GetAsyncKeyState(button_vk(button)) } as u16) & 0x8000 != 0
    }

    fn press(&mut self, button: MouseButton) -> Result<(), String> {
        press(button, true)
    }

    fn adopt(&mut self, button: MouseButton) {
        adopt(button);
    }

    fn release(&mut self, button: MouseButton) -> Result<(), String> {
        release(button, true).map(|_| ())
    }

    fn click_down(&mut self, button: MouseButton) -> Result<(), String> {
        press(button, false)
    }

    fn click_up(&mut self, button: MouseButton) -> Result<bool, String> {
        release(button, false)
    }

    fn pointer_on_hud(&self) -> bool {
        self.hud.is_some_and(|hud| pointer_in_window(hud) || holds_the_mouse(hud))
    }

    fn register_hotkey(&mut self, hotkey: Hotkey) -> Result<HotkeyId, RegisterError> {
        // Application hotkey ids go from 1 to 0xBFFF.
        self.next_id = self.next_id % 0xBFFF + 1;
        let id = self.next_id;
        let flags = hotkey_modifiers(hotkey.modifiers);
        if unsafe { RegisterHotKey(null_mut(), id, flags, u32::from(hotkey.key)) } != 0 {
            return Ok(id);
        }
        match last_error() {
            ERROR_HOTKEY_ALREADY_REGISTERED => Err(RegisterError::Taken),
            code => Err(RegisterError::Other(format!("erreur {code}"))),
        }
    }

    fn unregister_hotkey(&mut self, id: HotkeyId) {
        unsafe {
            UnregisterHotKey(null_mut(), id);
        }
    }

    fn start_capture(&mut self) -> Result<(), String> {
        GLOBAL_KEYS_SEEN.store(0, Ordering::Relaxed);
        let modifiers = held_modifiers();
        match &self.hooks {
            Some(hooks) => hooks.ask(|answer| HookRequest::StartCapture(modifiers, answer)),
            None => Err("le fil des crochets n'a pas démarré".into()),
        }
    }

    fn stop_capture(&mut self) {
        if let Some(hooks) = &self.hooks {
            hooks.tell(HookRequest::StopCapture);
        }
        crate::log::write(format!(
            "Capture terminée : événements clavier vus par le crochet global : {}, par la fenêtre du HUD : {}.",
            GLOBAL_KEYS_SEEN.load(Ordering::Relaxed),
            WINDOW_KEYS_SEEN.load(Ordering::Relaxed)
        ));
    }

    fn altgr_char(&self, hotkey: Hotkey) -> Option<char> {
        altgr_char(hotkey)
    }

    fn layout_char(&self, key: Vk) -> Option<char> {
        layout_char(key)
    }

    fn guard_hud(&mut self, on: bool) -> Result<(), String> {
        match &self.hooks {
            Some(hooks) => hooks.ask(|answer| HookRequest::GuardHud(on, answer)),
            None if !on => Ok(()),
            None => Err("le fil des crochets n'a pas démarré".into()),
        }
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
        let mut released = ButtonSet::EMPTY;
        for button in buttons.iter().filter(|b| self.is_down(*b)) {
            match inject(button, false) {
                Ok(()) => released = released.with(button),
                Err(e) => crate::log::write(format!("{} non relâché ({e}).", capitalized_name(button))),
            }
        }
        released
    }
}

/// Whether the window's thread holds the mouse: a press on it is not released yet (a drag of the
/// HUD's speed slider, or of the window itself), and Windows sends it every mouse message meanwhile,
/// wherever the pointer went (winit captures the mouse at each press).
fn holds_the_mouse(hwnd: isize) -> bool {
    let thread = thread_of(hwnd);
    thread != 0 && capture_of(thread) != 0
}

/// Whether the mouse pointer is over the window.
fn pointer_in_window(hwnd: isize) -> bool {
    let mut point = POINT::default();
    (unsafe { GetCursorPos(&mut point) } != 0) && point_in_window(hwnd, point)
}

/// Whether a point of the screen is over the window (visible, not minimised).
fn point_in_window(hwnd: isize, point: POINT) -> bool {
    let hwnd = hwnd as HWND;
    let mut rect = RECT::default();
    unsafe {
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 || GetWindowRect(hwnd, &mut rect) == 0 {
            return false;
        }
    }
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

/// Starts the engine thread (which starts the hook thread).
pub fn spawn_engine(
    setup: Vec<MacroSetup>,
    externals: Vec<ExternalSetup>,
    hud: Option<isize>,
    repaint: Box<dyn Fn() + Send>,
) -> Result<EngineHandle, String> {
    HUD_WINDOW.store(hud.unwrap_or(0), Ordering::Relaxed);
    let (sender, receiver) = mpsc::channel::<Input>();
    let (ready_sender, ready) = mpsc::sync_channel::<u32>(1);
    let shared = Arc::new(Mutex::new(Snapshot::default()));
    let publisher = Publisher::new(shared.clone(), repaint);
    let for_hooks = sender.clone();
    let thread = std::thread::Builder::new()
        .name("macrohud-moteur".into())
        .spawn(move || engine_main(setup, externals, hud, receiver, for_hooks, ready_sender, publisher))
        .map_err(|e| format!("fil du moteur : {e}"))?;
    let thread_id =
        ready.recv_timeout(Duration::from_secs(5)).map_err(|_| "le fil du moteur n'a pas démarré".to_string())?;
    let wake = Arc::new(move || post(thread_id, WM_WAKE));
    Ok(EngineHandle::new(sender, wake, shared, thread))
}

fn engine_main(
    setup: Vec<MacroSetup>,
    externals: Vec<ExternalSetup>,
    hud: Option<isize>,
    inputs: Receiver<Input>,
    sender: Sender<Input>,
    ready: SyncSender<u32>,
    mut publisher: Publisher,
) {
    ensure_message_queue();
    let thread_id = unsafe { GetCurrentThreadId() };
    let _ = ready.send(thread_id);
    let waker = EngineWaker { thread_id, sender };
    let report: Report = {
        let waker = waker.clone();
        Arc::new(move |run, output| waker.send(Event::Script { run, output }))
    };
    let hooks = match HookThread::spawn(waker, hud) {
        Ok(hooks) => Some(hooks),
        Err(e) => {
            crate::log::write(format!("Surveillance de la souris impossible : {e}"));
            None
        }
    };
    unsafe {
        // The clicks of « Multi-clic » keep their beat even when a game keeps the processor busy.
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
    }
    let timer = PreciseTimer::new();
    crate::log::write(if timer.high_resolution {
        "Minuterie du moteur : haute résolution (à la milliseconde)."
    } else {
        "Minuterie du moteur : ordinaire (ce Windows n'a pas la haute résolution) ; les clics gardent leur battement."
    });
    let backend = WindowsBackend { hooks, hud, next_id: 0, scripts: Scripts::default(), report };
    let mut engine = Engine::new(backend, &setup, externals);
    engine.start();
    publisher.publish(engine.snapshot());

    let mut msg = MSG::default();
    'run: loop {
        timer.wait(engine.next_deadline());
        // A macro switched by its hotkey or by a real click: the user is in another window (a game,
        // maybe always on top itself), so the HUD comes back above it to show the change.
        let mut raise = false;
        while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {
            match msg.message {
                WM_HOTKEY => {
                    crate::crash::crash_test_point(engine.crash_test_buttons());
                    engine.event(Event::Hotkey(msg.wParam as HotkeyId), Instant::now());
                    raise = true;
                }
                WM_QUIT => break 'run,
                _ => {}
            }
        }
        loop {
            match inputs.try_recv() {
                Ok(Input::Command(command)) => {
                    if matches!(command, Command::Stop(_)) {
                        crate::crash::crash_test_point(engine.crash_test_buttons());
                    }
                    engine.command(command, Instant::now());
                }
                Ok(Input::Event(event)) => {
                    raise |= matches!(event, Event::Click { .. });
                    engine.event(event, Instant::now());
                }
                Ok(Input::Shutdown) | Err(TryRecvError::Disconnected) => break 'run,
                Err(TryRecvError::Empty) => break,
            }
        }
        engine.tick(Instant::now());
        publisher.publish(engine.snapshot());
        if raise && let Some(hwnd) = hud {
            raise_window(hwnd);
        }
    }
    // Quitting: the script of an external macro has a moment to stop, its output still read.
    let mut next = engine.shutdown(Instant::now());
    publisher.publish(engine.snapshot());
    while let Some(at) = next {
        timer.wait(Some(at));
        while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {}
        while let Ok(input) = inputs.try_recv() {
            if let Input::Event(event) = input {
                engine.event(event, Instant::now());
            }
        }
        next = engine.shutdown(Instant::now());
        publisher.publish(engine.snapshot());
    }
}

/// Wakes the engine thread at the engine's next deadline, to the millisecond when Windows can (a
/// high-resolution waitable timer, Windows 10 1803 and later): a plain timeout ends on Windows'
/// clock tick, about every 16 ms, too coarse for 15 clicks per second.
struct PreciseTimer {
    handle: HANDLE,
    high_resolution: bool,
}

impl PreciseTimer {
    fn new() -> PreciseTimer {
        let create = |flags| unsafe { CreateWaitableTimerExW(null(), null(), flags, TIMER_ALL_ACCESS) };
        let precise = create(CREATE_WAITABLE_TIMER_HIGH_RESOLUTION);
        if !precise.is_null() {
            return PreciseTimer { handle: precise, high_resolution: true };
        }
        PreciseTimer { handle: create(0), high_resolution: false }
    }

    /// Waits for a message (hotkey, request of the HUD...) or for `deadline`.
    fn wait(&self, deadline: Option<Instant>) {
        let Some(at) = deadline else {
            unsafe { MsgWaitForMultipleObjectsEx(0, null(), INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE) };
            return;
        };
        let left = at.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        // Relative due time, in units of 100 ns (negative), at least one unit.
        let due = -i64::try_from(left.as_nanos() / 100).unwrap_or(i64::MAX).max(1);
        if !self.handle.is_null() && unsafe { SetWaitableTimer(self.handle, &due, 0, None, null(), 0) } != 0 {
            unsafe {
                MsgWaitForMultipleObjectsEx(1, &self.handle, INFINITE, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            }
            return;
        }
        // No timer: a timeout in milliseconds, rounded up (waking early would only mean waking again).
        let timeout = u32::try_from(left.as_millis() + 1).unwrap_or(INFINITE - 1);
        unsafe { MsgWaitForMultipleObjectsEx(0, null(), timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE) };
    }
}

impl Drop for PreciseTimer {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { CloseHandle(self.handle) };
        }
    }
}

// --- HUD window ---------------------------------------------------------------------------------------

/// Distance from the screen's edges of the HUD's default place (top right corner), in pixels.
const DEFAULT_MARGIN: i32 = 24;

/// The HUD's window, for placing it (in screen pixels, whatever the display scaling).
#[derive(Clone, Copy, Debug, Default)]
pub struct HudWindow {
    hwnd: Option<isize>,
}

impl HudWindow {
    pub fn from_creation_context(cc: &eframe::CreationContext<'_>) -> Self {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let hwnd = cc.window_handle().ok().and_then(|handle| match handle.as_raw() {
            RawWindowHandle::Win32(window) => Some(window.hwnd.get()),
            _ => None,
        });
        let window = HudWindow { hwnd };
        window.remove_system_frame(cc);
        window
    }

    /// The HUD draws its own outline and rounded corners: Windows must draw nothing around it.
    /// egui asks winit, for every borderless window, for the system's drop shadow; Windows then
    /// keeps a 1 px band on top of the window and, on Windows 11, draws its 1 px border and rounds
    /// the corners itself, which darkens them (P1.9). All of it goes; the window stays transparent
    /// (another setting). The HUD never sends `ViewportCommand::Decorations`, which would bring the
    /// shadow back.
    fn remove_system_frame(&self, cc: &eframe::CreationContext<'_>) {
        use winit::platform::windows::WindowExtWindows as _;
        let Some(hwnd) = self.hwnd else { return };
        // The shadow is part of winit's own state (it shapes the client area): only winit can drop it.
        // It does at once, the frame being recomputed before this returns.
        let shadow_dropped = cc.winit_window().map(|window| window.set_undecorated_shadow(false)).is_some();
        let border = dwm_set(hwnd, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE);
        let corners = dwm_set(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND as u32);
        crate::log::write(frame_report(shadow_dropped, border, corners, client_covers_window(hwnd)));
    }

    /// Raw handle, for the engine (is the pointer over the HUD?).
    pub fn raw(&self) -> Option<isize> {
        self.hwnd
    }

    fn rect(&self) -> Option<RECT> {
        let hwnd = self.hwnd? as HWND;
        let mut rect = RECT::default();
        (unsafe { GetWindowRect(hwnd, &mut rect) } != 0).then_some(rect)
    }

    /// Top-left corner, in screen pixels.
    pub fn position(&self) -> Option<(i32, i32)> {
        if unsafe { IsIconic(self.hwnd? as HWND) } != 0 {
            return None;
        }
        self.rect().map(|r| (r.left, r.top))
    }

    /// Puts the window at its saved place (or the default one), always inside a screen.
    pub fn place(&self, saved: Option<(i32, i32)>) {
        let (Some(hwnd), Some(rect)) = (self.hwnd, self.rect()) else { return };
        let size = (rect.right - rect.left, rect.bottom - rect.top);
        let target = saved.unwrap_or_else(|| default_position(size));
        let (x, y) = fit_on_screen(target, size);
        move_window(hwnd, x, y);
    }

    /// Starts listening to the keys sent to the HUD's own thread (the thread calling this, which owns
    /// the window): while the HUD has the focus, a new hotkey is captured there. `report` gets the
    /// outcome. Stopped by [`HudWindow::stop_key_capture`].
    pub fn start_key_capture(&self, report: Box<dyn Fn(Outcome)>) {
        remove_window_capture(true);
        let thread = unsafe { GetCurrentThreadId() };
        // A hook on a thread of this process needs no module handle.
        let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD, Some(window_keyboard_procedure), null_mut(), thread) };
        if hook.is_null() {
            crate::log::write(format!("Capture par la fenêtre du HUD impossible : erreur {}.", last_error()));
            return;
        }
        WINDOW_KEYS_SEEN.store(0, Ordering::Relaxed);
        // Looks often whether the keyboard went to another window (Alt+Tab: see `keyboard_lost`).
        let timer = unsafe { SetTimer(null_mut(), 0, KEYBOARD_CHECK_MS, Some(window_capture_timer)) };
        let capture = WindowCapture { capture: KeyCapture::new(held_modifiers()), hook, timer, thread, report };
        WINDOW_CAPTURE.with(|cell| match cell.try_borrow_mut() {
            Ok(mut slot) => *slot = Some(capture),
            Err(_) => {
                let mut capture = capture;
                capture.close();
            }
        });
    }

    /// Stops waiting for a key (releases of keys already swallowed are still swallowed).
    pub fn stop_key_capture(&self) {
        WINDOW_CAPTURE.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut()
                && let Some(window) = slot.as_mut()
            {
                window.capture.finish();
            }
        });
        remove_window_capture(false);
    }

    /// Removes the HUD's keyboard hook once it has nothing left to do (called at every frame).
    pub fn key_capture_tick(&self) {
        check_keyboard_lost();
        remove_window_capture(false);
    }

    /// Moves the window back inside the screen if part of it went out (after a resize, for instance).
    pub fn keep_on_screen(&self) {
        let (Some(hwnd), Some(rect)) = (self.hwnd, self.rect()) else { return };
        let position = (rect.left, rect.top);
        let fitted = fit_on_screen(position, (rect.right - rect.left, rect.bottom - rect.top));
        if fitted != position {
            move_window(hwnd, fitted.0, fitted.1);
        }
    }
}

/// Puts the HUD back on top of the other always-on-top windows, without activating it. Called from
/// the engine thread: the request is posted to the window's thread instead of waiting for it.
fn raise_window(hwnd: isize) {
    let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
    unsafe {
        SetWindowPos(hwnd as HWND, HWND_TOPMOST, 0, 0, 0, 0, flags);
    }
}

/// How often, during a capture on the HUD's thread, MacroHUD looks whether the keyboard went to
/// another window.
const KEYBOARD_CHECK_MS: u32 = 50;

/// A capture listening to the keyboard messages of the HUD's own thread.
struct WindowCapture {
    capture: KeyCapture,
    hook: HHOOK,
    /// The thread timer of [`window_capture_timer`] (0: none).
    timer: usize,
    /// The HUD's thread, the one listening.
    thread: u32,
    report: Box<dyn Fn(Outcome)>,
}

impl WindowCapture {
    fn close(&mut self) {
        remove_hook(&mut self.hook);
        if self.timer != 0 {
            unsafe { KillTimer(null_mut(), self.timer) };
            self.timer = 0;
        }
    }
}

thread_local! {
    static WINDOW_CAPTURE: RefCell<Option<WindowCapture>> = const { RefCell::new(None) };
}

/// Removes the HUD thread's keyboard hook: always (`force`), or once it no longer needs events.
fn remove_window_capture(force: bool) {
    WINDOW_CAPTURE.with(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else { return };
        let now = Instant::now();
        let done = slot.as_ref().is_some_and(|w| force || !w.capture.needs_events() || w.capture.expired(now));
        if done && let Some(mut window) = slot.take() {
            window.close();
        }
    });
}

/// During a capture on the HUD's thread: if the keyboard went to a window of another thread right
/// after an Alt, Windows took Alt+Tab (its window list) before the HUD saw the Tab: the capture
/// answers Alt+Tab, which the engine refuses (passage 3, R25). A foreground window unknown (0, as
/// after a click on the HUD) tells nothing: the HUD may still have the keyboard.
fn check_keyboard_lost() {
    WINDOW_CAPTURE.with(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else { return };
        let Some(window) = slot.as_mut() else { return };
        let foreground = unsafe { GetForegroundWindow() } as isize;
        if foreground == 0 || thread_of(foreground) == window.thread {
            return;
        }
        if let Some(outcome) = window.capture.keyboard_lost() {
            crate::log::write(format!(
                "Capture : le clavier est passé à une autre fenêtre ({}) juste après Alt : Windows a pris la touche suivante, réponse {outcome:?}.",
                describe(foreground)
            ));
            (window.report)(outcome);
        }
    });
}

unsafe extern "system" fn window_capture_timer(_hwnd: HWND, _message: u32, _id: usize, _time: u32) {
    if !PANICKING.load(Ordering::Relaxed) {
        check_keyboard_lost();
        remove_window_capture(false);
    }
}

unsafe extern "system" fn window_keyboard_procedure(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // HC_ACTION only: a message only peeked at (HC_NOREMOVE) comes back later as HC_ACTION.
    if code == HC_ACTION as i32 && !PANICKING.load(Ordering::Relaxed) {
        let (key, down) = crate::capture::key_message(wparam, lparam);
        let swallow = WINDOW_CAPTURE.with(|cell| {
            let mut slot = cell.try_borrow_mut().ok()?;
            let window = slot.as_mut()?;
            WINDOW_KEYS_SEEN.fetch_add(1, Ordering::Relaxed);
            let decision = window.capture.on_key(key, down);
            if let Some(outcome) = decision.outcome {
                (window.report)(outcome);
            }
            Some(decision.swallow)
        });
        if swallow == Some(true) {
            return 1;
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

/// Sets a 32-bit attribute of the window's frame, drawn by the desktop window manager.
fn dwm_set(hwnd: isize, attribute: DWMWINDOWATTRIBUTE, value: u32) -> i32 {
    unsafe {
        DwmSetWindowAttribute(
            hwnd as HWND,
            attribute as u32,
            (&value as *const u32).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    }
}

/// Whether the window's client area (where the HUD draws) covers the whole window: no band left
/// for Windows to draw in. `None` when it cannot be known.
fn client_covers_window(hwnd: isize) -> Option<bool> {
    let hwnd = hwnd as HWND;
    let mut window = RECT::default();
    let mut client = RECT::default();
    let mut origin = POINT::default();
    let known = unsafe {
        GetWindowRect(hwnd, &mut window) != 0
            && GetClientRect(hwnd, &mut client) != 0
            && ClientToScreen(hwnd, &mut origin) != 0
    };
    known.then(|| {
        (origin.x, origin.y) == (window.left, window.top)
            && (client.right, client.bottom) == (window.right - window.left, window.bottom - window.top)
    })
}

/// The run log's line about the frame Windows draws around the HUD: what was asked, what Windows
/// answered (`border`, `corners`: HRESULT), and whether the HUD draws in the whole window.
fn frame_report(shadow_dropped: bool, border: i32, corners: i32, covers: Option<bool>) -> String {
    let answer = |result: i32, done: &str, refused: &str| {
        if result == 0 { done.to_string() } else { format!("{refused} (Windows répond {:#010x})", result as u32) }
    };
    let shadow = if shadow_dropped { "retirée" } else { "laissée (fenêtre de winit introuvable)" };
    let area = match covers {
        Some(true) => "toute la fenêtre",
        Some(false) => "pas toute la fenêtre (une bande reste à Windows)",
        None => "inconnue",
    };
    format!(
        "Fenêtre du HUD : ombre de Windows : {shadow} ; bordure de Windows 11 : {} ; coins arrondis par Windows : {} ; zone dessinée par le HUD : {area}.",
        answer(border, "retirée", "non retirée"),
        answer(corners, "non", "réglage refusé"),
    )
}

fn move_window(hwnd: isize, x: i32, y: i32) {
    unsafe {
        SetWindowPos(hwnd as HWND, null_mut(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

fn work_area(monitor: windows_sys::Win32::Graphics::Gdi::HMONITOR) -> Option<RECT> {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    (unsafe { GetMonitorInfoW(monitor, &mut info) } != 0).then_some(info.rcWork)
}

fn fit_on_screen(position: (i32, i32), size: (i32, i32)) -> (i32, i32) {
    let rect = RECT { left: position.0, top: position.1, right: position.0 + size.0, bottom: position.1 + size.1 };
    let monitor = unsafe { MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST) };
    match work_area(monitor) {
        Some(area) => crate::app::fit_into(position, size, (area.left, area.top, area.right, area.bottom)),
        None => position,
    }
}

fn default_position(size: (i32, i32)) -> (i32, i32) {
    let monitor = unsafe { MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) };
    match work_area(monitor) {
        Some(area) => (area.right - size.0 - DEFAULT_MARGIN, area.top + DEFAULT_MARGIN),
        None => (DEFAULT_MARGIN, DEFAULT_MARGIN),
    }
}

// --- Process ------------------------------------------------------------------------------------------

/// Holds the single-instance mutex while MacroHUD runs.
pub struct SingleInstance(HANDLE);

impl Drop for SingleInstance {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// `None` when MacroHUD already runs in this Windows session.
pub fn single_instance() -> Option<SingleInstance> {
    let name = wide("Local\\MacroHUD.instance-unique");
    let handle = unsafe { CreateMutexW(null(), 0, name.as_ptr()) };
    let error = last_error();
    if handle.is_null() {
        // Cannot tell: better run than refuse to start.
        return Some(SingleInstance(null_mut()));
    }
    if error == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(handle);
        }
        return None;
    }
    Some(SingleInstance(handle))
}

/// Shows a message box and waits until it is closed. It runs on a thread of its own: the caller may
/// be in the middle of the window's event handling (a panic), where a nested message loop is unsafe.
pub fn message_box(title: &str, text: &str, kind: BoxKind) {
    let title = wide(title);
    let text = wide(text);
    let icon = match kind {
        BoxKind::Info => MB_ICONINFORMATION,
        BoxKind::Error => MB_ICONERROR,
    };
    let shown = std::thread::Builder::new().name("macrohud-message".into()).spawn(move || unsafe {
        MessageBoxW(null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | icon | MB_TOPMOST | MB_SETFOREGROUND);
    });
    if let Ok(thread) = shown {
        let _ = thread.join();
    }
}

/// Lets `macrohud --version` print in the terminal it was typed in (MacroHUD has no console of its own).
pub fn attach_console() {
    let output = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if output.is_null() || output == INVALID_HANDLE_VALUE {
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

/// `%APPDATA%\MacroHUD`.
pub fn settings_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join("AppData").join("Roaming")))
        .unwrap_or_else(std::env::temp_dir)
        .join("MacroHUD")
}

/// Local date and time.
pub fn local_time() -> Option<Stamp> {
    let mut time = SYSTEMTIME::default();
    unsafe {
        GetLocalTime(&mut time);
    }
    Some(Stamp {
        year: time.wYear.into(),
        month: time.wMonth.into(),
        day: time.wDay.into(),
        hour: time.wHour.into(),
        minute: time.wMinute.into(),
        second: time.wSecond.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotkey::{Modifier, vk};

    /// The constants of `guard` (tested on Linux) are the real Windows ones.
    #[test]
    fn hook_constants_match_windows() {
        use windows_sys::Win32::UI::WindowsAndMessaging as w;
        assert_eq!(guard::msg::WM_LBUTTONDOWN, w::WM_LBUTTONDOWN);
        assert_eq!(guard::msg::WM_LBUTTONUP, w::WM_LBUTTONUP);
        assert_eq!(guard::msg::WM_RBUTTONDOWN, w::WM_RBUTTONDOWN);
        assert_eq!(guard::msg::WM_RBUTTONUP, w::WM_RBUTTONUP);
        assert_eq!(guard::msg::WM_MBUTTONDOWN, w::WM_MBUTTONDOWN);
        assert_eq!(guard::msg::WM_MBUTTONUP, w::WM_MBUTTONUP);
        assert_eq!(guard::msg::LLMHF_INJECTED, w::LLMHF_INJECTED);
        assert_eq!(guard::msg::LLMHF_LOWER_IL_INJECTED, w::LLMHF_LOWER_IL_INJECTED);
    }

    /// The windows looked at during a release can be named, and a missing one says so.
    #[test]
    fn release_scenes_can_be_described() {
        assert_eq!(describe(0), "aucune");
        let scene = Scene::now(0);
        assert!(scene.reaches, "no press window: nothing to send");
        let delivery = scene.deliver(MouseButton::Right);
        assert_eq!(delivery, Delivery::Unknown);
        let line = scene.line(MouseButton::Right, &delivery);
        assert!(line.starts_with("Relâchement du bouton droit : sous le curseur "), "{line}");
        assert!(line.ends_with("fenêtre de l'appui inconnue."), "{line}");
    }

    /// The key codes of `hotkey` (tested on Linux) are the real Windows ones.
    #[test]
    fn key_constants_match_windows() {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse as k;
        assert_eq!(vk::F8, k::VK_F8);
        assert_eq!(vk::F24, k::VK_F24);
        assert_eq!(vk::SCROLL, k::VK_SCROLL);
        assert_eq!(vk::ESCAPE, k::VK_ESCAPE);
        assert_eq!(vk::LCONTROL, k::VK_LCONTROL);
        assert_eq!(vk::RMENU, k::VK_RMENU);
        assert_eq!(vk::LWIN, k::VK_LWIN);
        assert_eq!(vk::OEM_102, k::VK_OEM_102);
        assert_eq!(vk::PACKET, k::VK_PACKET);
        assert_eq!(vk::MEDIA_PLAY_PAUSE, k::VK_MEDIA_PLAY_PAUSE);
    }

    /// A hotkey registered twice is refused the second time, as when Windows or another program has it.
    #[test]
    fn a_hotkey_already_registered_is_reported_as_taken() {
        let hotkey =
            Hotkey::new(Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Alt).with(Modifier::Shift), vk::F24);
        let backend = |next_id| WindowsBackend {
            hooks: None,
            hud: None,
            next_id,
            scripts: Scripts::default(),
            report: Arc::new(|_, _| {}),
        };
        let (mut first, mut second) = (backend(100), backend(200));
        let id = first.register_hotkey(hotkey).expect("a rare combination is free");
        assert_eq!(second.register_hotkey(hotkey), Err(RegisterError::Taken));
        first.unregister_hotkey(id);
        let again = second.register_hotkey(hotkey).expect("free again");
        second.unregister_hotkey(again);
    }

    #[test]
    fn the_hook_thread_installs_and_removes_the_hooks() {
        let (sender, _events) = mpsc::channel();
        let waker = EngineWaker { thread_id: unsafe { GetCurrentThreadId() }, sender };
        let hooks = HookThread::spawn(waker, None).expect("hook thread");
        let middle = ButtonSet::only(MouseButton::Middle);
        hooks.ask(|a| HookRequest::Watch(middle, ButtonSet::EMPTY, a)).expect("mouse hook, held button");
        hooks.ask(|a| HookRequest::Watch(ButtonSet::EMPTY, middle, a)).expect("mouse hook, clicked button");
        hooks.ask(|a| HookRequest::Watch(ButtonSet::EMPTY, ButtonSet::EMPTY, a)).expect("no mouse hook");
        hooks.ask(|a| HookRequest::GuardHud(true, a)).expect("mouse hook, a script runs");
        hooks.ask(|a| HookRequest::GuardHud(false, a)).expect("no mouse hook again");
        hooks.ask(|a| HookRequest::StartCapture(Modifiers::NONE, a)).expect("keyboard hook");
        hooks.tell(HookRequest::StopCapture);
        drop(hooks);
    }

    /// How many processes are in the job.
    fn processes_in(job: &Job) -> u32 {
        use windows_sys::Win32::System::JobObjects::{
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JobObjectBasicAccountingInformation, QueryInformationJobObject,
        };
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let size = std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32;
        let asked = unsafe {
            QueryInformationJobObject(
                job.handle as HANDLE,
                JobObjectBasicAccountingInformation,
                (&raw mut info).cast(),
                size,
                null_mut(),
            )
        };
        assert_ne!(asked, 0, "QueryInformationJobObject, erreur {}", last_error());
        info.ActiveProcesses
    }

    fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < Duration::from_secs(10), "{what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// A script in its Job Object, and what it starts: they end together, by the engine (`terminate`)
    /// or by the crash handler (`emergency_release`), which forgets the job once it is dropped.
    #[test]
    fn a_script_ends_in_its_job_with_everything_it_started() {
        let _one = crate::script::ONE_AT_A_TIME.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // `cmd` starts `ping`, which runs for a while: two processes in the job (at least).
        let start = || {
            std::process::Command::new("cmd")
                .args(["/c", "ping -n 30 127.0.0.1 >nul"])
                .stdout(std::process::Stdio::null())
                .spawn()
                .expect("cmd")
        };
        let mut child = start();
        let job = Job::for_child(&child, ButtonSet::EMPTY).expect("job");
        assert_ne!(SCRIPT_JOB.load(Ordering::SeqCst), 0, "known to the crash handler");
        wait_until("ping should start", || processes_in(&job) >= 2);
        job.terminate();
        wait_until("both should end", || processes_in(&job) == 0);
        assert!(child.wait().is_ok());
        drop(job);
        assert_eq!(SCRIPT_JOB.load(Ordering::SeqCst), 0, "forgotten once dropped");

        let mut child = start();
        let job = Job::for_child(&child, ButtonSet::EMPTY).expect("job");
        wait_until("ping should start", || processes_in(&job) >= 2);
        assert!(emergency_release().is_empty(), "no button was down");
        wait_until("the crash handler ends both", || processes_in(&job) == 0);
        assert!(child.wait().is_ok());
        assert_eq!(SCRIPT_JOB.load(Ordering::SeqCst), 0);
        drop(job);
    }

    /// The run log says what Windows accepted about the HUD's frame (Windows 10 has no border nor
    /// rounded corners to remove: it refuses those two).
    #[test]
    fn the_frame_report_says_what_windows_accepted() {
        assert_eq!(
            frame_report(true, 0, 0, Some(true)),
            "Fenêtre du HUD : ombre de Windows : retirée ; bordure de Windows 11 : retirée ; coins arrondis \
             par Windows : non ; zone dessinée par le HUD : toute la fenêtre."
        );
        let e_invalidarg = 0x8007_0057_u32 as i32;
        let refused = frame_report(true, e_invalidarg, e_invalidarg, Some(false));
        assert!(refused.contains("bordure de Windows 11 : non retirée (Windows répond 0x80070057)"), "{refused}");
        assert!(refused.contains("coins arrondis par Windows : réglage refusé (Windows répond 0x80070057)"));
        assert!(refused.contains("pas toute la fenêtre (une bande reste à Windows)"), "{refused}");
        assert!(frame_report(false, 0, 0, None).contains("ombre de Windows : laissée (fenêtre de winit introuvable)"));
    }

    /// The HUD thread's keyboard hook installs and goes away (on the test's own thread).
    #[test]
    fn the_hud_thread_keyboard_hook_installs_and_goes_away() {
        let window = HudWindow::default();
        window.start_key_capture(Box::new(|_| {}));
        assert!(WINDOW_CAPTURE.with(|cell| cell.borrow().is_some()), "hook installed");
        let timer = WINDOW_CAPTURE.with(|cell| cell.borrow().as_ref().map_or(0, |w| w.timer));
        assert_ne!(timer, 0, "keyboard check timer set");
        // No Alt seen: whatever the foreground, the capture goes on.
        window.key_capture_tick();
        assert!(WINDOW_CAPTURE.with(|cell| cell.borrow().as_ref().is_some_and(|w| w.capture.needs_events())));
        window.stop_key_capture();
        assert!(WINDOW_CAPTURE.with(|cell| cell.borrow().is_none()), "hook and timer removed");
    }

    #[test]
    fn modifier_flags_for_register_hotkey() {
        let flags = hotkey_modifiers(Modifiers::NONE.with(Modifier::Ctrl).with(Modifier::Win));
        assert_eq!(flags, MOD_NOREPEAT | MOD_CONTROL | MOD_WIN);
        assert_eq!(hotkey_modifiers(Modifiers::NONE), MOD_NOREPEAT);
    }
}
