// Liminal Screen - Main Application Library
// Integrates all plugins, system tray, and event handling

pub mod autoplay_media;
pub mod display_manager;
pub mod notification_service;
pub mod power_monitor;
pub mod screensaver_engine;
pub mod speech;
pub mod updater;

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

/// Preview window label (singleton, reused across previews)
const PREVIEW_LABEL: &str = "preview";
/// Options window label
const OPTIONS_LABEL: &str = "options";
/// Main window label
const MAIN_WINDOW_LABEL: &str = "main";
/// Store key: the user's OS screensaver idle timeout, saved when Liminal
/// disables it so it can be restored. Presence also means "Liminal disabled it".
const OS_SCREENSAVER_PREV_KEY: &str = "osScreensaverPrevIdle";
/// Store key: set once the first-run options window has been shown.
const ONBOARDED_KEY: &str = "onboarded";

/// Read a VITE_* setting: runtime environment first (dev, where dotenv loads
/// ../.env), then the value baked in at compile time (release builds — a
/// bundled app launched from Finder/Explorer has no VITE_* vars in its
/// runtime environment, so `std::env::var` alone silently loses the fork
/// identity in production).
macro_rules! env_setting {
    ($name:literal) => {
        std::env::var($name)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| option_env!($name).map(String::from))
    };
}

/// Load persisted options from the store, falling back to env var defaults.
/// This ensures the backend uses user-saved preferences, not just .env defaults.
fn load_persisted_options<R: Runtime>(
    app: &tauri::App<R>,
) -> Result<AppOptions, Box<dyn std::error::Error>> {
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
    if let Some(notifications_enabled) = store.get("notificationsEnabled") {
        if let Some(val) = notifications_enabled.as_bool() {
            options.notifications_enabled = val;
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
    /// Bumped every time a pooled window is parked or shown, keyed by label.
    /// A deferred park can compare the epoch it captured against the current
    /// one to tell whether the window has been reused since (see
    /// `park_webview_window`).
    pub window_epochs: std::sync::Mutex<std::collections::HashMap<String, u64>>,
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
    // Notifications — env only, never persisted. Empty URL disables the feature.
    #[serde(default)]
    pub notification_url: String,
    #[serde(default = "default_notification_interval")]
    pub notification_check_interval_secs: u64,
    // User consent for notifications — persisted, user-settable, opt-in.
    // No notification is ever shown while this is false.
    #[serde(default)]
    pub notifications_enabled: bool,
    // Start at login — mirrors the OS login-item state, which is the source
    // of truth (never persisted to options.json). Synced from the OS at
    // startup and after every set_options/factory_reset. The env default
    // only applies on first install.
    #[serde(default)]
    pub autostart: bool,
}

fn default_notification_interval() -> u64 {
    3600
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            saver_url: env_setting!("VITE_SAVER_URL").unwrap_or_else(|| "about:blank".to_string()),
            saver_url_debug: env_setting!("VITE_SAVER_URL_DEBUG")
                .unwrap_or_else(|| "about:blank".to_string()),
            options_url: env_setting!("VITE_OPTIONS_URL").unwrap_or_default(),
            app_name: env_setting!("VITE_APP_NAME").unwrap_or_else(|| "Liminal Screen".to_string()),
            app_description: env_setting!("VITE_APP_DESCRIPTION").unwrap_or_default(),
            starts_in: env_setting!("VITE_DEFAULT_STARTS_IN")
                .and_then(|s| s.parse().ok())
                .unwrap_or(4.0),
            display_off_in: env_setting!("VITE_DEFAULT_DISPLAY_OFF_IN")
                .and_then(|s| s.parse().ok())
                .unwrap_or(8.0),
            require_pass_in: env_setting!("VITE_DEFAULT_REQUIRE_PASS_IN")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            run_on_battery: env_setting!("VITE_DEFAULT_RUN_ON_BATTERY")
                .map(|s| s == "true")
                .unwrap_or(false),
            debug: env_setting!("VITE_DEFAULT_DEBUG")
                .map(|s| s == "true")
                .unwrap_or(false),
            custom_options: serde_json::Value::Object(serde_json::Map::new()),
            instance_id: uuid::Uuid::new_v4().to_string(),
            notification_url: env_setting!("VITE_NOTIFICATION_URL").unwrap_or_default(),
            notification_check_interval_secs: env_setting!("VITE_NOTIFICATION_CHECK_INTERVAL_SECS")
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600),
            // Opt-in by default: the user must consent in the options window
            // before any feed notification is shown
            notifications_enabled: env_setting!("VITE_DEFAULT_NOTIFICATIONS_ENABLED")
                .map(|s| s == "true")
                .unwrap_or(false),
            // Screensaver apps live in the tray — start at login unless the
            // fork opts out. Applied on first install only (see setup_app).
            autostart: env_setting!("VITE_DEFAULT_AUTOSTART")
                .map(|s| s == "true")
                .unwrap_or(true),
        }
    }
}

