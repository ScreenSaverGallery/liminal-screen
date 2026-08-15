// Power monitor — idle time, battery state, screen blank/lock, sleep inhibition.
//
// Platform matrix:
//   macOS   — CGEventSource FFI (idle), IOKit FFI (battery), caffeinate (inhibit),
//             IOKit power assertions (detect non-HID activity: video, FaceTime),
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

/// Tracks whether the previous tick was blocked by a foreign display-sleep
/// assertion, so `get_effective_idle_time` only logs (and looks up the
/// blocking process's name) on the false→true edge, not on every tick of a
/// blocking episode that might last hours.
#[cfg(target_os = "macos")]
static WAS_BLOCKED_BY_MEDIA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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

/// Returns true when something other than Liminal is actively preventing display
/// sleep via an OS power assertion. On macOS this catches video players,
/// FaceTime, etc. that keep the display alive without HID input, so the
/// screensaver engine can treat the user as active even though the raw idle
/// timer is high. Always false on Windows/Linux in this implementation.
#[command]
pub fn is_media_active() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    return is_user_active_via_power_assertions();

    #[cfg(not(target_os = "macos"))]
    Ok(false)
}

/// Name of the process (if any) other than Liminal that's holding the
/// display-sleep assertion `is_media_active` detected. Separate from that
/// fast IOKit-only check because it shells out to `pmset` for the
/// human-readable owning-process list — fine for an occasional "why is the
/// saver blocked?" UI query, too heavy to run every second alongside the
/// engine's idle-time tick.
#[command]
pub fn get_media_blocker_name() -> Result<Option<String>, String> {
    #[cfg(target_os = "macos")]
    return Ok(find_media_blocking_process());

    #[cfg(not(target_os = "macos"))]
    Ok(None)
}

/// Raw HID idle time plus platform-specific activity signals. On macOS, if
/// another process holds a `NoDisplaySleep` power assertion, this returns 0
/// (the user is treated as active). Other platforms fall back to raw idle time.
pub fn get_effective_idle_time() -> Result<u64, String> {
    let raw = get_system_idle_time()?;

    #[cfg(target_os = "macos")]
    {
        use std::sync::atomic::Ordering;

        if is_user_active_via_power_assertions()? {
            // Only log (and shell out for the name) on the false→true edge —
            // this branch can otherwise fire every second for hours.
            if !WAS_BLOCKED_BY_MEDIA.swap(true, Ordering::Relaxed) {
                let blocker = find_media_blocking_process();
                println!(
                    "macOS: HID idle is {}s but {} holds a display-sleep assertion — treating as active",
                    raw,
                    blocker.as_deref().unwrap_or("another process")
                );
            }
            return Ok(0);
        }
        WAS_BLOCKED_BY_MEDIA.store(false, Ordering::Relaxed);
    }

    Ok(raw)
}

/// Snapshot of the OS-native screensaver configuration. Liminal is meant to be
/// the *only* screensaver — a system screensaver on an overlapping timer will
/// draw over Liminal's windows (its idle-sleep assertion does not suppress it),
/// so the options UI warns the user when one is enabled.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsScreensaverStatus {
    /// Whether we could actually read the setting on this platform/desktop.
    pub detected: bool,
    /// True when the OS screensaver is configured to activate on an idle timer.
    pub enabled: bool,
    /// Idle seconds before the OS screensaver starts; None when disabled/unknown.
    pub idle_seconds: Option<u64>,
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl OsScreensaverStatus {
    fn unknown() -> Self {
        Self {
            detected: false,
            enabled: false,
            idle_seconds: None,
        }
    }

    fn disabled() -> Self {
        Self {
            detected: true,
            enabled: false,
            idle_seconds: None,
        }
    }

    fn enabled(idle_seconds: Option<u64>) -> Self {
        Self {
            detected: true,
            enabled: true,
            idle_seconds,
        }
    }
}

#[command]
pub fn get_os_screensaver_status() -> Result<OsScreensaverStatus, String> {
    #[cfg(target_os = "macos")]
    return Ok(os_screensaver_status_macos());

    #[cfg(target_os = "windows")]
    return Ok(os_screensaver_status_windows());

    #[cfg(target_os = "linux")]
    return Ok(os_screensaver_status_linux());

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    Ok(OsScreensaverStatus {
        detected: false,
        enabled: false,
        idle_seconds: None,
    })
}

