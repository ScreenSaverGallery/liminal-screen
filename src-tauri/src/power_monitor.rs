// Power monitor — idle time, battery state, screen blank/lock, sleep inhibition.
//
// Platform matrix:
//   macOS   — CGEventSource FFI (idle), IOKit FFI (battery), caffeinate (inhibit),
//             AppleScript/ScreenSaverEngine/pmset (lock), pmset (blank)
//   Windows — GetLastInputInfo (idle), GetSystemPowerStatus (battery),
//             SetThreadExecutionState on a dedicated thread (inhibit),
//             LockWorkStation (lock), SC_MONITORPOWER broadcast (blank)
//   Linux   — xprintidle (X11) with D-Bus fallbacks for Wayland: Mutter IdleMonitor
//             (GNOME) and org.freedesktop.ScreenSaver (KDE). systemd-inhibit (inhibit),
//             loginctl / D-Bus / xdg-screensaver (lock), xset / kscreen-doctor (blank)

use tauri::{command, AppHandle, Runtime, State};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::sync::Mutex;

#[cfg(target_os = "windows")]
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

/// Kept as managed plugin state for command signature stability; the actual
/// inhibitor bookkeeping lives in module-level statics shared with the
/// `*_direct` functions so the engine and JS commands never fight each other.
pub struct PowerSaveBlocker;

impl PowerSaveBlocker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PowerSaveBlocker {
    fn default() -> Self {
        Self::new()
    }
}

/// Child process holding the sleep inhibition (caffeinate on macOS,
/// systemd-inhibit on Linux). Keeping the Child lets us kill AND reap it —
/// the previous pkill approach leaked zombies and used a pattern that never
/// matched when VITE_APP_NAME was customized.
#[cfg(any(target_os = "macos", target_os = "linux"))]
static INHIBIT_CHILD: Mutex<Option<std::process::Child>> = Mutex::new(None);

// ─── Commands callable from JavaScript ───────────────────────────────────────

#[command]
pub fn get_system_idle_time() -> Result<u64, String> {
    #[cfg(target_os = "windows")]
    return get_idle_time_windows();

    #[cfg(target_os = "macos")]
    return get_idle_time_macos();

    #[cfg(target_os = "linux")]
    return get_idle_time_linux();
}

#[command]
pub fn get_system_idle_state(threshold: u64) -> Result<String, String> {
    let idle_time = get_system_idle_time()?;
    if idle_time >= threshold {
        Ok("idle".to_string())
    } else {
        Ok("active".to_string())
    }
}

#[command]
pub fn is_on_battery_power() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    return is_on_battery_windows();

    #[cfg(target_os = "macos")]
    return is_on_battery_macos();

    #[cfg(target_os = "linux")]
    return is_on_battery_linux();
}

#[command]
pub fn lock_screen() -> Result<(), String> {
    lock_system_direct()
}

#[command]
pub fn blank_screen() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return blank_screen_windows();

    #[cfg(target_os = "macos")]
    return blank_screen_macos();

    #[cfg(target_os = "linux")]
    return blank_screen_linux();
}

#[command]
pub fn prevent_display_sleep<R: Runtime>(
    _app: AppHandle<R>,
    _state: State<PowerSaveBlocker>,
) -> Result<u32, String> {
    prevent_display_sleep_direct().map(|_| 1)
}

#[command]
pub fn allow_display_sleep<R: Runtime>(
    _app: AppHandle<R>,
    _state: State<PowerSaveBlocker>,
    _blocker_id: u32,
) -> Result<(), String> {
    allow_display_sleep_direct()
}

// ─── Direct versions — callable from the engine without Tauri State ──────────

/// Prevent display sleep. Idempotent — repeated calls keep a single inhibitor.
pub fn prevent_display_sleep_direct() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return win_power::prevent();

    #[cfg(target_os = "macos")]
    return prevent_sleep_macos_direct();

    #[cfg(target_os = "linux")]
    return prevent_sleep_linux_direct();

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Ok(())
}

/// Allow display sleep — releases the inhibitor acquired above.
pub fn allow_display_sleep_direct() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return win_power::allow();

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    return release_inhibit_child();

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Ok(())
}

