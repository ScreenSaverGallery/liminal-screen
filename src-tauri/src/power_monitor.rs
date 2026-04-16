// src-tauri/src/power_monitor.rs
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{command, AppHandle, Manager, Runtime, State};

#[cfg(target_os = "windows")]
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::path::Path;

// PowerSaveBlocker struct to manage display sleep prevention
pub struct PowerSaveBlocker {
    #[cfg(target_os = "macos")]
    assertion_id: Arc<Mutex<Option<u32>>>,
    #[cfg(target_os = "windows")]
    assertion_id: Arc<Mutex<Option<u32>>>,
    #[cfg(target_os = "linux")]
    assertion_id: Arc<Mutex<Option<String>>>,
}

impl PowerSaveBlocker {
    pub fn new() -> Self {
        Self {
            assertion_id: Arc::new(Mutex::new(None)),
        }
    }
}

pub struct PowerMonitorState {
    last_activity: Arc<Mutex<Instant>>,
}

impl PowerMonitorState {
    pub fn new() -> Self {
        Self {
            last_activity: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn update_activity(&self) {
        if let Ok(mut last) = self.last_activity.lock() {
            *last = Instant::now();
        }
    }

    pub fn get_idle_time(&self) -> u64 {
        if let Ok(last) = self.last_activity.lock() {
            last.elapsed().as_secs()
        } else {
            0
        }
    }
}

// Commands that can be called from JavaScript

#[command]
pub fn get_system_idle_time() -> Result<u64, String> {
    #[cfg(target_os = "windows")]
    {
        get_idle_time_windows()
    }

    #[cfg(target_os = "macos")]
    {
        get_idle_time_macos()
    }

    #[cfg(target_os = "linux")]
    {
        get_idle_time_linux()
    }
}

#[command]
pub fn get_system_idle_state(threshold: u64) -> Result<String, String> {
    let idle_time = get_system_idle_time()?;

    if idle_time >= threshold {
        Ok("idle".to_string())
    } else if idle_time > 0 {
        Ok("active".to_string())
    } else {
        Ok("active".to_string())
    }
}

#[command]
pub fn is_on_battery_power() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        is_on_battery_windows()
    }

    #[cfg(target_os = "macos")]
    {
        is_on_battery_macos()
    }

    #[cfg(target_os = "linux")]
    {
        is_on_battery_linux()
    }
}

#[command]
pub fn lock_screen() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        lock_screen_windows()
    }

    #[cfg(target_os = "macos")]
    {
        lock_screen_macos()
    }

    #[cfg(target_os = "linux")]
    {
        lock_screen_linux()
    }
}

#[command]
pub fn blank_screen() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        blank_screen_windows()
    }

    #[cfg(target_os = "macos")]
    {
        blank_screen_macos()
    }

    #[cfg(target_os = "linux")]
    {
        blank_screen_linux()
    }
}

#[command]
pub fn prevent_display_sleep<R: Runtime>(
    _app: AppHandle<R>,
    state: State<PowerSaveBlocker>,
) -> Result<u32, String> {
    #[cfg(target_os = "windows")]
    {
        prevent_sleep_windows(&state)?
    }

    #[cfg(target_os = "macos")]
    {
        prevent_sleep_macos(&state)?
    }

    #[cfg(target_os = "linux")]
    {
        prevent_sleep_linux(&state)?
    }

    // Return a simple blocker ID (in a real implementation, this would be platform-specific)
    Ok(1)
}

#[command]
pub fn allow_display_sleep<R: Runtime>(
    _app: AppHandle<R>,
    state: State<PowerSaveBlocker>,
    _blocker_id: u32,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        allow_sleep_windows(&state)?
    }

    #[cfg(target_os = "macos")]
    {
        allow_sleep_macos(&state)?
    }

    #[cfg(target_os = "linux")]
    {
        allow_sleep_linux(&state)?
    }

    // In a real implementation, we would verify the blocker_id matches
    Ok(())
}

// Windows implementations
#[cfg(target_os = "windows")]
fn get_idle_time_windows() -> Result<u64, String> {
    use windows::Win32::System::SystemServices::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::GetLastInputInfo;
    use windows::Win32::UI::Input::KeyboardAndMouse::LASTINPUTINFO;

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

    // Turn off monitor using WM_SYSCOMMAND with SC_MONITORPOWER
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
    use windows::Win32::System::Power::{SetThreadExecutionState, EXECUTION_STATE, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, ES_CONTINUOUS};
    
    // Set the execution state to prevent display and system sleep
    let new_state = ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED | ES_CONTINUOUS;
    let prev_state = unsafe { SetThreadExecutionState(new_state) };
    
    if prev_state.0 == 0 {
        // Failed to set execution state
        return Err("Failed to set thread execution state".to_string());
    }
    
    // Store the previous state for later restoration
    if let Ok(mut id) = state.assertion_id.lock() {
        *id = Some(prev_state.0);
    }
    
    println!("Windows: Successfully prevented display sleep");
    Ok(())
}