/// Disable the OS-native screensaver ("Never"). Reversible via
/// [`set_os_screensaver_idle_direct`]; callers persist the prior value first.
pub fn set_os_screensaver_disabled_direct() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return write_screensaver_idle_macos(0);

    #[cfg(target_os = "windows")]
    return set_screensaver_active_windows(false, None);

    #[cfg(target_os = "linux")]
    return write_gnome_idle_delay(0);

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    Ok(())
}

/// Restore the OS-native screensaver to the given idle timeout (seconds).
pub fn set_os_screensaver_idle_direct(seconds: u64) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return write_screensaver_idle_macos(seconds);

    #[cfg(target_os = "windows")]
    return set_screensaver_active_windows(true, Some(seconds));

    #[cfg(target_os = "linux")]
    return write_gnome_idle_delay(seconds);

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = seconds;
        Ok(())
    }
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

/// Reverse a forced blank. On Linux this resets Mutter PowerSaveMode; on other
/// platforms the OS wakes the display on user input, so this is a no-op.
pub fn unblank_screen() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        if let Err(e) = set_mutter_power_save_mode(0) {
            println!("Mutter unblank not available: {}", e);
            return Err(e);
        }
        println!("Linux: Display unblanked via Mutter PowerSaveMode");
        return Ok(());
    }

    #[cfg(not(target_os = "linux"))]
    Ok(())
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
    /// Return a dictionary mapping assertion-type strings to active counts.
    /// Used to detect display-sleep assertions held by video players, FaceTime,
    /// etc. that do not reset the HID idle timer.
    fn IOPMCopyAssertionsStatus(assertions: *mut core_foundation::base::CFTypeRef) -> i32;
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

/// True when a process other than Liminal is holding a display-sleep-blocking
/// power assertion. Video players and video calls keep the display awake this
/// way without generating HID events, so the raw idle timer would wrongly say
/// the user is idle. We subtract our own `caffeinate -d` assertion (tracked in
/// `INHIBIT_CHILD`) so Liminal doesn't count itself as activity.
#[cfg(target_os = "macos")]
fn is_user_active_via_power_assertions() -> Result<bool, String> {
    use core_foundation::base::{CFTypeRef, TCFType};
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;

    let mut raw: CFTypeRef = std::ptr::null();
    let result = unsafe { IOPMCopyAssertionsStatus(&mut raw) };
    if result != 0 {
        // Could not read assertion status; fail safe and assume not active.
        return Ok(false);
    }

    let dict = unsafe {
        CFDictionary::<CFString, CFNumber>::wrap_under_create_rule(raw as CFDictionaryRef)
    };

    // The dictionary keys are the actual assertion type names, not the
    // constant name "NoDisplaySleep" (verified via `pmset -g assertions`).
    // Video players/calls hold `PreventUserIdleDisplaySleep`; `caffeinate -d`
    // (Liminal's own inhibitor) shows up under the same key, while some older
    // tools still register the legacy `NoDisplaySleepAssertion` type.
    let count_for = |name: &str| -> i64 {
        dict.find(&CFString::new(name))
            .and_then(|n| n.to_i64())
            .unwrap_or(0)
    };
    let total = count_for("PreventUserIdleDisplaySleep") + count_for("NoDisplaySleepAssertion");

    // Liminal itself holds a `PreventUserIdleDisplaySleep` assertion while the
    // saver is shown (via caffeinate -d). Exclude it so we don't self-suppress.
    let own_assertion = INHIBIT_CHILD.lock().unwrap().is_some();
    let others = (total - if own_assertion { 1 } else { 0 }).max(0);

    Ok(others > 0)
}

