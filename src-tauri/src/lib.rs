// Liminal Screen - Main Application Library
// Integrates all plugins, system tray, and event handling

pub mod autoplay_media;
pub mod display_manager;
pub mod power_monitor;
pub mod screensaver_engine;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::WebviewWindowBuilder,
    AppHandle, Emitter, Manager, Runtime, WebviewUrl,
};
use tauri_plugin_store::StoreExt;

/// Initialize environment variables from .env file (development only).
/// Tauri's Rust backend doesn't auto-load .env files, so we use the dotenv crate.
fn init_env() {
    #[cfg(debug_assertions)]
    {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let env_path = std::path::PathBuf::from(manifest_dir).join("../.env");
        
        if let Err(e) = dotenv::from_path(&env_path) {
            eprintln!("[dotenv] Warning: Could not load {:?}: {}", env_path, e);
        }
    }
}

/// Options window label
const OPTIONS_LABEL: &str = "options";
/// Main window label
const MAIN_WINDOW_LABEL: &str = "main";

/// Load persisted options from the store, falling back to env var defaults.
/// This ensures the backend uses user-saved preferences, not just .env defaults.
fn load_persisted_options<R: Runtime>(app: &tauri::App<R>) -> Result<AppOptions, Box<dyn std::error::Error>> {
    // Start with defaults from env vars
    let mut options = AppOptions::default();
    
    // Try to load persisted options from store
    let store = app.store("options.json")?;
    
    // Load each field if present in store, overriding defaults
    if let Some(starts_in) = store.get("startsIn") {
        if let Some(val) = starts_in.as_f64() {
            options.starts_in = val;
        }
    }
    if let Some(display_off_in) = store.get("displayOffIn") {
        if let Some(val) = display_off_in.as_f64() {
            options.display_off_in = val;
        }
    }
    if let Some(require_pass_in) = store.get("requirePassIn") {
        if let Some(val) = require_pass_in.as_f64() {
            options.require_pass_in = val;
        }
    }
    if let Some(run_on_battery) = store.get("runOnBattery") {
        if let Some(val) = run_on_battery.as_bool() {
            options.run_on_battery = val;
        }
    }
    if let Some(debug) = store.get("debug") {
        if let Some(val) = debug.as_bool() {
            options.debug = val;
        }
    }
    
    // Load custom options (JSON blob)
    if let Some(custom) = store.get("customOptions") {
        if custom.is_object() {
            options.custom_options = custom;
        }
    }

    if let Some(instance_id) = store.get("instanceId") {
        if let Some(val) = instance_id.as_str() {
            options.instance_id = val.to_string();
        }
    }
    // URLs, app_name, app_description are never persisted — always from .env
    Ok(options)
}

/// Application state
pub struct AppState {
    pub active_savers: std::sync::Mutex<Vec<String>>,
    pub options: std::sync::Mutex<AppOptions>,
}

/// Application options
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppOptions {
    // Fork identity — env only, never persisted
    pub saver_url: String,
    pub saver_url_debug: String,
    pub options_url: String,
    pub app_name: String,
    pub app_description: String,
    // Mandatory timing — persisted individually
    pub starts_in: f64,       // Minutes
    pub display_off_in: f64,  // Minutes
    pub require_pass_in: f64, // Minutes
    pub run_on_battery: bool,
    pub debug: bool,
    // Custom (fork-defined) — persisted as JSON blob, appended to saver URL as query params
    pub custom_options: serde_json::Value,
    // Auto-generated instance UUID — persisted, regenerated on factory reset, never user-settable
    pub instance_id: String,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            saver_url: std::env::var("VITE_SAVER_URL")
                .unwrap_or_else(|_| "about:blank".to_string()),
            saver_url_debug: std::env::var("VITE_SAVER_URL_DEBUG")
                .unwrap_or_else(|_| "about:blank".to_string()),
            options_url: std::env::var("VITE_OPTIONS_URL").unwrap_or_default(),
            app_name: std::env::var("VITE_APP_NAME")
                .unwrap_or_else(|_| "Liminal Screen".to_string()),
            app_description: std::env::var("VITE_APP_DESCRIPTION").unwrap_or_default(),
            starts_in: std::env::var("VITE_DEFAULT_STARTS_IN")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.2),
            display_off_in: std::env::var("VITE_DEFAULT_DISPLAY_OFF_IN")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0),
            require_pass_in: std::env::var("VITE_DEFAULT_REQUIRE_PASS_IN")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0),
            run_on_battery: std::env::var("VITE_DEFAULT_RUN_ON_BATTERY")
                .map(|s| s == "true")
                .unwrap_or(false),
            debug: std::env::var("VITE_DEFAULT_DEBUG")
                .map(|s| s == "true")
                .unwrap_or(false),
            custom_options: serde_json::Value::Object(serde_json::Map::new()),
            instance_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// Initialize the application