/// Lock the system session.
pub fn lock_system_direct() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return lock_system_macos_direct();

    #[cfg(target_os = "windows")]
    return lock_screen_windows();

    #[cfg(target_os = "linux")]
    return lock_screen_linux();

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        println!("Warning: Lock not implemented for this platform");
        Ok(())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn store_inhibit_child(child: std::process::Child) {
    let mut guard = INHIBIT_CHILD.lock().unwrap();
    if let Some(mut old) = guard.replace(child) {
        let _ = old.kill();
        let _ = old.wait();
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn release_inhibit_child() -> Result<(), String> {
    if let Some(mut child) = INHIBIT_CHILD.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait(); // reap — otherwise the killed process stays a zombie
        println!("Display sleep inhibitor released (pid {})", child.id());
    }
    Ok(())
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_idle_time_windows() -> Result<u64, String> {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut last_input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        if GetLastInputInfo(&mut last_input).as_bool() {
            // Both values are 32-bit ms counters that wrap every ~49.7 days;
            // wrapping_sub gives the correct delta across the wrap boundary.
            let idle_ms = GetTickCount().wrapping_sub(last_input.dwTime);
            Ok(u64::from(idle_ms) / 1000)
        } else {
            Err("Failed to get last input info".to_string())
        }
    }
}

#[cfg(target_os = "windows")]
fn is_on_battery_windows() -> Result<bool, String> {
    unsafe {
        let mut status: SYSTEM_POWER_STATUS = std::mem::zeroed();
        if GetSystemPowerStatus(&mut status).is_ok() {
            Ok(status.ACLineStatus == 0)
        } else {
            Err("Failed to get power status".to_string())
        }
    }
}

#[cfg(target_os = "windows")]
fn lock_screen_windows() -> Result<(), String> {
    use windows::Win32::System::Shutdown::LockWorkStation;

    unsafe { LockWorkStation().map_err(|e| format!("LockWorkStation failed: {}", e)) }
}

#[cfg(target_os = "windows")]
fn blank_screen_windows() -> Result<(), String> {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageW, HWND_BROADCAST, SC_MONITORPOWER, WM_SYSCOMMAND,
    };

    // lParam 2 = power off the display
    unsafe {
        SendMessageW(
            HWND_BROADCAST,
            WM_SYSCOMMAND,
            Some(WPARAM(SC_MONITORPOWER as usize)),
            Some(LPARAM(2)),
        );
    }
    Ok(())
}

/// SetThreadExecutionState with ES_CONTINUOUS is per-thread and is cleared
/// when the calling thread exits. Tauri may run commands on short-lived
/// worker threads, so the calls are funneled to one dedicated long-lived
/// thread that owns the execution state for the whole app.
#[cfg(target_os = "windows")]
mod win_power {
    use std::sync::mpsc::{self, Sender};
    use std::sync::OnceLock;

    enum Msg {
        Prevent,
        Allow,
    }

    static TX: OnceLock<Sender<Msg>> = OnceLock::new();

    fn sender() -> &'static Sender<Msg> {
        TX.get_or_init(|| {
            let (tx, rx) = mpsc::channel::<Msg>();
            std::thread::Builder::new()
                .name("power-state".into())
                .spawn(move || {
                    use windows::Win32::System::Power::{
                        SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED,
                        ES_SYSTEM_REQUIRED,
                    };
                    for msg in rx {
                        unsafe {
                            match msg {
                                Msg::Prevent => {
                                    SetThreadExecutionState(
                                        ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED,
                                    );
                                }
                                Msg::Allow => {
                                    SetThreadExecutionState(ES_CONTINUOUS);
                                }
                            }
                        }
                    }
                })
                .expect("failed to spawn power-state thread");
            tx
        })
    }

    pub fn prevent() -> Result<(), String> {
        sender()
            .send(Msg::Prevent)
            .map_err(|e| format!("power-state thread unavailable: {}", e))?;
        println!("Windows: Display sleep prevented");
        Ok(())
    }

    pub fn allow() -> Result<(), String> {
        sender()
            .send(Msg::Allow)
            .map_err(|e| format!("power-state thread unavailable: {}", e))?;
        println!("Windows: Display sleep allowed");
        Ok(())
    }
}

// ─── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