/// Name of the first process other than Liminal holding a display-sleep
/// assertion, e.g. "LocalSend". Parses `pmset -g assertions`' per-process
/// listing rather than the IOKit dictionary used above, because that's the
/// only place macOS exposes the owning process's name — `IOPMCopyAssertionsStatus`
/// only gives system-wide counts. A line looks like:
///   pid 30046(LocalSend): [0x...] 36:24:34 NoDisplaySleepAssertion named: "..."
#[cfg(target_os = "macos")]
fn find_media_blocking_process() -> Option<String> {
    const TARGET_TYPES: [&str; 2] = ["PreventUserIdleDisplaySleep", "NoDisplaySleepAssertion"];

    let own_pid = INHIBIT_CHILD.lock().unwrap().as_ref().map(|c| c.id());

    let output = std::process::Command::new("pmset")
        .args(["-g", "assertions"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);

    let mut in_process_section = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Listed by owning process") {
            in_process_section = true;
            continue;
        }
        if !in_process_section || !trimmed.starts_with("pid ") {
            continue;
        }

        let Some(paren_open) = trimmed.find('(') else {
            continue;
        };
        let Some(paren_close) = trimmed[paren_open..].find(')').map(|i| i + paren_open) else {
            continue;
        };

        let pid: Option<u32> = trimmed["pid ".len()..paren_open].trim().parse().ok();
        if pid.is_some() && pid == own_pid {
            continue; // our own caffeinate -d inhibitor
        }

        let assertion_part = &trimmed[paren_close..];
        if TARGET_TYPES.iter().any(|t| assertion_part.contains(t)) {
            return Some(trimmed[paren_open + 1..paren_close].to_string());
        }
    }
    None
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

/// Read the system screensaver idle timeout. `defaults -currentHost read
/// com.apple.screensaver idleTime` returns the seconds before the OS screensaver
/// starts; 0 means "Never". A missing key or non-zero exit means we can't tell,
/// so we report `detected: false` rather than guessing a default.
#[cfg(target_os = "macos")]
fn os_screensaver_status_macos() -> OsScreensaverStatus {
    let output = std::process::Command::new("defaults")
        .args(["-currentHost", "read", "com.apple.screensaver", "idleTime"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            match String::from_utf8_lossy(&out.stdout).trim().parse::<u64>() {
                Ok(0) => OsScreensaverStatus::disabled(),
                Ok(secs) => OsScreensaverStatus::enabled(Some(secs)),
                Err(_) => OsScreensaverStatus::unknown(),
            }
        }
        _ => OsScreensaverStatus::unknown(),
    }
}

/// Write the system screensaver idle timeout (0 = Never). `defaults` lands the
/// value in the cfprefsd-backed store; we then nudge cfprefsd so the screensaver
/// subsystem re-reads it promptly rather than on next login (best-effort).
#[cfg(target_os = "macos")]
fn write_screensaver_idle_macos(seconds: u64) -> Result<(), String> {
    use std::process::Command;

    let status = Command::new("defaults")
        .args([
            "-currentHost",
            "write",
            "com.apple.screensaver",
            "idleTime",
            "-int",
            &seconds.to_string(),
        ])
        .status()
        .map_err(|e| format!("Failed to run defaults write: {}", e))?;
    if !status.success() {
        return Err(format!(
            "defaults write com.apple.screensaver idleTime exited with {:?}",
            status.code()
        ));
    }

    // Flush the prefs cache so the change is picked up without a re-login.
    let _ = Command::new("killall").arg("cfprefsd").status();
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
    block_on_zbus(async {
        let conn = zbus::Connection::session().await?;
        let proxy = zbus::Proxy::new(
            &conn,
            "org.gnome.Mutter.IdleMonitor",
            "/org/gnome/Mutter/IdleMonitor/Core",
            "org.gnome.Mutter.IdleMonitor",
        )
        .await?;
        proxy
            .call_method("GetIdletime", &())
            .await?
            .body()
            .deserialize::<u64>()
    })
    .map(|ms| ms / 1000)
}

/// KDE and others implementing org.freedesktop.ScreenSaver.GetSessionIdleTime
/// (returns seconds as uint32). GNOME does not implement this method.
#[cfg(target_os = "linux")]
fn idle_fdo_screensaver_dbus() -> Option<u64> {
    block_on_zbus(async {
        let conn = zbus::Connection::session().await?;
        let proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.ScreenSaver",
            "/ScreenSaver",
            "org.freedesktop.ScreenSaver",
        )
        .await?;
        proxy
            .call_method("GetSessionIdleTime", &())
            .await?
            .body()
            .deserialize::<u32>()
            .map(u64::from)
    })
}