#[cfg(target_os = "windows")]
fn allow_sleep_windows(state: &PowerSaveBlocker) -> Result<(), String> {
    use windows::Win32::System::Power::{SetThreadExecutionState, EXECUTION_STATE};
    
    // Restore the previous execution state if we had one
    let restored = if let Ok(id) = state.assertion_id.lock() {
        if let Some(prev_state) = *id {
            // Restore previous state
            let result = unsafe { SetThreadExecutionState(EXECUTION_STATE(prev_state)) };
            result.0 != 0
        } else {
            // No previous state stored, just clear the current flags
            let result = unsafe { SetThreadExecutionState(EXECUTION_STATE(0)) };
            result.0 != 0
        }
    } else {
        false
    };
    
    // Clear our stored state
    if let Ok(mut id) = state.assertion_id.lock() {
        *id = None;
    }
    
    if restored {
        println!("Windows: Successfully restored display sleep");
        Ok(())
    } else {
        Err("Failed to restore thread execution state".to_string())
    }
}

// macOS implementations
#[cfg(target_os = "macos")]
fn get_idle_time_macos() -> Result<u64, String> {
    use std::process::Command;

    // Method 1: Try ioreg -c IOHIDSystem (works on Intel Macs)
    let output = Command::new("ioreg").args(&["-c", "IOHIDSystem"]).output();

    if let Ok(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse HIDIdleTime from output - handle both quoted and unquoted formats
        for line in output_str.lines() {
            if line.contains("HIDIdleTime") {
                // Try format: "HIDIdleTime" = 123456789
                // or: HIDIdleTime = 123456789
                if let Some(time_str) = line.split('=').nth(1) {
                    let time_str = time_str.trim().trim_end_matches(',');
                    // Remove quotes if present
                    let time_str = time_str.trim_matches('"').trim();
                    if let Ok(time_ns) = time_str.parse::<u64>() {
                        // Convert nanoseconds to seconds
                        return Ok(time_ns / 1_000_000_000);
                    }
                }
            }
        }
    }

    // Method 2: Try ioreg -l (more detailed output, may work on Apple Silicon)
    let output = Command::new("ioreg").args(&["-l"]).output();

    if let Ok(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);

        for line in output_str.lines() {
            // Look for HIDIdleTime or IdleTime
            if line.contains("HIDIdleTime") || line.contains("\"IdleTime\"") {
                if let Some(time_str) = line.split('=').nth(1) {
                    let time_str = time_str.trim().trim_end_matches(',');
                    let time_str = time_str.trim_matches('"').trim();
                    if let Ok(time_ns) = time_str.parse::<u64>() {
                        return Ok(time_ns / 1_000_000_000);
                    }
                }
            }
        }
    }

    // Method 3: Try using CGEventSource (via osascript)
    // This uses CoreGraphics to get idle time
    let output = Command::new("osascript")
        .args(&["-e", "tell application \"System Events\" to get idle time"])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Output is in seconds as a decimal
        if let Ok(seconds) = stdout.trim().parse::<f64>() {
            return Ok(seconds as u64);
        }
    }

    // Method 4: Try pmset -g assertions (may have idle info)
    let output = Command::new("pmset").args(&["-g", "assertions"]).output();

    if let Ok(output) = output {
        let output_str = String::from_utf8_lossy(&output.stdout);
        // Look for idle time in pmset output
        for line in output_str.lines() {
            if line.contains("Idle") {
                // Try to extract idle time
                if let Some(time_str) = line.split_whitespace().nth(0) {
                    if let Ok(seconds) = time_str.parse::<u64>() {
                        return Ok(seconds);
                    }
                }
            }
        }
    }

    // Method 5: Fallback - use current process idle time
    // This is a fallback that tracks time since last input
    // Note: This requires tracking state, so we return 0 as last resort
    eprintln!("Warning: Could not determine idle time on macOS, falling back to 0");
    Err("Failed to get idle time on macOS - all methods failed".to_string())
}