fn setup_app<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    // Load persisted options from store, falling back to env var defaults
    let options = load_persisted_options(app).unwrap_or_else(|e| {
        eprintln!("[store] Warning: Could not load persisted options, using defaults: {}", e);
        AppOptions::default()
    });

    // Persist instanceId on first run (default() generated a new one; save it so it survives restarts)
    if let Ok(store) = app.store("options.json") {
        if store.get("instanceId").is_none() {
            store.set("instanceId", options.instance_id.clone());
            let _ = store.save();
        }
    }

    // Initialize app state with loaded options
    let app_state = AppState {
        active_savers: std::sync::Mutex::new(Vec::new()),
        options: std::sync::Mutex::new(options),
    };
    app.manage(app_state);

    // Create system tray
    create_tray(app)?;

    // Get the main window and hide it initially if desired
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        // Window is already created by tauri.conf.json
        let title = std::env::var("VITE_APP_NAME").unwrap_or_else(|_| "Liminal Screen".to_string());
        let _ = window.set_title(&title);
    }

    // Initialize and start the screensaver engine
    let engine = screensaver_engine::ScreensaverEngine::new();
    app.manage(engine.clone());

    // Start engine immediately - this runs independently of JavaScript context
    if let Err(e) = engine.start_engine(app.handle().clone()) {
        eprintln!("Failed to start screensaver engine: {}", e);
    } else {
        println!("Screensaver engine started successfully");
    }

    Ok(())
}

/// Create the system tray
fn create_tray<R: Runtime>(app: &tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_name = std::env::var("VITE_APP_NAME")
        .unwrap_or_else(|_| "Liminal Screen".to_string());

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
        .tooltip(&app_name)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "options" => {
                let _ = open_options_or_fallback(app);
            }
            "preview" => {
                let _ = preview_screensaver(app.clone());
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

/// Build the initialization script injected at document-start into every remote window.
/// Sets navigator.id to the instance UUID and appends the app identifier to navigator.userAgent.
/// Single quotes in app_name are escaped so the JS string literal is valid.
fn build_init_script(instance_id: &str, app_name: &str) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let safe_name = app_name.replace('\\', "\\\\").replace('\'', "\\'");
    format!(
        "(function(){{\
            try{{Object.defineProperty(navigator,'id',{{value:'{}',writable:false,configurable:false}});}}catch(e){{}}\
            try{{Object.defineProperty(navigator,'userAgent',{{value:navigator.userAgent+' {} LiminalScreen/{}',writable:false,configurable:false}});}}catch(e){{}}\
        }})()",
        instance_id, safe_name, version
    )
}

/// Open the remote options window
fn open_options_window<R: Runtime>(app: &AppHandle<R>, options_url: String) -> Result<(), String> {
    // Check if options window already exists
    if let Some(window) = app.get_webview_window(OPTIONS_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    // Get app identity + instance UUID from state
    let (app_name, app_description, instance_id) = {
        let state = app.state::<AppState>();
        let options = state.options.lock().unwrap();
        (options.app_name.clone(), options.app_description.clone(), options.instance_id.clone())
    };

    // Parse URL and append app identity as query params
    let mut url: url::Url = options_url
        .parse()
        .map_err(|e| format!("Failed to parse options URL '{}': {}", options_url, e))?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("appName", &app_name);
        if !app_description.is_empty() {
            params.append_pair("appDescription", &app_description);
        }
    }

    let options_title = format!("{} Options", app_name);
    let window = WebviewWindowBuilder::new(app, OPTIONS_LABEL, WebviewUrl::External(url))
        .title(&options_title)
        .inner_size(900.0, 600.0)
        .resizable(true)
        .decorations(true)
        .visible(true)
        .initialization_script(&build_init_script(&instance_id, &app_name))
        .build()
        .map_err(|e| format!("Failed to create options window: {}", e))?;

    let _ = window.show();

    Ok(())
}

/// Preview the screensaver
#[tauri::command]
fn preview_screensaver<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    // TODO: Implement token validation when security is enabled
    // Emit event to main window to start preview
    app.emit("preview-screensaver", {})
        .map_err(|e| format!("Failed to emit preview event: {}", e))
}