/// Run a zbus async block on the current thread. zbus is async-first; this is
/// acceptable because these calls are short local D-Bus round-trips.
#[cfg(target_os = "linux")]
fn block_on_zbus<T, F>(future: F) -> Option<T>
where
    F: std::future::Future<Output = Result<T, zbus::Error>>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.block_on(future) {
            Ok(v) => Some(v),
            Err(e) => {
                println!("zbus call failed: {}", e);
                None
            }
        },
        Err(_) => match tokio::runtime::Runtime::new() {
            Ok(rt) => match rt.block_on(future) {
                Ok(v) => Some(v),
                Err(e) => {
                    println!("zbus call failed: {}", e);
                    None
                }
            },
            Err(e) => {
                println!("Failed to create tokio runtime for zbus: {}", e);
                None
            }
        },
    }
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
    // loginctl works on both X11 and Wayland under systemd-logind.
    if run_ok("loginctl", &["lock-session"]) {
        return Ok(());
    }
    // Pure-Rust D-Bus lock via org.freedesktop.ScreenSaver.
    if lock_screen_fdo_dbus().is_ok() {
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
fn lock_screen_fdo_dbus() -> Result<(), String> {
    block_on_zbus(async {
        let conn = zbus::Connection::session().await?;
        let proxy = zbus::Proxy::new(
            &conn,
            "org.freedesktop.ScreenSaver",
            "/ScreenSaver",
            "org.freedesktop.ScreenSaver",
        )
        .await?;
        proxy.call_method("Lock", &()).await.map(|_| ())
    })
    .ok_or_else(|| "D-Bus ScreenSaver.Lock failed".to_string())
}

#[cfg(target_os = "linux")]
fn blank_screen_linux() -> Result<(), String> {
    // GNOME/Mutter (X11 and Wayland): set PowerSaveMode to 1 to blank all
    // displays immediately. This is the only reliable way to force DPMS off on
    // GNOME Wayland, where XWayland's xset DPMS extension is a stub.
    if let Err(e) = set_mutter_power_save_mode(1) {
        println!("Mutter blank not available: {}", e);
    } else {
        println!("Linux: Display blanked via Mutter PowerSaveMode");
        return Ok(());
    }

    // KDE Wayland / Plasma.
    if run_ok("kscreen-doctor", &["--dpms", "off"]) {
        return Ok(());
    }

    // Native X11: real DPMS extension.
    if run_ok("xset", &["dpms", "force", "off"]) {
        return Ok(());
    }

    // Legacy screensaver activation (does NOT actually power off the panel).
    if run_ok("xdg-screensaver", &["activate"]) {
        return Ok(());
    }
    if run_ok("gnome-screensaver-command", &["-a"]) {
        return Ok(());
    }

    Err("Failed to blank screen: no compatible command found".to_string())
}

/// Set Mutter's PowerSaveMode property (0 = on, 1 = off / blanked).
#[cfg(target_os = "linux")]
fn set_mutter_power_save_mode(mode: i32) -> Result<(), String> {
    block_on_zbus(async {
        let conn = zbus::Connection::session().await?;
        let proxy = zbus::Proxy::new(
            &conn,
            "org.gnome.Mutter.DisplayConfig",
            "/org/gnome/Mutter/DisplayConfig",
            "org.freedesktop.DBus.Properties",
        )
        .await?;
        proxy
            .call_method(
                "Set",
                &(
                    "org.gnome.Mutter.DisplayConfig",
                    "PowerSaveMode",
                    zbus::zvariant::Value::I32(mode),
                ),
            )
            .await
            .map(|_| ())
    })
    .ok_or_else(|| "Failed to set Mutter PowerSaveMode".to_string())
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

/// Best-effort: GNOME's `org.gnome.desktop.session idle-delay` (uint32 seconds,
/// 0 = never) governs when the session blanks / the screensaver kicks in. Other
/// desktops (KDE, etc.) expose this differently, so a failure here is reported
/// as `detected: false` rather than a false "no conflict".
#[cfg(target_os = "linux")]
fn os_screensaver_status_linux() -> OsScreensaverStatus {
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.session", "idle-delay"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            // Value looks like "uint32 300".
            let raw = String::from_utf8_lossy(&out.stdout);
            match raw
                .split_whitespace()
                .last()
                .and_then(|s| s.parse::<u64>().ok())
            {
                Some(0) => OsScreensaverStatus::disabled(),
                Some(secs) => OsScreensaverStatus::enabled(Some(secs)),
                None => OsScreensaverStatus::unknown(),
            }
        }
        _ => OsScreensaverStatus::unknown(),
    }
}