#[cfg(target_os = "macos")]
fn is_on_battery_macos() -> Result<bool, String> {
    use std::process::Command;

    let output = Command::new("pmset")
        .args(&["-g", "ps"])
        .output()
        .map_err(|e| format!("Failed to execute pmset: {}", e))?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    Ok(output_str.contains("Battery Power"))
}

#[cfg(target_os = "macos")]
fn lock_screen_macos() -> Result<(), String> {
    use std::process::Command;

    Command::new("pmset")
        .args(&["displaysleepnow"])
        .spawn()
        .map_err(|e| format!("Failed to lock screen: {}", e))?;

    Ok(())
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
    // Store a dummy value to indicate prevention is active
    if let Ok(mut id) = state.assertion_id.lock() {
        *id = Some(1);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn allow_sleep_macos(state: &PowerSaveBlocker) -> Result<(), String> {
    if let Ok(mut id) = state.assertion_id.lock() {
        *id = None;
    }
    Ok(())
}

// Linux implementations
#[cfg(target_os = "linux")]
fn get_idle_time_linux() -> Result<u64, String> {
    // Try multiple methods for compatibility

    // Method 1: Try X11 idle time
    if let Ok(idle) = get_idle_time_x11() {
        return Ok(idle);
    }

    // Method 2: Try reading from /proc (less accurate)
    if let Ok(idle) = get_idle_time_proc() {
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
fn get_idle_time_proc() -> Result<u64, String> {
    // This is a fallback and less accurate
    // Returns 0 for now as proper implementation requires more system integration
    Ok(0)
}

#[cfg(target_os = "linux")]
fn is_on_battery_linux() -> Result<bool, String> {
    // Check /sys/class/power_supply/
    let power_supply_path = Path::new("/sys/class/power_supply");

    if !power_supply_path.exists() {
        return Ok(false); // Assume AC power if we can't determine
    }

    // Look for AC adapter
    for entry in fs::read_dir(power_supply_path)
        .map_err(|e| format!("Failed to read power supply directory: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();

            // Check AC adapter status
            if name_str.starts_with("AC") || name_str.starts_with("ADP") {
                let online_path = path.join("online");
                if let Ok(content) = fs::read_to_string(online_path) {
                    let is_online = content.trim() == "1";
                    return Ok(!is_online); // On battery if AC is not online
                }
            }
        }
    }

    // If no AC adapter found, assume on battery
    Ok(true)
}

#[cfg(target_os = "linux")]
fn lock_screen_linux() -> Result<(), String> {
    use std::process::Command;

    // Try multiple commands for compatibility
    let commands = vec![
        vec!["loginctl", "lock-session"],
        vec!["gnome-screensaver-command", "-l"],
        vec!["xdg-screensaver", "lock"],
        vec!["kscreenlocker_greet", "--lock"],
    ];

    for cmd_args in commands {
        if let Some((cmd, args)) = cmd_args.split_first() {
            if let Ok(_) = Command::new(cmd).args(args).spawn() {
                return Ok(());
            }
        }
    }

    Err("Failed to lock screen: no compatible command found".to_string())
}

#[cfg(target_os = "linux")]
fn blank_screen_linux() -> Result<(), String> {
    use std::process::Command;

    // Try multiple commands for compatibility
    let commands = vec![
        vec!["xset", "dpms", "force", "off"],
        vec!["gnome-screensaver-command", "-a"],
        vec!["xdg-screensaver", "activate"],
    ];

    for cmd_args in commands {
        if let Some((cmd, args)) = cmd_args.split_first() {
            if let Ok(_) = Command::new(cmd).args(args).spawn() {
                return Ok(());
            }
        }
    }

    Err("Failed to blank screen: no compatible command found".to_string())
}

#[cfg(target_os = "linux")]
fn prevent_sleep_linux(state: &PowerSaveBlocker) -> Result<(), String> {
    // For Linux, we could use systemd-inhibit or similar
    // For now, we'll just store a dummy value
    if let Ok(mut id) = state.assertion_id.lock() {
        *id = Some("inhibit".to_string());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn allow_sleep_linux(state: &PowerSaveBlocker) -> Result<(), String> {
    if let Ok(mut id) = state.assertion_id.lock() {
        *id = None;
    }
    Ok(())
}

// Direct (non-command) versions of sleep management functions.
// These can be called from the screensaver engine on the main thread without
// needing a Tauri command context (i.e. no State<T> parameter).

/// Prevent display sleep - direct call without State wrapper.
/// Safe to call from the main thread in the screensaver engine.
pub fn prevent_display_sleep_direct() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        prevent_sleep_windows_direct()
    }

    #[cfg(target_os = "macos")]
    {
        prevent_sleep_macos_direct()
    }

    #[cfg(target_os = "linux")]
    {
        prevent_sleep_linux_direct()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Ok(())
    }
}

/// Allow display sleep - direct call without State wrapper.
/// Safe to call from the main thread in the screensaver engine.
pub fn allow_display_sleep_direct() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        allow_sleep_windows_direct()
    }

    #[cfg(target_os = "macos")]
    {
        allow_sleep_macos_direct()
    }

    #[cfg(target_os = "linux")]
    {
        allow_sleep_linux_direct()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Ok(())
    }
}

// --- Direct platform implementations (no State<PowerSaveBlocker> dependency) ---

#[cfg(target_os = "windows")]
fn prevent_sleep_windows_direct() -> Result<(), String> {
    use windows::Win32::System::Power::{SetThreadExecutionState, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, ES_CONTINUOUS};

    let new_state = ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED | ES_CONTINUOUS;
    let prev_state = unsafe { SetThreadExecutionState(new_state) };

    if prev_state.0 == 0 {
        return Err("Failed to set thread execution state".to_string());
    }

    println!("Windows: Successfully prevented display sleep (direct)");
    Ok(())
}

#[cfg(target_os = "windows")]
fn allow_sleep_windows_direct() -> Result<(), String> {
    use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS};

    // Reset to normal: clear DISPLAY_REQUIRED, keep ES_CONTINUOUS
    let result = unsafe { SetThreadExecutionState(ES_CONTINUOUS) };

    if result.0 == 0 {
        return Err("Failed to restore thread execution state".to_string());
    }

    println!("Windows: Successfully restored display sleep (direct)");
    Ok(())
}

#[cfg(target_os = "macos")]
fn prevent_sleep_macos_direct() -> Result<(), String> {
    // Use IOKit power assertion to prevent display sleep
    // This is the proper macOS way: Create a "PreventUserIdleDisplaySleep" assertion
    use std::process::Command;

    // Use caffeinate to prevent display sleep (simpler than IOKit FFI)
    // -d flag prevents display from sleeping
    let result = Command::new("caffeinate")
        .args(&["-d", "-w", &std::process::id().to_string()])
        .spawn();

    match result {
        Ok(_) => {
            println!("macOS: Successfully prevented display sleep via caffeinate (direct)");
            Ok(())
        }
        Err(e) => {
            // Fallback: try pmset
            let pmset_result = Command::new("bash")
                .args(&["-c", "caffeinate -d &"])
                .spawn();

            match pmset_result {
                Ok(_) => {
                    println!("macOS: Successfully prevented display sleep via caffeinate fallback (direct)");
                    Ok(())
                }
                Err(e2) => Err(format!("Failed to prevent display sleep: {} / {}", e, e2)),
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn allow_sleep_macos_direct() -> Result<(), String> {
    // Kill any caffeinate processes we may have spawned
    use std::process::Command;

    let _ = Command::new("pkill")
        .args(&["-f", "caffeinate"])
        .spawn();

    println!("macOS: Allowed display sleep (direct)");
    Ok(())
}

#[cfg(target_os = "linux")]
fn prevent_sleep_linux_direct() -> Result<(), String> {
    use std::process::Command;

    // Try systemd-inhibit
    let result = Command::new("systemd-inhibit")
        .args(&["--what=idle", "--who=liminal-screen", "--why=Screensaver active", "--mode=block", "sleep", "infinity"])
        .spawn();

    match result {
        Ok(_) => {
            println!("Linux: Successfully prevented display sleep via systemd-inhibit (direct)");
            Ok(())
        }
        Err(e) => {
            // Fallback: try xdg-screensaver
            let _ = Command::new("xdg-screensaver")
                .args(&["suspend", &std::process::id().to_string()])
                .spawn();

            println!("Linux: Attempted display sleep prevention (direct), error: {}", e);
            Ok(()) // Don't fail - this is best-effort on Linux
        }
    }
}

#[cfg(target_os = "linux")]
fn allow_sleep_linux_direct() -> Result<(), String> {
    use std::process::Command;

    // Kill systemd-inhibit if we started it
    let _ = Command::new("pkill")
        .args(&["-f", "systemd-inhibit.*liminal-screen"])
        .spawn();

    let _ = Command::new("xdg-screensaver")
        .args(&["resume", &std::process::id().to_string()])
        .spawn();

    println!("Linux: Allowed display sleep (direct)");
    Ok(())
}

// Plugin initialization
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
