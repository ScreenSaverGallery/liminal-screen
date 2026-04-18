use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, Manager, Runtime, State};

#[cfg(target_os = "windows")]
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

pub struct PowerSaveBlocker {
    #[cfg(target_os = "macos")]
    assertion_id: Arc<Mutex<Option<u32>>>,
    #[cfg(target_os = "windows")]
    assertion_id: Arc<Mutex<Option<u32>>>,
    #[cfg(target_os = "linux")]
    assertion_id: Arc<Mutex<Option<u32>>>,
}

impl PowerSaveBlocker {
    pub fn new() -> Self {
        Self {
            assertion_id: Arc::new(Mutex::new(None)),
        }
    }
}

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
    state: State<PowerSaveBlocker>,
) -> Result<u32, String> {
    #[cfg(target_os = "windows")]
    prevent_sleep_windows(&state)?;

    #[cfg(target_os = "macos")]
    prevent_sleep_macos(&state)?;

    #[cfg(target_os = "linux")]
    prevent_sleep_linux(&state)?;

    Ok(1)
}

#[command]
pub fn allow_display_sleep<R: Runtime>(
    _app: AppHandle<R>,
    state: State<PowerSaveBlocker>,
    _blocker_id: u32,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    allow_sleep_windows(&state)?;

    #[cfg(target_os = "macos")]
    allow_sleep_macos(&state)?;

    #[cfg(target_os = "linux")]
    allow_sleep_linux(&state)?;

    Ok(())
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_idle_time_windows() -> Result<u64, String> {
    use windows::Win32::System::SystemServices::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    unsafe {
        let mut last_input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        if GetLastInputInfo(&mut last_input).is_ok() {
            let tick_count = GetTickCount();
            let idle_ms = tick_count - last_input.dwTime;
            Ok((idle_ms / 1000) as u64)
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
    use std::process::Command;

    Command::new("rundll32.exe")
        .args(&["user32.dll,LockWorkStation"])
        .spawn()
        .map_err(|e| format!("Failed to lock screen: {}", e))?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn blank_screen_windows() -> Result<(), String> {
    use std::process::Command;

    Command::new("powershell.exe")
        .args(&[
            "-Command",
            "(Add-Type '[DllImport(\"user32.dll\")]public static extern int SendMessage(int hWnd, int hMsg, int wParam, int lParam);' -Name a -Namespace Win32Functions -PassThru)::SendMessage(-1, 0x0112, 0xF170, 2)"
        ])
        .spawn()
        .map_err(|e| format!("Failed to blank screen: {}", e))?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn prevent_sleep_windows(state: &PowerSaveBlocker) -> Result<(), String> {
    use windows::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
    };

    let new_state = ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED | ES_CONTINUOUS;
    let prev_state = unsafe { SetThreadExecutionState(new_state) };

    if prev_state.0 == 0 {
        return Err("Failed to set thread execution state".to_string());
    }

    if let Ok(mut id) = state.assertion_id.lock() {
        *id = Some(prev_state.0);
    }

    println!("Windows: Display sleep prevented");
    Ok(())
}

#[cfg(target_os = "windows")]
fn allow_sleep_windows(state: &PowerSaveBlocker) -> Result<(), String> {
    use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS, EXECUTION_STATE};

    let restored = if let Ok(id) = state.assertion_id.lock() {
        if let Some(prev_state) = *id {
            let result = unsafe { SetThreadExecutionState(EXECUTION_STATE(prev_state)) };
            result.0 != 0
        } else {
            let result = unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
            result.0 != 0
        }
    } else {
        false
    };

    if let Ok(mut id) = state.assertion_id.lock() {
        *id = None;
    }

    if restored {
        println!("Windows: Display sleep restored");
        Ok(())
    } else {
        Err("Failed to restore thread execution state".to_string())
    }
}

// ─── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn get_idle_time_macos() -> Result<u64, String> {
    use std::process::Command;

    // Method 1: ioreg -c IOHIDSystem (works on Intel Macs)
    let output = Command::new("ioreg").args(&["-c", "IOHIDSystem"]).output();

    if let Ok(output) = output {
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
    }

    // Method 2: ioreg -l (more detailed, may work on Apple Silicon)
    let output = Command::new("ioreg").args(&["-l"]).output();

    if let Ok(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            if line.contains("HIDIdleTime") || line.contains("\"IdleTime\"") {
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
    }

    // Method 3: CGEventSource via osascript
    let output = Command::new("osascript")
        .args(&["-e", "tell application \"System Events\" to get idle time"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Ok(seconds) = stdout.trim().parse::<f64>() {
            return Ok(seconds as u64);
        }
    }

    Err("Failed to get idle time on macOS — all methods failed".to_string())
}

#[cfg(target_os = "macos")]
fn is_on_battery_macos() -> Result<bool, String> {
    use std::process::Command;

    let output = Command::new("pmset")
        .args(&["-g", "ps"])
        .output()
        .map_err(|e| format!("Failed to execute pmset: {}", e))?;

    Ok(String::from_utf8_lossy(&output.stdout).contains("Battery Power"))
}

#[cfg(target_os = "macos")]
fn blank_screen_macos() -> Result<(), String> {
    use std::process::Command;

    Command::new("pmset")
        .args(&["displaysleepnow"])
        .spawn()
        .map_err(|e| format!("Failed to blank screen: {}", e))?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn prevent_sleep_macos(state: &PowerSaveBlocker) -> Result<(), String> {
    use std::process::Command;

    if state.assertion_id.lock().unwrap().is_some() {
        return Ok(()); // already active
    }

    let child = Command::new("caffeinate")
        .args(&["-d", "-w", &std::process::id().to_string()])
        .spawn()
        .map_err(|e| format!("Failed to spawn caffeinate: {}", e))?;

    let pid = child.id();
    if let Ok(mut id) = state.assertion_id.lock() {
        *id = Some(pid);
    }
    println!("macOS: Display sleep prevented (caffeinate pid {})", pid);
    Ok(())
}

#[cfg(target_os = "macos")]
fn allow_sleep_macos(state: &PowerSaveBlocker) -> Result<(), String> {
    use std::process::Command;

    if let Ok(mut id) = state.assertion_id.lock() {
        if let Some(pid) = *id {
            let _ = Command::new("kill")
                .args(&["-TERM", &pid.to_string()])
                .spawn();
            *id = None;
            println!(
                "macOS: Display sleep allowed (terminated caffeinate pid {})",
                pid
            );
        }
    }
    Ok(())
}

// ─── Linux ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn get_idle_time_linux() -> Result<u64, String> {
    if let Ok(idle) = get_idle_time_x11() {
        return Ok(idle);
    }
    Err("Failed to get idle time on Linux".to_string())
}

#[cfg(target_os = "linux")]
fn get_idle_time_x11() -> Result<u64, String> {
    use std::process::Command;

    let output = Command::new("xprintidle")
        .output()
        .map_err(|_| "xprintidle not available")?;

    if output.status.success() {
        let idle_ms = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .map_err(|e| format!("Failed to parse xprintidle output: {}", e))?;
        Ok(idle_ms / 1000)
    } else {
        Err("xprintidle failed".to_string())
    }
}

#[cfg(target_os = "linux")]
fn is_on_battery_linux() -> Result<bool, String> {
    let power_supply_path = Path::new("/sys/class/power_supply");

    if !power_supply_path.exists() {
        return Ok(false);
    }

    for entry in fs::read_dir(power_supply_path)
        .map_err(|e| format!("Failed to read power supply directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            if name_str.starts_with("AC") || name_str.starts_with("ADP") {
                let online_path = path.join("online");
                if let Ok(content) = fs::read_to_string(online_path) {
                    return Ok(content.trim() != "1");
                }
            }
        }
    }

    Ok(true) // No AC adapter found — assume battery
}

#[cfg(target_os = "linux")]
fn lock_screen_linux() -> Result<(), String> {
    use std::process::Command;

    let commands: &[&[&str]] = &[
        &["loginctl", "lock-session"],
        &["gnome-screensaver-command", "-l"],
        &["xdg-screensaver", "lock"],
        &["kscreenlocker_greet", "--lock"],
    ];

    for cmd_args in commands {
        if let Some((cmd, args)) = cmd_args.split_first() {
            if Command::new(cmd).args(*args).spawn().is_ok() {
                return Ok(());
            }
        }
    }

    Err("Failed to lock screen: no compatible command found".to_string())
}

#[cfg(target_os = "linux")]
fn blank_screen_linux() -> Result<(), String> {
    use std::process::Command;

    let commands: &[&[&str]] = &[
        &["xset", "dpms", "force", "off"],
        &["gnome-screensaver-command", "-a"],
        &["xdg-screensaver", "activate"],
    ];

    for cmd_args in commands {
        if let Some((cmd, args)) = cmd_args.split_first() {
            if Command::new(cmd).args(*args).spawn().is_ok() {
                return Ok(());
            }
        }
    }

    Err("Failed to blank screen: no compatible command found".to_string())
}

#[cfg(target_os = "linux")]
fn prevent_sleep_linux(state: &PowerSaveBlocker) -> Result<(), String> {
    use std::process::Command;

    if state.assertion_id.lock().unwrap().is_some() {
        return Ok(());
    }

    let app_name = std::env::var("VITE_APP_NAME").unwrap_or_else(|_| "Liminal Screen".to_string());

    let result = Command::new("systemd-inhibit")
        .args(&[
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
            let pid = child.id();
            if let Ok(mut id) = state.assertion_id.lock() {
                *id = Some(pid);
            }
            println!(
                "Linux: Display sleep prevented (systemd-inhibit pid {})",
                pid
            );
        }
        Err(e) => println!("Linux: Warning: Could not prevent display sleep: {}", e),
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn allow_sleep_linux(state: &PowerSaveBlocker) -> Result<(), String> {
    use std::process::Command;

    if let Ok(mut id) = state.assertion_id.lock() {
        if let Some(pid) = *id {
            let _ = Command::new("kill")
                .args(&["-TERM", &pid.to_string()])
                .spawn();
            *id = None;
            println!(
                "Linux: Display sleep allowed (terminated systemd-inhibit pid {})",
                pid
            );
        }
    }
    Ok(())
}

// ─── Direct (no State<T>) versions — callable from engine without Tauri context ──

/// Prevent display sleep — direct call, no State wrapper.
pub fn prevent_display_sleep_direct() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return prevent_sleep_windows_direct();

    #[cfg(target_os = "macos")]
    return prevent_sleep_macos_direct();

    #[cfg(target_os = "linux")]
    return prevent_sleep_linux_direct();

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Ok(())
}

/// Allow display sleep — direct call, no State wrapper.
pub fn allow_display_sleep_direct() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return allow_sleep_windows_direct();

    #[cfg(target_os = "macos")]
    return allow_sleep_macos_direct();

    #[cfg(target_os = "linux")]
    return allow_sleep_linux_direct();

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Ok(())
}

/// Lock the system — direct call, no State wrapper.
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

#[cfg(target_os = "windows")]
fn prevent_sleep_windows_direct() -> Result<(), String> {
    use windows::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
    };

    let new_state = ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED | ES_CONTINUOUS;
    let prev_state = unsafe { SetThreadExecutionState(new_state) };

    if prev_state.0 == 0 {
        return Err("Failed to set thread execution state".to_string());
    }

    println!("Windows: Display sleep prevented (direct)");
    Ok(())
}

#[cfg(target_os = "windows")]
fn allow_sleep_windows_direct() -> Result<(), String> {
    use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS};

    let result = unsafe { SetThreadExecutionState(ES_CONTINUOUS) };

    if result.0 == 0 {
        return Err("Failed to restore thread execution state".to_string());
    }

    println!("Windows: Display sleep restored (direct)");
    Ok(())
}

#[cfg(target_os = "macos")]
fn prevent_sleep_macos_direct() -> Result<(), String> {
    use std::process::Command;

    // -d: prevent display sleep; -w <pid>: auto-exit when our process exits
    let result = Command::new("caffeinate")
        .args(&["-d", "-w", &std::process::id().to_string()])
        .spawn();

    match result {
        Ok(_) => {
            println!("macOS: Display sleep prevented (caffeinate direct)");
            Ok(())
        }
        Err(e) => Err(format!(
            "Failed to prevent display sleep via caffeinate: {}",
            e
        )),
    }
}

#[cfg(target_os = "macos")]
fn allow_sleep_macos_direct() -> Result<(), String> {
    use std::process::Command;

    // Kill only caffeinate processes spawned with our PID via -w <our_pid>
    let pattern = format!("caffeinate.*{}", std::process::id());
    let _ = Command::new("pkill").args(&["-f", &pattern]).spawn();

    println!("macOS: Display sleep allowed (direct)");
    Ok(())
}

#[cfg(target_os = "macos")]
fn lock_system_macos_direct() -> Result<(), String> {
    use std::process::Command;

    // CGSession -suspend triggers the macOS lock screen
    let cgsession =
        "/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession";

    match Command::new(cgsession).arg("-suspend").spawn() {
        Ok(_) => {
            println!("macOS: System locked via CGSession");
            Ok(())
        }
        Err(e) => {
            // Fallback: at least sleep the display
            println!(
                "CGSession failed ({}), falling back to pmset displaysleepnow",
                e
            );
            Command::new("pmset")
                .args(&["displaysleepnow"])
                .spawn()
                .map_err(|e2| format!("Lock failed — CGSession: {} / pmset: {}", e, e2))?;
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn prevent_sleep_linux_direct() -> Result<(), String> {
    use std::process::Command;

    let app_name = std::env::var("VITE_APP_NAME").unwrap_or_else(|_| "Liminal Screen".to_string());

    let result = Command::new("systemd-inhibit")
        .args(&[
            "--what=idle:sleep",
            &format!("--who={}", app_name),
            "--why=Screensaver active",
            "--mode=block",
            "sleep",
            "infinity",
        ])
        .spawn();

    match result {
        Ok(_) => {
            println!("Linux: Display sleep prevented via systemd-inhibit (direct)");
            Ok(())
        }
        Err(e) => {
            let _ = Command::new("xdg-screensaver")
                .args(&["suspend", &std::process::id().to_string()])
                .spawn();
            println!(
                "Linux: systemd-inhibit failed ({}), tried xdg-screensaver",
                e
            );
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
fn allow_sleep_linux_direct() -> Result<(), String> {
    use std::process::Command;

    let _ = Command::new("pkill")
        .args(&["-f", "systemd-inhibit.*liminal-screen"])
        .spawn();

    let _ = Command::new("xdg-screensaver")
        .args(&["resume", &std::process::id().to_string()])
        .spawn();

    println!("Linux: Display sleep allowed (direct)");
    Ok(())
}

// ─── Plugin initialization ────────────────────────────────────────────────────

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("power-monitor")
        .setup(|app, _api| {
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