/// Command to open options window
#[tauri::command]
fn open_options(app: AppHandle) -> Result<(), String> {
    open_options_or_fallback(&app)
}

/// Command to get app options
#[tauri::command]
fn get_options(state: tauri::State<AppState>) -> Result<AppOptions, String> {
    // TODO: Implement token validation when security is enabled
    let options = state.options.lock().unwrap();
    Ok(options.clone())
}


/// Command to create a preview window with navigator.id injected via initialization_script.
/// Must be created from Rust because the JS WebviewWindow API does not expose initializationScript.
#[tauri::command]
fn create_preview_window<R: Runtime>(app: AppHandle<R>, url: String, label: String) -> Result<(), String> {
    if app.get_webview_window(&label).is_some() {
        return Ok(());
    }
    let (instance_id, app_name) = {
        let state = app.state::<AppState>();
        let guard = state.options.lock().unwrap();
        (guard.instance_id.clone(), guard.app_name.clone())
    };
    let parsed_url: url::Url = url
        .parse()
        .map_err(|e| format!("Invalid preview URL '{}': {}", url, e))?;
    WebviewWindowBuilder::new(&app, label, WebviewUrl::External(parsed_url))
        .title("Screensaver Preview")
        .inner_size(800.0, 600.0)
        .resizable(true)
        .decorations(true)
        .visible(true)
        .always_on_top(false)
        .skip_taskbar(false)
        .initialization_script(&build_init_script(&instance_id, &app_name))
        .build()
        .map_err(|e| format!("Failed to create preview window: {}", e))?;
    Ok(())
}


/// Command to factory reset app options
#[tauri::command]
fn factory_reset_options<R: Runtime>(app: AppHandle<R>, state: tauri::State<AppState>) -> Result<AppOptions, String> {
    let default_options = AppOptions::default();

    let store = app.store("options.json").map_err(|e| format!("Failed to open store: {}", e))?;
    store.clear();
    store.set("instanceId", default_options.instance_id.clone());
    store.save().map_err(|e| format!("Failed to save reset: {}", e))?;
    {
        let mut current = state.options.lock().unwrap();
        *current = default_options.clone();
    }
    Ok(default_options)
}

fn validate_options(options: &AppOptions) -> Result<(), String> {
    if options.starts_in < 0.1 || options.starts_in > 1440.0 {
        return Err("startsIn must be between 0.1 and 1440 minutes".into());
    }
    if options.display_off_in < 0.5 || options.display_off_in > 1440.0 {
        return Err("displayOffIn must be between 0.5 and 1440 minutes".into());
    }
    if options.require_pass_in < 0.0 || options.require_pass_in > 1440.0 {
        return Err("requirePassIn must be between 0 and 1440 minutes".into());
    }
    Ok(())
}