#[cfg(target_os = "macos")]
#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPSCopyPowerSourcesInfo() -> core_foundation::base::CFTypeRef;
    fn IOPSGetProvidingPowerSourceType(
        snapshot: core_foundation::base::CFTypeRef,
    ) -> core_foundation::string::CFStringRef;
}

#[cfg(target_os = "macos")]
fn get_idle_time_macos() -> Result<u64, String> {
    // kCGEventSourceStateHIDSystemState = 1, kCGAnyInputEventType = ~0.
    // Works on both Intel and Apple Silicon, no subprocess, no permissions.
    const HID_SYSTEM_STATE: i32 = 1;
    const ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

    let secs =
        unsafe { CGEventSourceSecondsSinceLastEventType(HID_SYSTEM_STATE, ANY_INPUT_EVENT_TYPE) };
    if secs.is_finite() && secs >= 0.0 {
        return Ok(secs as u64);
    }

    // Fallback: parse HIDIdleTime from the IO registry
    get_idle_time_macos_ioreg()
}

#[cfg(target_os = "macos")]
fn get_idle_time_macos_ioreg() -> Result<u64, String> {
    use std::process::Command;

    let output = Command::new("ioreg")
        .args(["-c", "IOHIDSystem"])
        .output()
        .map_err(|e| format!("Failed to run ioreg: {}", e))?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    for line in output_str.lines() {
        if line.contains("HIDIdleTime") {
            if let Some(time_str) = line.split('=').nth(1) {
                let time_str = time_str
                    .trim()
                    .trim_end_matches(',')
                    .trim_matches('"')
                    .trim();
                if let Ok(time_ns) = time_str.parse::<u64>() {
                    return Ok(time_ns / 1_000_000_000);
                }
            }
        }
    }

    Err("Failed to get idle time on macOS — all methods failed".to_string())
}

#[cfg(target_os = "macos")]
fn is_on_battery_macos() -> Result<bool, String> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;

    unsafe {
        let snapshot = IOPSCopyPowerSourcesInfo();
        if !snapshot.is_null() {
            // wrap_under_create_rule releases the snapshot when dropped
            let _snapshot = CFType::wrap_under_create_rule(snapshot);
            let source_type = IOPSGetProvidingPowerSourceType(snapshot);
            if !source_type.is_null() {
                // Get rule: IOKit owns the string, we must not release it
                let s = CFString::wrap_under_get_rule(source_type).to_string();
                return Ok(s == "Battery Power");
            }
        }
    }

    // Fallback: pmset
    let output = std::process::Command::new("pmset")
        .args(["-g", "ps"])
        .output()
        .map_err(|e| format!("Failed to execute pmset: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).contains("Battery Power"))
}

#[cfg(target_os = "macos")]
fn blank_screen_macos() -> Result<(), String> {
    use std::process::Command;

    let status = Command::new("pmset")
        .args(["displaysleepnow"])
        .status()
        .map_err(|e| format!("Failed to blank screen: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "pmset displaysleepnow exited with {:?}",
            status.code()
        ))
    }
}

#[cfg(target_os = "macos")]
fn prevent_sleep_macos_direct() -> Result<(), String> {
    use std::process::Command;

    if INHIBIT_CHILD.lock().unwrap().is_some() {
        return Ok(()); // already active
    }

    // -d: prevent display sleep; -w <pid>: auto-exit when our process exits
    let child = Command::new("caffeinate")
        .args(["-d", "-w", &std::process::id().to_string()])
        .spawn()
        .map_err(|e| format!("Failed to spawn caffeinate: {}", e))?;

    println!(
        "macOS: Display sleep prevented (caffeinate pid {})",
        child.id()
    );
    store_inhibit_child(child);
    Ok(())
}