/// Initialize the application
fn setup_app<R: Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    // Load persisted options from store, falling back to env var defaults
    let mut options = load_persisted_options(app).unwrap_or_else(|e| {
        eprintln!(
            "[store] Warning: Could not load persisted options, using defaults: {}",
            e
        );
        AppOptions::default()
    });

    // Persist instanceId on first run (default() generated a new one; save it so it survives restarts)
    let mut first_run = false;
    if let Ok(store) = app.store("options.json") {
        if store.get("instanceId").is_none() {
            first_run = true;
            store.set("instanceId", options.instance_id.clone());
            let _ = store.save();
        }
    }

    // Start-at-login: apply the env default on first install only, then adopt
    // whatever the OS reports — the login item is the source of truth, so
    // changes made in System Settings are picked up here on every launch.
    // Dev builds never auto-register (that would install the debug binary as
    // a login item), but the options-window toggle still works there.
    {
        use tauri_plugin_autostart::ManagerExt;
        let autolaunch = app.autolaunch();
        if first_run && options.autostart && !cfg!(debug_assertions) {
            if let Err(e) = autolaunch.enable() {
                eprintln!("[autostart] Warning: could not enable start at login: {e}");
            }
        }
        options.autostart = autolaunch.is_enabled().unwrap_or(false);
    }

    // Accessory activation policy is required for the saver to appear over
    // another app's full-screen Space, which is where most users are most of the
    // time. A Regular (Dock-visible) app is a full participant in activation, so
    // showing a window is an activation request — and macOS answers that from
    // inside someone else's full-screen Space by switching Spaces or refusing,
    // neither of which puts a saver on screen. Accessory apps float over the
    // active Space instead of competing for it, which is how menu-bar utilities
    // work; this app already lives in the tray, so it loses nothing but the Dock
    // icon. Verified by bisection: nothing else (window level, collection
    // behavior, orderFrontRegardless) substitutes for it.
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    // Initialize app state with loaded options
    let app_state = AppState {
        active_savers: std::sync::Mutex::new(Vec::new()),
        options: std::sync::Mutex::new(options),
        window_epochs: std::sync::Mutex::new(std::collections::HashMap::new()),
    };
    app.manage(app_state);

    // Create system tray
    create_tray(app)?;

    // Get the main window and hide it initially if desired
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        // Window is already created by tauri.conf.json
        let title = {
            let state = app.state::<AppState>();
            let options = state.options.lock().unwrap();
            options.app_name.clone()
        };
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

    // Spawn update checker in background
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = updater::update_silent(handle).await {
            eprintln!("[updater] Error: {}", e);
        }
    });

    // Start remote notification feed polling (exits immediately when no URL is configured)
    notification_service::start_notification_service(app.handle().clone());

    // First-run onboarding: surface the options window once so the user discovers
    // the settings and the screensaver-conflict prompt. Gated on a persisted flag
    // so login-autostart launches stay silent; factory_reset clears it, so a reset
    // makes the next launch behave like a fresh install.
    let onboarded = app
        .store("options.json")
        .ok()
        .and_then(|s| s.get(ONBOARDED_KEY))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !onboarded {
        if let Err(e) = open_options_or_fallback(app.handle()) {
            eprintln!("[onboarding] Could not open options on first run: {}", e);
        }
        if let Ok(store) = app.store("options.json") {
            store.set(ONBOARDED_KEY, true);
            let _ = store.save();
        }
    }

    Ok(())
}

/// Create the system tray
fn create_tray<R: Runtime>(app: &tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    let app_name = {
        let state = app.state::<AppState>();
        let options = state.options.lock().unwrap();
        options.app_name.clone()
    };

    // Create menu items - no Show/Hide since main window is fallback only
    let options_i = MenuItem::with_id(app, "options", "Options", true, None::<&str>)?;
    let preview_i = MenuItem::with_id(app, "preview", "Preview Screensaver", true, None::<&str>)?;
    let check_updates_i = MenuItem::with_id(
        app,
        "check-updates",
        "Check for Updates",
        true,
        None::<&str>,
    )?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&options_i, &preview_i, &check_updates_i, &quit_i])?;

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
            "check-updates" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = updater::check_update(handle).await {
                        eprintln!("[updater] Manual check failed: {}", e);
                    }
                });
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

/// Fragment marker carrying the live options payload. See `inject_options_payload`.
const OPTIONS_FRAGMENT_PREFIX: &str = "#__liminal=";

/// Attach the current options to a URL as a fragment payload so that a pooled
/// window reports fresh values on every navigation.
///
/// A webview keeps the `initialization_script` it was built with, and there is no
/// way to replace that on a live webview. Since windows are now pooled rather
/// than recreated (see `park_webview_window`), the snapshot baked in at creation
/// would otherwise be what `navigator.liminalScreen` reports for the rest of the
/// session — stale the moment the user saves an option. The init script re-reads
/// this payload on every navigation instead, and strips it before the page's own
/// scripts run.
///
/// The fragment is used rather than a query parameter because fragments are not
/// sent to the server: the payload carries the instance UUID and every setting,
/// which the page itself is already trusted with but the saver's host is not.
///
/// Any fragment the URL already had is carried inside the payload and restored
/// when the init script strips ours, so a page's own hash routing is unaffected.
pub fn inject_options_payload(url: &str, options: &AppOptions) -> Result<String, String> {
    let mut parsed: url::Url = url
        .parse()
        .map_err(|e| format!("Invalid URL '{}': {}", url, e))?;

    let payload = serde_json::json!({ "o": options, "f": parsed.fragment() });
    let json = serde_json::to_string(&payload)
        .map_err(|e| format!("Failed to serialize options payload: {}", e))?;

    // Encode to `[A-Za-z0-9%]` only: the result has to survive as a URL fragment
    // and be readable with `decodeURIComponent`, and option values can contain
    // anything — including the `&` and `=` that would otherwise split the payload.
    let encoded =
        percent_encoding::utf8_percent_encode(&json, percent_encoding::NON_ALPHANUMERIC).to_string();

    // `Url::set_fragment` does not escape `%`, so the encoding above is preserved.
    parsed.set_fragment(Some(&format!(
        "{}{}",
        OPTIONS_FRAGMENT_PREFIX.trim_start_matches('#'),
        encoded
    )));
    Ok(parsed.to_string())
}

/// Build the initialization script injected at document-start into every remote window.
/// Sets navigator.id to the instance UUID, appends the app identifier
/// (`LiminalScreen/{version} ({app_name})`) to navigator.userAgent and navigator.appVersion,
/// and exposes the full options snapshot (plus the native `version`) as the frozen
/// navigator.liminalScreen object. Options are embedded as a JSON object literal, so
/// serde handles all string escaping.
///
/// The embedded snapshot is only the fallback: when the URL carries a fragment
/// payload (`inject_options_payload`) the script prefers it, which is what keeps
/// a pooled window's options current. Subframes have no payload of their own and
/// so continue to see the snapshot.
fn build_init_script(options: &AppOptions) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let json = serde_json::to_string(options).unwrap_or_else(|_| "{}".to_string());
    // U+2028/U+2029 are valid JSON but rejected inside JS source by pre-ES2019 parsers
    let json = json
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029");
    format!(
        "(function(){{\
            var o={json};\
            var P='{prefix}';\
            if(location.hash.indexOf(P)===0){{\
                try{{\
                    var d=JSON.parse(decodeURIComponent(location.hash.slice(P.length)));\
                    if(d&&d.o)o=d.o;\
                    try{{history.replaceState(null,'',location.href.split('#')[0]+(d&&d.f?'#'+d.f:''));}}catch(e){{}}\
                }}catch(e){{}}\
            }}\
            o.version='{version}';\
            try{{Object.freeze(o.customOptions);}}catch(e){{}}\
            var ident=' LiminalScreen/{version} ('+o.appName+')';\
            try{{Object.defineProperty(navigator,'id',{{value:o.instanceId,writable:false,configurable:false}});}}catch(e){{}}\
            try{{Object.defineProperty(navigator,'userAgent',{{value:(navigator.userAgent||'')+ident,writable:false,configurable:false}});}}catch(e){{}}\
            try{{Object.defineProperty(navigator,'appVersion',{{value:(navigator.appVersion||'')+ident,writable:false,configurable:false}});}}catch(e){{}}\
            try{{Object.defineProperty(navigator,'liminalScreen',{{value:Object.freeze(o),writable:false,configurable:false}});}}catch(e){{}}\
        }})()",
        json = json,
        prefix = OPTIONS_FRAGMENT_PREFIX,
        version = version
    )
}