/// Command to set app options
#[tauri::command]
fn set_options<R: Runtime>(app: AppHandle<R>, state: tauri::State<AppState>, options: AppOptions) -> Result<(), String> {
    validate_options(&options)?;

    // Preserve identity fields — these are fork-controlled via .env, never user-settable
    let new_options = {
        let current = state.options.lock().unwrap();
        AppOptions {
            saver_url: current.saver_url.clone(),
            saver_url_debug: current.saver_url_debug.clone(),
            options_url: current.options_url.clone(),
            app_name: current.app_name.clone(),
            app_description: current.app_description.clone(),
            instance_id: current.instance_id.clone(),
            ..options.clone()
        }
    };
    *state.options.lock().unwrap() = new_options;

    let store = app.store("options.json").map_err(|e| format!("Failed to open store: {}", e))?;
    store.set("startsIn", options.starts_in);
    store.set("displayOffIn", options.display_off_in);
    store.set("requirePassIn", options.require_pass_in);
    store.set("runOnBattery", options.run_on_battery);
    store.set("debug", options.debug);
    if options.custom_options.is_object() {
        store.set("customOptions", options.custom_options);
    }
    store.save().map_err(|e| format!("Failed to save options: {}", e))?;

    Ok(())
}

/// Command to get screensaver engine status
#[tauri::command]
fn get_screensaver_status(
    state: tauri::State<screensaver_engine::ScreensaverEngine>,
) -> Result<screensaver_engine::ScreensaverStatus, String> {
    Ok(state.get_status())
}

/// Command to manually activate screensaver (for preview/testing).
/// Only activates from Idle state — Tauri commands run on the main thread.
#[tauri::command]
fn activate_screensaver_command<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<screensaver_engine::ScreensaverEngine>,
) -> Result<(), String> {
    if state.get_state() != screensaver_engine::ScreensaverState::Idle {
        return Ok(());
    }
    state.activate_screensaver(&app)
}

/// Command to manually deactivate screensaver.
/// Resets to Idle from any non-Idle state.
#[tauri::command]
fn deactivate_screensaver_command<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<screensaver_engine::ScreensaverEngine>,
) -> Result<(), String> {
    if state.get_state() == screensaver_engine::ScreensaverState::Idle {
        return Ok(());
    }
    state.deactivate_screensaver(&app)
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
fn navigate_webview(app: AppHandle, label: String, url: String) -> Result<(), String> {
    let parsed = url
        .parse()
        .map_err(|e| format!("Invalid URL '{}': {}", url, e))?;
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.navigate(parsed);
        Ok(())
    } else {
        Err(format!("Window '{}' not found", label))
    }
}

/// Command to evaluate JavaScript in a webview
#[tauri::command]
fn evaluate_javascript(app: AppHandle, label: String, script: String) -> Result<String, String> {
    if let Some(window) = app.get_webview_window(&label) {
        let _result = window.eval(&script).map_err(|e| e.to_string())?;
        Ok("Executed".to_string())
    } else {
        Err(format!("Window {} not found", label))
    }
}

/// Acquire application-level power management blocker
#[tauri::command]
fn acquire_app_power_blocker<R: tauri::Runtime>(_app: tauri::AppHandle<R>) -> Result<u32, String> {
    power_monitor::prevent_display_sleep_direct().map(|_| 1)
}

/// Release application-level power management blocker
#[tauri::command]
fn release_app_power_blocker<R: tauri::Runtime>(_app: tauri::AppHandle<R>) -> Result<(), String> {
    power_monitor::allow_display_sleep_direct()
}

/// Open devtools (requires `devtools` Cargo feature + debug build)
#[tauri::command]
fn open_devtools(_window: tauri::Window) {
    // Intentionally left as a stub — enable the `devtools` Cargo feature to implement
}

/// Main entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load environment variables from .env file (development only)
    init_env();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(power_monitor::init())
        .plugin(display_manager::init())
        .plugin(autoplay_media::init())
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            open_devtools,
            get_options,
            set_options,
            factory_reset_options,
            create_preview_window,
            evaluate_javascript,
            open_options,
            preview_screensaver,
            navigate_webview,
            add_active_saver,
            clear_active_savers,
            get_active_savers,
            acquire_app_power_blocker,
            release_app_power_blocker,
            get_screensaver_status,
            activate_screensaver_command,
            deactivate_screensaver_command,
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