#[cfg(target_os = "macos")]
fn lock_system_macos_direct() -> Result<(), String> {
    use std::process::Command;

    // Modern macOS (10.15+) no longer ships CGSession at the legacy path.
    // Reliable lock methods, in order of preference:
    // 1. AppleScript keystroke — triggers the lock screen shortcut (Ctrl+Cmd+Q).
    //    Requires Accessibility permission (System Settings → Privacy → Accessibility);
    //    without it osascript exits non-zero and we fall through.
    // 2. Open ScreenSaverEngine — locks if "require password after screensaver" is on
    // 3. pmset displaysleepnow — same caveat as 2
    let applescript =
        "tell application \"System Events\" to keystroke \"q\" using {command down, control down}";
    match Command::new("osascript").args(["-e", applescript]).status() {
        Ok(status) if status.success() => {
            println!("macOS: System locked via AppleScript");
            return Ok(());
        }
        Ok(status) => println!("AppleScript lock exited with code: {:?}", status.code()),
        Err(e) => println!("AppleScript lock failed to run: {}", e),
    }

    match Command::new("open")
        .args(["-a", "ScreenSaverEngine"])
        .status()
    {
        Ok(status) if status.success() => {
            println!("macOS: ScreenSaverEngine launched (locks if passwd required)");
            return Ok(());
        }
        Ok(status) => println!("ScreenSaverEngine exited with code: {:?}", status.code()),
        Err(e) => println!("ScreenSaverEngine launch failed: {}", e),
    }

    println!("Falling back to pmset displaysleepnow — this only locks if 'Require password after sleep or screensaver' is enabled in System Settings");
    Command::new("pmset")
        .args(["displaysleepnow"])
        .status()
        .map_err(|e| format!("Lock failed — all methods exhausted. pmset: {}", e))?;

    Ok(())
}

// ─── Linux ────────────────────────────────────────────────────────────────────

/// Run a command and report success only if it exits 0. `spawn().is_ok()` is
/// NOT enough: on Wayland `xset` exists but fails, and the old code treated
/// "binary found" as "screen blanked".
#[cfg(target_os = "linux")]
fn run_ok(cmd: &str, args: &[&str]) -> bool {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|t| t.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn get_idle_time_linux() -> Result<u64, String> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Remember the last method that worked so we don't spawn up to three
    // subprocesses per tick on sessions where the first ones always fail.
    static LAST_GOOD: AtomicUsize = AtomicUsize::new(usize::MAX);

    type Method = fn() -> Option<u64>;
    // X11 sessions: xprintidle first. Wayland: D-Bus interfaces first —
    // xprintidle under XWayland only sees XWayland client input.
    let methods: &[Method] = if is_wayland_session() {
        &[idle_mutter_dbus, idle_fdo_screensaver_dbus, idle_xprintidle]
    } else {
        &[idle_xprintidle, idle_mutter_dbus, idle_fdo_screensaver_dbus]
    };

    let cached = LAST_GOOD.load(Ordering::Relaxed);
    if let Some(method) = methods.get(cached) {
        if let Some(secs) = method() {
            return Ok(secs);
        }
    }

    for (i, method) in methods.iter().enumerate() {
        if i == cached {
            continue;
        }
        if let Some(secs) = method() {
            LAST_GOOD.store(i, Ordering::Relaxed);
            return Ok(secs);
        }
    }

    Err("Failed to get idle time on Linux (tried xprintidle, Mutter IdleMonitor, org.freedesktop.ScreenSaver)".to_string())
}

/// X11: xprintidle prints idle milliseconds.
#[cfg(target_os = "linux")]
fn idle_xprintidle() -> Option<u64> {
    let output = std::process::Command::new("xprintidle").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let idle_ms = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .ok()?;
    Some(idle_ms / 1000)
}

/// GNOME (X11 + Wayland): Mutter IdleMonitor, returns milliseconds as uint64.
#[cfg(target_os = "linux")]
fn idle_mutter_dbus() -> Option<u64> {
    let output = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply=literal",
            "--dest=org.gnome.Mutter.IdleMonitor",
            "/org/gnome/Mutter/IdleMonitor/Core",
            "org.gnome.Mutter.IdleMonitor.GetIdletime",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    // Reply looks like: "   uint64 123456"
    let stdout = String::from_utf8_lossy(&output.stdout);
    let ms = stdout.split_whitespace().last()?.parse::<u64>().ok()?;
    Some(ms / 1000)
}

/// KDE and others implementing org.freedesktop.ScreenSaver.GetSessionIdleTime
/// (returns seconds as uint32). GNOME does not implement this method.
#[cfg(target_os = "linux")]
fn idle_fdo_screensaver_dbus() -> Option<u64> {
    let output = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply=literal",
            "--dest=org.freedesktop.ScreenSaver",
            "/ScreenSaver",
            "org.freedesktop.ScreenSaver.GetSessionIdleTime",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.split_whitespace().last()?.parse::<u64>().ok()
}