/// Core and plugin permissions the remote options page needs.
///
/// App-defined commands (`get_options`, `set_options`, `create_preview_window`, …)
/// are not ACL-gated, but core (`core:`) and plugin commands are — and every
/// capability is scoped to *local* content unless it declares remote URLs. So the
/// grants in `capabilities/options.json` don't apply to a page served over
/// http(s): the call is rejected with
/// `opener.open_url not allowed on window "options" … allowed on: [windows: "options", URL: local]`.
const REMOTE_OPTIONS_PERMISSIONS: [&str; 5] = [
    // liminalAPI.openUrl()
    "opener:allow-open-url",
    // liminalAPI.ask() / showMessage()
    "dialog:allow-ask",
    "dialog:allow-message",
    // liminalAPI.startAutoSync() and the unsubscribe it returns
    "core:event:allow-listen",
    "core:event:allow-unlisten",
];

/// Registered once per process — reopening the options window must not stack
/// duplicate grants onto the runtime authority.
static REMOTE_OPTIONS_GRANT: std::sync::Once = std::sync::Once::new();

/// Grant [`REMOTE_OPTIONS_PERMISSIONS`] to the fork's own options origin.
///
/// The URL comes from `VITE_OPTIONS_URL` and is only known at runtime, so this
/// can't live in a static capability file — forks configure `.env`, not the ACL.
/// The grant is scoped to the options window and to that one origin, and is
/// additive: `add_capability` merges into the compiled ACL rather than replacing it.
///
/// Scoped to the origin rather than the exact URL on purpose — an options page is
/// usually a SPA, so client-side routing would otherwise break IPC after the first
/// navigation. Same-origin pages are equally trusted: the fork controls them all.
fn grant_remote_options_permissions<R: Runtime>(app: &AppHandle<R>, url: &url::Url) {
    let origin = url.origin();
    if !origin.is_tuple() {
        // Opaque origin (file:, data:) — nothing meaningful to scope a grant to.
        return;
    }
    let origin = origin.ascii_serialization();

    REMOTE_OPTIONS_GRANT.call_once(|| {
        let mut capability = tauri::ipc::CapabilityBuilder::new("options-remote-capability")
            // Local content in this window is already covered by options.json.
            .local(false)
            .remote(origin.clone())
            .window(OPTIONS_LABEL);
        for permission in REMOTE_OPTIONS_PERMISSIONS {
            capability = capability.permission(permission);
        }

        match app.add_capability(capability) {
            Ok(()) => println!("[options] Granted remote IPC permissions to {}", origin),
            Err(e) => eprintln!(
                "[options] Warning: could not grant remote IPC permissions to {}: {}. \
                 Dialogs, openUrl() and live option sync will not work on the options page.",
                origin, e
            ),
        }
    });
}

/// How long macOS needs to finish sliding a window out of its fullscreen Space.
/// Only relevant for windows the user fullscreened by hand — saver windows cover
/// the screen without native fullscreen (see `apply_saver_window_level`).
const FULLSCREEN_EXIT_SETTLE_MS: u64 = 800;

/// Bump and return a pooled window's epoch. Called on every park and on every
/// show, so a deferred action can detect that the window was reused since.
pub fn bump_window_epoch<R: Runtime>(window: &tauri::webview::WebviewWindow<R>) -> u64 {
    match window.app_handle().try_state::<AppState>() {
        Some(state) => {
            let mut epochs = state.window_epochs.lock().unwrap();
            let epoch = epochs.entry(window.label().to_string()).or_insert(0);
            *epoch += 1;
            *epoch
        }
        None => 0,
    }
}

fn window_epoch<R: Runtime>(window: &tauri::webview::WebviewWindow<R>) -> u64 {
    window
        .app_handle()
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .window_epochs
                .lock()
                .unwrap()
                .get(window.label())
                .copied()
        })
        .unwrap_or(0)
}

/// Park a webview window instead of destroying it: stop media, blank it, hide it.
///
/// wry deliberately over-retains the WKWebView on drop — `Drop for InnerWebView`
/// calls `webview.retain()` and `manager.retain()` to avoid a use-after-free — so
/// a destroyed webview is never released and its WebKit helper processes stay
/// alive. Every create/destroy cycle adds another set, which is why memory used
/// to climb with each activation. Nothing we do at teardown can fix that, so
/// every window we open more than once is pooled instead: parked when closed,
/// re-navigated and re-shown when needed again. Baseline memory is then a fixed
/// cost (one webview per display, plus preview and options) rather than a leak.
pub fn park_webview_window<R: Runtime>(window: &tauri::webview::WebviewWindow<R>) {
    autoplay_media::stop_webview(window);

    // Blank first so a reused window never flashes the previous content.
    if let Ok(url) = "about:blank".parse() {
        if let Err(e) = window.navigate(url) {
            println!("Warning: Failed to navigate window to about:blank: {}", e);
        }
    }

    let epoch = bump_window_epoch(window);

    // macOS ignores `hide()` on a window that is still in its own fullscreen
    // Space. Saver windows never use native fullscreen, but the preview and
    // options windows are user-resizable and can be fullscreened by hand, so
    // leave fullscreen and retry the hide once the transition has settled.
    let was_fullscreen = window.is_fullscreen().unwrap_or(false);
    if was_fullscreen {
        if let Err(e) = window.set_fullscreen(false) {
            println!("Warning: Failed to exit fullscreen before hide: {}", e);
        }
    }

    if let Err(e) = window.hide() {
        println!("Warning: Failed to hide parked window: {}", e);
    }

    if was_fullscreen {
        let window = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(FULLSCREEN_EXIT_SETTLE_MS)).await;
            // Skip if the window has been shown again in the meantime —
            // otherwise this would hide a window the user just reopened.
            if window_epoch(&window) == epoch {
                let _ = window.hide();
            }
        });
    }
}