/// GNOME best-effort: set `idle-delay` (0 = never) and match
/// `idle-activation-enabled`. Other desktops are left untouched.
#[cfg(target_os = "linux")]
fn write_gnome_idle_delay(seconds: u64) -> Result<(), String> {
    use std::process::Command;

    let status = Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.session",
            "idle-delay",
            &seconds.to_string(),
        ])
        .status()
        .map_err(|e| format!("Failed to run gsettings set: {}", e))?;
    if !status.success() {
        return Err(format!(
            "gsettings set idle-delay exited with {:?}",
            status.code()
        ));
    }

    // Keep the screensaver activation flag consistent with the delay.
    let _ = Command::new("gsettings")
        .args([
            "set",
            "org.gnome.desktop.screensaver",
            "idle-activation-enabled",
            if seconds == 0 { "false" } else { "true" },
        ])
        .status();
    Ok(())
}

// ─── Windows ───────────────────────────────────────────────────────────────────

/// Best-effort: `HKCU\Control Panel\Desktop` holds `ScreenSaveActive` ("1"/"0")
/// and `ScreenSaveTimeOut` (seconds). Read via `reg query` to avoid pulling in
/// the registry Win32 feature. If the active flag can't be read we report
/// `detected: false`.
#[cfg(target_os = "windows")]
fn os_screensaver_status_windows() -> OsScreensaverStatus {
    fn read_value(value: &str) -> Option<String> {
        let out = std::process::Command::new("reg")
            .args(["query", "HKCU\\Control Panel\\Desktop", "/v", value])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        // Line looks like: "    ScreenSaveTimeOut    REG_SZ    600"
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout
            .lines()
            .find(|l| l.contains(value))
            .and_then(|l| l.split_whitespace().last().map(str::to_string))
    }

    match read_value("ScreenSaveActive") {
        Some(active) if active == "1" => {
            let secs = read_value("ScreenSaveTimeOut").and_then(|v| v.parse::<u64>().ok());
            OsScreensaverStatus::enabled(secs.filter(|s| *s > 0))
        }
        Some(_) => OsScreensaverStatus::disabled(),
        None => OsScreensaverStatus::unknown(),
    }
}

/// Toggle the screensaver via `SystemParametersInfoW` — takes effect immediately
/// and persists to the user's profile (`SPIF_UPDATEINIFILE`). When enabling, an
/// optional timeout (seconds) is applied too.
#[cfg(target_os = "windows")]
fn set_screensaver_active_windows(active: bool, timeout: Option<u64>) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETSCREENSAVEACTIVE,
        SPI_SETSCREENSAVETIMEOUT,
    };

    unsafe {
        SystemParametersInfoW(
            SPI_SETSCREENSAVEACTIVE,
            u32::from(active),
            None,
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
        .map_err(|e| format!("SPI_SETSCREENSAVEACTIVE failed: {}", e))?;

        if active {
            if let Some(secs) = timeout {
                SystemParametersInfoW(
                    SPI_SETSCREENSAVETIMEOUT,
                    secs as u32,
                    None,
                    SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
                )
                .map_err(|e| format!("SPI_SETSCREENSAVETIMEOUT failed: {}", e))?;
            }
        }
    }
    Ok(())
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
            is_media_active,
            get_media_blocker_name,
            get_os_screensaver_status,
            lock_screen,
            blank_screen,
            prevent_display_sleep,
            allow_display_sleep,
        ])
        .build()
}