#[cfg(target_os = "linux")]
fn is_on_battery_linux() -> Result<bool, String> {
    let power_supply_path = Path::new("/sys/class/power_supply");

    if !power_supply_path.exists() {
        return Ok(false);
    }

    let mut has_battery = false;
    for entry in fs::read_dir(power_supply_path)
        .map_err(|e| format!("Failed to read power supply directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            if name_str.starts_with("AC") || name_str.starts_with("ADP") {
                if let Ok(content) = fs::read_to_string(path.join("online")) {
                    return Ok(content.trim() != "1");
                }
            }
            if name_str.starts_with("BAT") {
                has_battery = true;
            }
        }
    }

    // No AC adapter entry found: desktops (no battery at all) are on mains;
    // only assume battery when a battery device actually exists.
    Ok(has_battery)
}

#[cfg(target_os = "linux")]
fn lock_screen_linux() -> Result<(), String> {
    // loginctl works on both X11 and Wayland under systemd-logind; the D-Bus
    // ScreenSaver interface covers KDE and most desktops; the rest are legacy.
    if run_ok("loginctl", &["lock-session"]) {
        return Ok(());
    }
    if run_ok(
        "dbus-send",
        &[
            "--session",
            "--dest=org.freedesktop.ScreenSaver",
            "/ScreenSaver",
            "org.freedesktop.ScreenSaver.Lock",
        ],
    ) {
        return Ok(());
    }
    if run_ok("xdg-screensaver", &["lock"]) {
        return Ok(());
    }
    if run_ok("gnome-screensaver-command", &["-l"]) {
        return Ok(());
    }

    Err("Failed to lock screen: no compatible command found".to_string())
}

#[cfg(target_os = "linux")]
fn blank_screen_linux() -> Result<(), String> {
    // X11: xset DPMS. Wayland: kscreen-doctor covers KDE; GNOME Wayland has no
    // stable CLI for forcing DPMS off, so we fall back to screensaver activation.
    let commands: &[(&str, &[&str])] = if is_wayland_session() {
        &[
            ("kscreen-doctor", &["--dpms", "off"]),
            ("xset", &["dpms", "force", "off"]), // XWayland, best effort
            ("xdg-screensaver", &["activate"]),
        ]
    } else {
        &[
            ("xset", &["dpms", "force", "off"]),
            ("xdg-screensaver", &["activate"]),
            ("gnome-screensaver-command", &["-a"]),
        ]
    };

    for (cmd, args) in commands {
        if run_ok(cmd, args) {
            return Ok(());
        }
    }

    Err("Failed to blank screen: no compatible command found".to_string())
}

#[cfg(target_os = "linux")]
fn prevent_sleep_linux_direct() -> Result<(), String> {
    use std::process::Command;

    if INHIBIT_CHILD.lock().unwrap().is_some() {
        return Ok(()); // already active
    }

    let app_name = std::env::var("VITE_APP_NAME")
        .ok()
        .or_else(|| option_env!("VITE_APP_NAME").map(String::from))
        .unwrap_or_else(|| "Liminal Screen".to_string());

    let result = Command::new("systemd-inhibit")
        .args([
            "--what=idle:sleep",
            &format!("--who={}", app_name),
            "--why=Screensaver active",
            "--mode=block",
            "sleep",
            "infinity",
        ])
        .spawn();

    match result {
        Ok(child) => {
            println!(
                "Linux: Display sleep prevented (systemd-inhibit pid {})",
                child.id()
            );
            store_inhibit_child(child);
            Ok(())
        }
        Err(e) => {
            // Best effort fallback; xdg-screensaver suspend needs an X window id,
            // so this may be a no-op on some desktops.
            println!(
                "Linux: Warning: systemd-inhibit unavailable ({}), display may sleep",
                e
            );
            Ok(())
        }
    }
}

// ─── Plugin initialization ────────────────────────────────────────────────────

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("power-monitor")
        .setup(|app, _api| {
            use tauri::Manager;
            app.manage(PowerSaveBlocker::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_idle_time,
            get_system_idle_state,
            is_on_battery_power,
            lock_screen,
            blank_screen,
            prevent_display_sleep,
            allow_display_sleep,
        ])
        .build()
}
