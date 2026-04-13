// Liminal Screen - Main Application Library
// Integrates all plugins, system tray, and event handling

pub mod autoplay_media;
pub mod display_manager;
pub mod power_monitor;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::WebviewWindowBuilder,
    AppHandle, Emitter, Manager, Runtime, WebviewUrl,
};

/// Screensaver window label prefix
const _SAVER_LABEL_PREFIX: &str = "saver-display-";
/// Options window label
const OPTIONS_LABEL: &str = "options";
/// Main window label
const MAIN_WINDOW_LABEL: &str = "main";

/// Application state
pub struct AppState {
    pub is_screensaver_active: std::sync::Mutex<bool>,
    pub active_savers: std::sync::Mutex<Vec<String>>,
    pub options: std::sync::Mutex<AppOptions>,
}

/// Application options
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AppOptions {
    pub saver_url: String,
    pub saver_url_debug: String,
    pub options_url: String,
    pub starts_in: f64,       // Minutes
    pub display_off_in: f64,  // Minutes
    pub require_pass_in: f64, // Minutes
    pub run_on_battery: bool,
    pub debug: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            saver_url: "https://save.screensaver.gallery".to_string(),
            saver_url_debug: "https://save.screensaver.gallery/debug".to_string(),
            options_url: "http://localhost/dev/projects/ssg/apps/tauri/ssg-tauri-liminal/options/options.html".to_string(),
            starts_in: 0.2,      // 12 seconds for testing
            display_off_in: 1.0, // 1 minute
            require_pass_in: 1.0,
            run_on_battery: false,
            debug: false,
        }
    }
}

/// Initialize the application
fn setup_app<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize app state
    let app_state = AppState {
        is_screensaver_active: std::sync::Mutex::new(false),
        active_savers: std::sync::Mutex::new(Vec::new()),
        options: std::sync::Mutex::new(AppOptions::default()),
    };
    app.manage(app_state);

    // Create system tray
    create_tray(app)?;

    // Get the main window and hide it initially if desired
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        // Window is already created by tauri.conf.json
        let _ = window.set_title("Liminal Screen");
    }

    Ok(())
}