/// Read an integer from the environment, accepting `0x`-prefixed hex.
#[cfg(target_os = "macos")]
fn env_num(name: &str) -> Option<isize> {
    let raw = std::env::var(name).ok()?;
    let raw = raw.trim();
    let parsed = match raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        Some(hex) => isize::from_str_radix(hex, 16),
        None => raw.parse(),
    };
    match parsed {
        Ok(v) => {
            println!("[env] {} = {} (0x{:x})", name, v, v);
            Some(v)
        }
        Err(e) => {
            println!("[env] Ignoring {}='{}': {}", name, raw, e);
            None
        }
    }
}

/// macOS: make a saver window cover the screen without native fullscreen.
///
/// Native fullscreen puts each window in its own Space, which costs a ~0.5 s
/// animation each way, permits only one transition at a time, and leaves `hide()`
/// unreliable until the transition settles. Raising the window to the screen-saver
/// level covers the menu bar and the Dock instead, and show/hide is immediate —
/// which is what lets a parked saver be reused without any staggering or delays.
#[cfg(target_os = "macos")]
pub fn apply_saver_window_level<R: Runtime>(window: &tauri::webview::WebviewWindow<R>) {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool};
    use objc2_foundation::NSRect;

    // NSScreenSaverWindowLevel. Enough on its own: a level only orders windows
    // *within* a Space, and once the collection behavior below lets the saver join
    // another app's full-screen Space, that app's own window is at
    // NSNormalWindowLevel (0). Going higher buys nothing and costs something —
    // CGShieldingWindowLevel() (2147483628 on macOS 26) also outranks
    // kCGAssistiveTechHighWindowLevel (1500), so it would occlude VoiceOver,
    // Switch Control and Zoom, which should stay above a screensaver.
    const NS_SCREEN_SAVER_WINDOW_LEVEL: isize = 1000;
    // canJoinAllSpaces | fullScreenAuxiliary.
    //
    // fullScreenAuxiliary (1 << 8) is what allows the window to be shown on the
    // same Space as a full-screen window — without it a saver is confined to the
    // desktop Space and never appears over an app the user has fullscreened,
    // which is most of the time for most people. Its opposite, fullScreenNone
    // (1 << 9), is what a window uses to opt out of fullscreen entirely; setting
    // that here is what made the saver invisible from inside a fullscreen app.
    //
    // Deliberately NOT stationary (1 << 4): documented as making a window behave
    // "like the desktop window", which risks the wallpaper layer.
    const COLLECTION_BEHAVIOR: usize = (1 << 0) | (1 << 8);

    // Both overridable so a display problem can be bisected without a rebuild —
    // e.g. LIMINAL_SAVER_LEVEL=3 for an ordinary floating window, or
    // LIMINAL_SAVER_BEHAVIOR=0 for macOS defaults.
    let level = env_num("LIMINAL_SAVER_LEVEL").unwrap_or(NS_SCREEN_SAVER_WINDOW_LEVEL);
    let behavior = env_num("LIMINAL_SAVER_BEHAVIOR")
        .map(|v| v as usize)
        .unwrap_or(COLLECTION_BEHAVIOR);

    let label = window.label().to_string();
    if let Err(e) = window.with_webview(move |pw| unsafe {
        let wkwebview = &*(pw.inner() as *mut AnyObject);
        let ns_window: *mut AnyObject = msg_send![wkwebview, window];
        if ns_window.is_null() {
            println!("macOS saver level: NSWindow is null for {}", label);
            return;
        }
        let _: () = msg_send![&*ns_window, setLevel: level];
        let _: () = msg_send![&*ns_window, setCollectionBehavior: behavior];

        // Geometry check: a saver that looks wrong at the screen edge is almost
        // always one of these three rects disagreeing. When they match, the
        // window covers the screen and the webview covers the window, so
        // anything visible at the edge is being painted by the webview itself.
        let win_frame: NSRect = msg_send![&*ns_window, frame];
        let content_view: *mut AnyObject = msg_send![&*ns_window, contentView];
        let content_bounds: NSRect = msg_send![&*content_view, bounds];
        let wv_frame: NSRect = msg_send![wkwebview, frame];
        println!(
            "macOS {} frames: window={:?}+{:?} content={:?}+{:?} webview={:?}+{:?}",
            label,
            (win_frame.origin.x, win_frame.origin.y),
            (win_frame.size.width, win_frame.size.height),
            (content_bounds.origin.x, content_bounds.origin.y),
            (content_bounds.size.width, content_bounds.size.height),
            (wv_frame.origin.x, wv_frame.origin.y),
            (wv_frame.size.width, wv_frame.size.height),
        );

        // Deliberately NOT orderFrontRegardless: under the Accessory policy the
        // plain `show()` already puts the saver over the active Space, and it
        // leaves the window key — so keystrokes that dismiss the saver are
        // swallowed rather than delivered to whatever is underneath.
        // NSApplicationActivationPolicy: 0 = Regular, 1 = Accessory, 2 = Prohibited.
        let ns_app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
        let policy: isize = msg_send![ns_app, activationPolicy];
        println!(
            "SAVER CONFIG {}: level={} behavior=0x{:x} activationPolicy={}",
            label,
            level,
            behavior,
            match policy {
                0 => "Regular",
                1 => "Accessory",
                2 => "Prohibited",
                _ => "unknown",
            }
        );

        // Read the compositor's view of the window back. A window that reports
        // visible with correct bounds but shows nothing is either fully
        // transparent, occluded, or hosting a webview that isn't drawing — these
        // tell which.
        let level: isize = msg_send![&*ns_window, level];
        let alpha: f64 = msg_send![&*ns_window, alphaValue];
        let opaque: Bool = msg_send![&*ns_window, isOpaque];
        let visible: Bool = msg_send![&*ns_window, isVisible];
        let on_screen: usize = msg_send![&*ns_window, occlusionState];
        let behavior: usize = msg_send![&*ns_window, collectionBehavior];
        let wv_hidden: Bool = msg_send![wkwebview, isHidden];
        let wv_alpha: f64 = msg_send![wkwebview, alphaValue];
        let wv_superview: *mut AnyObject = msg_send![wkwebview, superview];
        let content_view: *mut AnyObject = msg_send![&*ns_window, contentView];
        println!(
            "macOS {} NSWindow: level={} alpha={} opaque={} visible={} occlusion=0x{:x} behavior=0x{:x} | WKWebView: hidden={} alpha={} attached={} isContentView={}",
            label,
            level,
            alpha,
            opaque.as_bool(),
            visible.as_bool(),
            on_screen,
            behavior,
            wv_hidden.as_bool(),
            wv_alpha,
            !wv_superview.is_null(),
            std::ptr::eq(wv_superview, content_view),
        );
    }) {
        println!(
            "Warning: could not raise {} to saver level: {}",
            window.label(),
            e
        );
    }
}

/// Open the remote options window
fn open_options_window<R: Runtime>(app: &AppHandle<R>, options_url: String) -> Result<(), String> {
    // Snapshot options from state (app identity + instance UUID + everything injected)
    let options = {
        let state = app.state::<AppState>();
        let guard = state.options.lock().unwrap();
        guard.clone()
    };

    // Parse URL and append app identity as query params
    let mut url: url::Url = options_url
        .parse()
        .map_err(|e| format!("Failed to parse options URL '{}': {}", options_url, e))?;
    {
        let mut params = url.query_pairs_mut();
        params.append_pair("appName", &options.app_name);
        if !options.app_description.is_empty() {
            params.append_pair("appDescription", &options.app_description);
        }
    }

    // Must happen before the webview loads, so the page's first IPC call is allowed.
    // Uses the origin only, so it is unaffected by the payload fragment below.
    grant_remote_options_permissions(app, &url);

    // Carry the live options, so a reused options window doesn't report the
    // snapshot baked into its init script at creation.
    let url: url::Url = inject_options_payload(url.as_str(), &options)?
        .parse()
        .map_err(|e| format!("Failed to parse options URL: {}", e))?;

    // Reuse the pooled options window rather than building a second one — see
    // `park_webview_window` for why webviews are never destroyed.
    if let Some(window) = app.get_webview_window(OPTIONS_LABEL) {
        // Only re-navigate a parked window. Reloading one that is already open
        // would discard whatever the user had typed into the form.
        if !window.is_visible().unwrap_or(false) {
            let _ = window.navigate(url);
        }
        bump_window_epoch(&window);
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let options_title = format!("{} Options", options.app_name);
    let window = WebviewWindowBuilder::new(app, OPTIONS_LABEL, WebviewUrl::External(url))
        .title(&options_title)
        .inner_size(900.0, 600.0)
        .resizable(true)
        .decorations(true)
        .visible(true)
        .initialization_script(build_init_script(&options))
        .build()
        .map_err(|e| format!("Failed to create options window: {}", e))?;

    let _ = window.show();

    // Take over the window's own close button so we park (hide + blank) instead
    // of destroying the webview. Destroying leaks the underlying WKWebView process.
    let close_window = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            park_webview_window(&close_window);
        }
    });

    Ok(())
}

/// Preview the screensaver
#[tauri::command]
fn preview_screensaver<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    // TODO: Implement token validation when security is enabled
    // Emit event to main window to start preview
    app.emit("preview-screensaver", ())
        .map_err(|e| format!("Failed to emit preview event: {}", e))
}

/// Command to open options window
#[tauri::command]
async fn open_options(app: AppHandle) -> Result<(), String> {
    open_options_or_fallback(&app)
}

/// Command to close the options window, so a remote options page can offer its
/// own "Close" button (`liminalAPI.closeOptions()`).
///
/// Deliberately an app command rather than the core window API: `window.close()`
/// from a remote page needs `core:window:allow-close` granted to that page's
/// origin, and a denied core command surfaces as nothing happening. App-defined
/// commands aren't ACL-gated, so this works on any fork without widening what a
/// remote page is allowed to do.
///
/// A no-op when the window is already gone. Parks (hides + blanks) the options
/// window so it can be reused instead of leaking a webview process.
#[tauri::command]
async fn close_options<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(OPTIONS_LABEL) {
        app.run_on_main_thread(move || {
            park_webview_window(&window);
        })
        .map_err(|e| format!("Failed to close options window: {}", e))?;
    }
    Ok(())
}

/// Command to get app options
#[tauri::command]
fn get_options(state: tauri::State<AppState>) -> Result<AppOptions, String> {
    // TODO: Implement token validation when security is enabled
    let options = state.options.lock().unwrap();
    Ok(options.clone())
}

/// Command to create or reuse the preview window. Only one preview webview is
/// ever created (label `PREVIEW_LABEL`); it is parked when closed and reused.
/// Created from Rust because the JS `WebviewWindow` API cannot set
/// `initialization_script`, which is how `navigator.id` gets injected.
#[tauri::command]
async fn create_preview_window<R: Runtime>(app: AppHandle<R>, url: String) -> Result<(), String> {
    let options = {
        let state = app.state::<AppState>();
        let guard = state.options.lock().unwrap();
        guard.clone()
    };
    // Carry the live options, so a reused preview window doesn't report the
    // snapshot baked into its init script at creation.
    let parsed_url: url::Url = inject_options_payload(&url, &options)?
        .parse()
        .map_err(|e| format!("Invalid preview URL '{}': {}", url, e))?;

    // Reuse the pooled preview window if it already exists. Unlike options, a
    // preview always re-navigates: the point of it is to restart the saver.
    if let Some(window) = app.get_webview_window(PREVIEW_LABEL) {
        let _ = window.navigate(parsed_url);
        bump_window_epoch(&window);
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let window = WebviewWindowBuilder::new(&app, PREVIEW_LABEL, WebviewUrl::External(parsed_url))
        .title("Screensaver Preview")
        .inner_size(800.0, 600.0)
        .resizable(true)
        .decorations(true)
        .visible(true)
        .always_on_top(false)
        .skip_taskbar(false)
        .initialization_script(build_init_script(&options))
        // Preview loads the same saver content as saver windows — it needs the
        // same speechSynthesis fallback (no-op where the native API exists)
        .initialization_script(speech::POLYFILL_JS)
        .build()
        .map_err(|e| format!("Failed to create preview window: {}", e))?;

    // Park (hide + blank) instead of destroying on close, so the same webview
    // can be reused. Destroying leaks the underlying webview process on macOS.
    let close_app = app.clone();
    let close_window = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            park_webview_window(&close_window);
            let _ = close_app.emit("preview-closed", ());
        }
    });

    Ok(())
}