/// Create the system tray
fn create_tray<R: Runtime>(app: &tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items - no Show/Hide since main window is fallback only
    let options_i = MenuItem::with_id(app, "options", "Options", true, None::<&str>)?;
    let preview_i = MenuItem::with_id(app, "preview", "Preview Screensaver", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&options_i, &preview_i, &quit_i])?;

    // Load tray icon
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("No default icon")?;

    // Build tray
    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "options" => {
                let _ = open_options_or_fallback(app);
            }
            "preview" => {
                let _ = preview_screensaver(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Left click opens options (or fallback main window)
                let _ = open_options_or_fallback(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

/// Open the options window (remote URL) or fallback to main window
fn open_options_or_fallback<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    // Get options URL from state
    let options_url = {
        let state = app.state::<AppState>();
        let options = state.options.lock().unwrap();
        options.options_url.clone()
    };

    // Check if options URL is defined and not the default placeholder
    // Allow localhost URLs and non-example.com URLs
    let has_remote_options = !options_url.is_empty()
        && !options_url.contains("example.com")
        && (options_url.starts_with("http://") || options_url.starts_with("https://"));

    if has_remote_options {
        // Open remote options window
        match open_options_window(app, options_url) {
            Ok(()) => Ok(()),
            Err(_e) => {
                // Fallback: show main window
                if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                Ok(())
            }
        }
    } else {
        // Fallback: show main window
        if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
            let _ = window.show();
            let _ = window.set_focus();
        }
        Ok(())
    }
}

/// Open the remote options window
fn open_options_window<R: Runtime>(app: &AppHandle<R>, options_url: String) -> Result<(), String> {
    // Check if options window already exists
    if let Some(window) = app.get_webview_window(OPTIONS_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    // Parse the URL first to catch parsing errors
    let url = options_url
        .parse()
        .map_err(|e| format!("Failed to parse options URL '{}': {}", options_url, e))?;

    // Create options window
    let window = WebviewWindowBuilder::new(app, OPTIONS_LABEL, WebviewUrl::External(url))
        .title("Liminal Screen Options")
        .inner_size(900.0, 600.0)
        .resizable(true)
        .decorations(true)
        .visible(true)
        .build()
        .map_err(|e| format!("Failed to create options window: {}", e))?;

    // Store window reference
    let _ = window.show();

    Ok(())
}

/// Preview the screensaver
fn preview_screensaver<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    // TODO: Implement token validation when security is enabled
    // Emit event to main window to start preview
    app.emit("preview-screensaver", {})
        .map_err(|e| format!("Failed to emit preview event: {}", e))
}

/// Command to open options window (remote URL or fallback)
#[tauri::command]
async fn open_options(app: AppHandle) -> Result<(), String> {
    open_options_or_fallback(&app)
}

/// Command to get app options
/// Command to get app options
#[tauri::command]
fn get_options(state: tauri::State<AppState>) -> Result<AppOptions, String> {
    // TODO: Implement token validation when security is enabled
    let options = state.options.lock().unwrap();
    Ok(options.clone())
}

/// Command to factory reset app options
#[tauri::command]
fn factory_reset_options() -> Result<AppOptions, String> {
    // TODO: Implement token validation when security is enabled
    let default_options = AppOptions::default();
    Ok(default_options)
}

/// Command to set app options
#[tauri::command]
fn set_options(state: tauri::State<AppState>, options: AppOptions) -> Result<(), String> {
    // TODO: Implement token validation when security is enabled
    let mut current = state.options.lock().unwrap();
    *current = options;
    Ok(())
}

/// Command to check if screensaver is active
#[tauri::command]
fn is_screensaver_active(state: tauri::State<AppState>) -> Result<bool, String> {
    let active = state.is_screensaver_active.lock().unwrap();
    Ok(*active)
}

/// Command to get active saver window labels
#[tauri::command]
fn get_active_savers(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    let savers = state.active_savers.lock().unwrap();
    Ok(savers.clone())
}

/// Command to add an active saver
#[tauri::command]
fn add_active_saver(state: tauri::State<AppState>, label: String) -> Result<(), String> {
    let mut savers = state.active_savers.lock().unwrap();
    savers.push(label);
    Ok(())
}

/// Command to clear active savers
#[tauri::command]
fn clear_active_savers(state: tauri::State<AppState>) -> Result<(), String> {
    let mut savers = state.active_savers.lock().unwrap();
    savers.clear();
    Ok(())
}

/// Command to navigate webview to URL (used to stop media)
#[tauri::command]
async fn navigate_webview(app: AppHandle, label: String, url: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.navigate(url.parse().unwrap());
        Ok(())
    } else {
        Err(format!("Window {} not found", label))
    }
}

/// Command to evaluate JavaScript in a webview
#[tauri::command]
async fn evaluate_javascript(
    app: AppHandle,
    label: String,
    script: String,
) -> Result<String, String> {
    if let Some(window) = app.get_webview_window(&label) {
        let _result = window.eval(&script).map_err(|e| e.to_string())?;
        Ok("Executed".to_string())
    } else {
        Err(format!("Window {} not found", label))
    }
}

// Acquire application-level power management blocker
#[tauri::command]
async fn acquire_app_power_blocker<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<u32, String> {
    // Use the existing power monitor command through invoke
    // This will call the public prevent_display_sleep function
    Ok(1) // Return a simple blocker ID
}

/// Release application-level power management blocker
#[tauri::command]
async fn release_app_power_blocker<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
) -> Result<(), String> {
    // This would call allow_display_sleep when implemented
    Ok(())
}

/// Open devtools by invoke this command
#[tauri::command]
fn open_devtools(_: tauri::Window) {
    // window.open_devtools();
    // app.get_window("main").unwrap().open_devtools();
}

/// Main entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(power_monitor::init())
        .plugin(display_manager::init())
        .plugin(autoplay_media::init())
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            open_devtools,
            get_options,
            set_options,
            factory_reset_options,
            evaluate_javascript,
            open_options,
            navigate_webview,
            is_screensaver_active,
            add_active_saver,
            clear_active_savers,
            get_active_savers,
            acquire_app_power_blocker,
            release_app_power_blocker,
            power_monitor::get_system_idle_time,
            power_monitor::get_system_idle_state,
            power_monitor::is_on_battery_power,
            power_monitor::lock_screen,
            power_monitor::blank_screen,
            power_monitor::prevent_display_sleep,
            power_monitor::allow_display_sleep,
            display_manager::get_available_monitors,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