/// Command to factory reset app options
#[tauri::command]
fn factory_reset_options<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<AppState>,
) -> Result<AppOptions, String> {
    let mut default_options = AppOptions::default();

    // Reset start-at-login to the env default (dev builds never auto-enable,
    // matching first-install behavior in setup_app).
    default_options.autostart =
        apply_autostart(&app, default_options.autostart && !cfg!(debug_assertions));

    let store = app
        .store("options.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;
    // Undo anything Liminal changed on the system so a reset truly matches a
    // fresh install: restore the OS screensaver if we had disabled it. (clear()
    // below also wipes the onboarding flag, so the next launch re-onboards.)
    if let Some(prev) = store.get(OS_SCREENSAVER_PREV_KEY).and_then(|v| v.as_u64()) {
        if prev > 0 {
            if let Err(e) = power_monitor::set_os_screensaver_idle_direct(prev) {
                eprintln!("[reset] Could not restore OS screensaver: {}", e);
            }
        }
    }
    store.clear();
    store.set("instanceId", default_options.instance_id.clone());
    store
        .save()
        .map_err(|e| format!("Failed to save reset: {}", e))?;
    {
        let mut current = state.options.lock().unwrap();
        *current = default_options.clone();
    }
    // Notify all windows (options UI, remote pages via liminal-api)
    let _ = app.emit("reset-options", ());
    let _ = app.emit("options-updated", default_options.clone());
    Ok(default_options)
}

/// Enable/disable the OS login item to match `desired`, returning the actual
/// state afterwards. Failures are logged, not fatal — the returned value always
/// reflects what the OS reports, so callers stay truthful to reality.
fn apply_autostart<R: Runtime>(app: &AppHandle<R>, desired: bool) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    let current = autolaunch.is_enabled().unwrap_or(false);
    if desired != current {
        let result = if desired {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        if let Err(e) = result {
            eprintln!("[autostart] Warning: could not update start at login: {e}");
        }
    }
    autolaunch.is_enabled().unwrap_or(false)
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
fn set_options<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<AppState>,
    options: AppOptions,
) -> Result<(), String> {
    validate_options(&options)?;

    // Non-object custom options are ignored, not persisted
    let custom_options = if options.custom_options.is_object() {
        options.custom_options.clone()
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    // Preserve identity fields — these are fork-controlled via .env, never user-settable
    let mut new_options = {
        let current = state.options.lock().unwrap();
        AppOptions {
            saver_url: current.saver_url.clone(),
            saver_url_debug: current.saver_url_debug.clone(),
            options_url: current.options_url.clone(),
            app_name: current.app_name.clone(),
            app_description: current.app_description.clone(),
            instance_id: current.instance_id.clone(),
            notification_url: current.notification_url.clone(),
            notification_check_interval_secs: current.notification_check_interval_secs,
            custom_options,
            ..options
        }
    };

    // Apply start-at-login to the OS login item (its persistence layer — not
    // options.json). Non-fatal on failure so the other settings still save;
    // reading the state back makes the UI reflect what actually happened.
    new_options.autostart = apply_autostart(&app, new_options.autostart);

    *state.options.lock().unwrap() = new_options.clone();

    let store = app
        .store("options.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;
    store.set("startsIn", new_options.starts_in);
    store.set("displayOffIn", new_options.display_off_in);
    store.set("requirePassIn", new_options.require_pass_in);
    store.set("runOnBattery", new_options.run_on_battery);
    store.set("debug", new_options.debug);
    store.set("notificationsEnabled", new_options.notifications_enabled);
    store.set("customOptions", new_options.custom_options.clone());
    store
        .save()
        .map_err(|e| format!("Failed to save options: {}", e))?;

    // Notify all windows (options UI, remote pages via liminal-api startAutoSync)
    let _ = app.emit("options-updated", new_options);

    Ok(())
}

/// Disable the OS-native screensaver so it can't appear over Liminal. Saves the
/// current timeout first so it can be restored (see `restore_os_screensaver`).
#[tauri::command]
fn disable_os_screensaver<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let status = power_monitor::get_os_screensaver_status()?;
    power_monitor::set_os_screensaver_disabled_direct()?;

    // Remember the prior timeout only when it was actually enabled — this also
    // marks "Liminal disabled it" for the Restore affordance.
    if status.detected {
        if let Some(prev) = status.idle_seconds.filter(|s| *s > 0) {
            if let Ok(store) = app.store("options.json") {
                store.set(OS_SCREENSAVER_PREV_KEY, prev);
                let _ = store.save();
            }
        }
    }
    Ok(())
}

/// Restore the OS-native screensaver to the timeout saved by
/// `disable_os_screensaver`, then clear the saved value.
#[tauri::command]
fn restore_os_screensaver<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let store = app
        .store("options.json")
        .map_err(|e| format!("Failed to open store: {}", e))?;
    let prev = store.get(OS_SCREENSAVER_PREV_KEY).and_then(|v| v.as_u64());

    match prev.filter(|s| *s > 0) {
        Some(secs) => {
            power_monitor::set_os_screensaver_idle_direct(secs)?;
            store.delete(OS_SCREENSAVER_PREV_KEY);
            let _ = store.save();
            Ok(())
        }
        None => Err("No saved system screensaver setting to restore".into()),
    }
}

/// The OS screensaver timeout Liminal saved when it disabled the screensaver,
/// or null if Liminal hasn't disabled it. Drives the "Restore" affordance.
#[tauri::command]
fn get_saved_os_screensaver_idle<R: Runtime>(app: AppHandle<R>) -> Result<Option<u64>, String> {
    Ok(app
        .store("options.json")
        .ok()
        .and_then(|s| s.get(OS_SCREENSAVER_PREV_KEY))
        .and_then(|v| v.as_u64()))
}

/// Return the running application version (compiled from Cargo.toml).
#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
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

/// Return the full screensaver state-machine state (Idle / ScreensaverActive /
/// DisplayOff / Locked). Unlike `get_screensaver_status` (which collapses
/// DisplayOff and Locked into `is_active = false`), this exposes the exact
/// state — the assertion target for E2E tests of the
/// idle → saver → display-off → lock chain.
#[tauri::command]
fn get_screensaver_state(
    state: tauri::State<screensaver_engine::ScreensaverEngine>,
) -> screensaver_engine::ScreensaverState {
    state.get_state()
}

/// Test-only hook: override the idle time fed to the state machine so E2E tests
/// can drive transitions deterministically without waiting real minutes. Pass a
/// number of seconds to fake, or `null` to clear the override and resume real
/// OS idle detection. Combined with `set_options` (small thresholds) and
/// `get_screensaver_state`, this makes the whole chain scriptable.
///
/// Compiled to a no-op error in release builds so it can never ship.
#[cfg(debug_assertions)]
#[tauri::command]
fn debug_set_idle(
    secs: Option<f64>,
    state: tauri::State<screensaver_engine::ScreensaverEngine>,
) -> Result<(), String> {
    state.set_idle_override(secs.map(|s| s.max(0.0) as u64));
    Ok(())
}

#[cfg(not(debug_assertions))]
#[tauri::command]
fn debug_set_idle() -> Result<(), String> {
    Err("debug_set_idle is only available in debug builds".to_string())
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

/// Command to park a webview window (stop media, navigate about:blank, hide).
/// Used by the frontend so it can hide the reusable preview/options windows
/// without going through the core window API permissions.
#[tauri::command]
fn park_webview_window_command(app: AppHandle, label: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(&label) {
        park_webview_window(&window);
        if label == PREVIEW_LABEL {
            let _ = app.emit("preview-closed", ());
        }
        Ok(())
    } else {
        Err(format!("Window '{}' not found", label))
    }
}

/// Command to evaluate JavaScript in a webview
#[tauri::command]
fn evaluate_javascript(app: AppHandle, label: String, script: String) -> Result<String, String> {
    if let Some(window) = app.get_webview_window(&label) {
        window.eval(&script).map_err(|e| e.to_string())?;
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

/// Open devtools for the calling window (the `devtools` Cargo feature is enabled)
#[tauri::command]
fn open_devtools(window: tauri::WebviewWindow) {
    window.open_devtools();
}

/// Command for a user-triggered update check. Emits `update-available` /
/// `update-not-available` and also returns the result directly.
#[tauri::command]
async fn check_for_updates(app: AppHandle) -> Result<Option<updater::UpdateInfo>, String> {
    updater::check_update(app).await.map_err(|e| e.to_string())
}

/// Command to download + install a pending update. Emits progress events and
/// restarts the app when done.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    updater::download_and_install(app)
        .await
        .map_err(|e| e.to_string())
}

/// Main entry point
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load environment variables from .env file (development only)
    init_env();

    // WebView2 has no runtime autoplay switch — the policy must be passed as a
    // browser argument before the first webview is created.
    #[cfg(target_os = "windows")]
    {
        let mut args = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
        if !args.contains("--autoplay-policy") {
            if !args.is_empty() {
                args.push(' ');
            }
            args.push_str("--autoplay-policy=no-user-gesture-required");
            std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", args);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(power_monitor::init())
        .plugin(display_manager::init())
        .plugin(autoplay_media::init())
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            open_devtools,
            check_for_updates,
            install_update,
            get_options,
            set_options,
            factory_reset_options,
            get_app_version,
            create_preview_window,
            evaluate_javascript,
            open_options,
            close_options,
            preview_screensaver,
            navigate_webview,
            park_webview_window_command,
            add_active_saver,
            clear_active_savers,
            get_active_savers,
            acquire_app_power_blocker,
            release_app_power_blocker,
            get_screensaver_status,
            get_screensaver_state,
            debug_set_idle,
            disable_os_screensaver,
            restore_os_screensaver,
            get_saved_os_screensaver_idle,
            activate_screensaver_command,
            deactivate_screensaver_command,
            speech::speak_text,
            speech::cancel_speech,
            speech::speech_synthesis_supported,
            power_monitor::get_system_idle_time,
            power_monitor::get_system_idle_state,
            power_monitor::is_on_battery_power,
            power_monitor::get_os_screensaver_status,
            power_monitor::lock_screen,
            power_monitor::blank_screen,
            power_monitor::prevent_display_sleep,
            power_monitor::allow_display_sleep,
            display_manager::get_available_monitors,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_have_valid_timing() {
        let opts = AppOptions::default();
        assert!(validate_options(&opts).is_ok());
    }

    #[test]
    fn validate_options_rejects_starts_in_too_low() {
        let opts = AppOptions {
            starts_in: 0.05,
            ..AppOptions::default()
        };
        assert!(validate_options(&opts).is_err());
    }

    #[test]
    fn validate_options_rejects_display_off_too_low() {
        let opts = AppOptions {
            display_off_in: 0.4,
            ..AppOptions::default()
        };
        assert!(validate_options(&opts).is_err());
    }

    #[test]
    fn validate_options_rejects_negative_require_pass() {
        let opts = AppOptions {
            require_pass_in: -1.0,
            ..AppOptions::default()
        };
        assert!(validate_options(&opts).is_err());
    }

    #[test]
    fn validate_options_accepts_boundary_values() {
        let opts = AppOptions {
            starts_in: 0.1,
            display_off_in: 0.5,
            require_pass_in: 0.0,
            ..AppOptions::default()
        };
        assert!(validate_options(&opts).is_ok());
    }

    #[test]
    fn validate_options_rejects_values_over_max() {
        let opts = AppOptions {
            starts_in: 1441.0,
            ..AppOptions::default()
        };
        assert!(validate_options(&opts).is_err());
    }

    #[test]
    fn instance_id_is_valid_uuid() {
        let opts = AppOptions::default();
        assert!(uuid::Uuid::parse_str(&opts.instance_id).is_ok());
    }

    #[test]
    fn two_defaults_have_different_instance_ids() {
        let a = AppOptions::default();
        let b = AppOptions::default();
        assert_ne!(a.instance_id, b.instance_id);
    }

    #[test]
    fn options_serialize_to_camel_case() {
        let opts = AppOptions::default();
        let json = serde_json::to_value(&opts).unwrap();
        assert!(json.get("startsIn").is_some());
        assert!(json.get("displayOffIn").is_some());
        assert!(json.get("instanceId").is_some());
        assert!(json.get("notificationUrl").is_some());
    }

    #[test]
    fn notifications_are_opt_in_by_default() {
        // Guard: the default can legitimately be flipped via env/compile-time
        // setting; only assert when the fork has not overridden it.
        let overridden = std::env::var("VITE_DEFAULT_NOTIFICATIONS_ENABLED").is_ok()
            || option_env!("VITE_DEFAULT_NOTIFICATIONS_ENABLED").is_some();
        if !overridden {
            assert!(!AppOptions::default().notifications_enabled);
        }
    }

    #[test]
    fn notifications_consent_defaults_to_false_when_missing_from_payload() {
        // Payloads from older SDKs won't contain the field — consent must
        // never be implicitly granted by deserialization.
        let json = serde_json::to_value(AppOptions::default()).unwrap();
        let mut map = json.as_object().unwrap().clone();
        map.remove("notificationsEnabled");
        let opts: AppOptions = serde_json::from_value(serde_json::Value::Object(map)).unwrap();
        assert!(!opts.notifications_enabled);
    }

    // ── inject_options_payload ───────────────────────────────────────────────

    /// Decode the payload the way the init script does: strip the marker,
    /// percent-decode, parse.
    fn decode_payload(url: &str) -> serde_json::Value {
        let fragment = url::Url::parse(url)
            .unwrap()
            .fragment()
            .expect("no fragment")
            .to_string();
        let encoded = fragment
            .strip_prefix(OPTIONS_FRAGMENT_PREFIX.trim_start_matches('#'))
            .expect("missing payload marker")
            .to_string();
        let decoded = percent_encoding::percent_decode_str(&encoded)
            .decode_utf8()
            .unwrap();
        serde_json::from_str(&decoded).unwrap()
    }

    #[test]
    fn payload_carries_current_options() {
        let mut opts = AppOptions::default();
        opts.instance_id = "uuid-123".to_string();
        opts.starts_in = 7.0;
        let url = inject_options_payload("https://saver.example.com/", &opts).unwrap();

        let payload = decode_payload(&url);
        assert_eq!(payload["o"]["instanceId"], "uuid-123");
        assert_eq!(payload["o"]["startsIn"], 7.0);
        // Nothing was added to the query, so the host never receives the payload
        assert!(url::Url::parse(&url).unwrap().query().is_none());
    }

    #[test]
    fn payload_survives_values_containing_url_delimiters() {
        // `&` and `=` would split the payload if it were not fully encoded
        let mut opts = AppOptions::default();
        opts.app_name = "a&b=c#d?e".to_string();
        opts.custom_options = serde_json::json!({ "k": "v&w=x" });
        let url = inject_options_payload("https://saver.example.com/", &opts).unwrap();

        let payload = decode_payload(&url);
        assert_eq!(payload["o"]["appName"], "a&b=c#d?e");
        assert_eq!(payload["o"]["customOptions"]["k"], "v&w=x");
    }

    #[test]
    fn payload_preserves_a_fragment_the_url_already_had() {
        let opts = AppOptions::default();
        let url = inject_options_payload("https://saver.example.com/?q=1#/route", &opts).unwrap();

        // The original fragment rides inside the payload for the init script to restore
        assert_eq!(decode_payload(&url)["f"], "/route");
        // ...and the query is left exactly as it was
        assert_eq!(url::Url::parse(&url).unwrap().query(), Some("q=1"));
    }

    #[test]
    fn payload_records_absent_fragment_as_null() {
        let opts = AppOptions::default();
        let url = inject_options_payload("https://saver.example.com/", &opts).unwrap();
        assert!(decode_payload(&url)["f"].is_null());
    }

    #[test]
    fn payload_rejects_an_invalid_url() {
        assert!(inject_options_payload("not a url", &AppOptions::default()).is_err());
    }

    #[test]
    fn init_script_prefers_the_url_payload_over_the_baked_snapshot() {
        let script = build_init_script(&AppOptions::default());
        assert!(script.contains("location.hash.indexOf(P)===0"));
        assert!(script.contains("if(d&&d.o)o=d.o;"));
        // ...and hands the page back its own fragment
        assert!(script.contains("history.replaceState"));
    }

    #[test]
    fn init_script_escapes_quotes_and_backslashes_via_json() {
        let mut opts = AppOptions::default();
        opts.app_name = r#"It's "so" \ tricky"#.to_string();
        opts.instance_id = "uuid-123".to_string();
        let script = build_init_script(&opts);
        // serde_json escapes double quotes and backslashes inside the embedded object literal
        assert!(script.contains(r#"It's \"so\" \\ tricky"#));
        assert!(script.contains("uuid-123"));
    }

    #[test]
    fn init_script_contains_version_suffix() {
        let script = build_init_script(&AppOptions::default());
        assert!(script.contains(&format!(
            "' LiminalScreen/{} ('+o.appName+')'",
            env!("CARGO_PKG_VERSION")
        )));
    }

    #[test]
    fn init_script_extends_user_agent_and_app_version() {
        let script = build_init_script(&AppOptions::default());
        assert!(script.contains("Object.defineProperty(navigator,'userAgent'"));
        assert!(script.contains("Object.defineProperty(navigator,'appVersion'"));
        assert!(script.contains("(navigator.userAgent||'')+ident"));
        assert!(script.contains("(navigator.appVersion||'')+ident"));
    }

    #[test]
    fn init_script_exposes_full_options_object() {
        let script = build_init_script(&AppOptions::default());
        assert!(script.contains("Object.defineProperty(navigator,'liminalScreen'"));
        // Every AppOptions field must appear in the embedded JSON (camelCase)
        for key in [
            "saverUrl",
            "saverUrlDebug",
            "optionsUrl",
            "appName",
            "appDescription",
            "startsIn",
            "displayOffIn",
            "requirePassIn",
            "runOnBattery",
            "debug",
            "customOptions",
            "instanceId",
            "notificationUrl",
            "notificationCheckIntervalSecs",
            "notificationsEnabled",
            "autostart",
        ] {
            assert!(script.contains(&format!("\"{}\"", key)), "missing {}", key);
        }
        // Native app version is grafted onto the exposed object
        assert!(script.contains(&format!("o.version='{}'", env!("CARGO_PKG_VERSION"))));
    }
}
