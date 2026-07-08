use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ScreensaverState {
    Idle,
    ScreensaverActive,
    DisplayOff,
    Locked,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ScreensaverStatus {
    pub is_active: bool,
    pub is_monitoring: bool,
}

/// Pure state-machine step: given the current idle time, thresholds and state,
/// decide which state (if any) to transition to next.
///
/// Priority: Lock > Display Off > Screensaver Active > Idle (deactivation).
/// Returns None when no transition is needed.
pub fn compute_next_action(
    idle_secs: u64,
    starts_in_secs: u64,
    display_off_secs: u64,
    require_pass_secs: u64,
    current_state: ScreensaverState,
) -> Option<ScreensaverState> {
    // PRIORITY 1: LOCK (security) — only when enabled (require_pass_secs > 0)
    if require_pass_secs > 0 && idle_secs >= require_pass_secs {
        return (current_state != ScreensaverState::Locked).then_some(ScreensaverState::Locked);
    }

    // PRIORITY 2: DISPLAY OFF (power saving)
    if idle_secs >= display_off_secs {
        return (current_state != ScreensaverState::DisplayOff)
            .then_some(ScreensaverState::DisplayOff);
    }

    // PRIORITY 3: SCREENSAVER ACTIVATION (visual) — requires the screensaver
    // window to actually get screen time before display-off kicks in
    if idle_secs >= starts_in_secs
        && current_state == ScreensaverState::Idle
        && starts_in_secs < display_off_secs
    {
        return Some(ScreensaverState::ScreensaverActive);
    }

    // PRIORITY 4: DEACTIVATION (user activity)
    if idle_secs < starts_in_secs && current_state != ScreensaverState::Idle {
        return Some(ScreensaverState::Idle);
    }

    None
}

/// Pure URL builder: appends primitive custom options as query parameters.
/// Nested objects, arrays and null values are skipped. Returns the base URL
/// untouched when there is nothing to append (avoids gratuitous normalization).
pub fn build_saver_url(
    base_url: &str,
    custom_options: &serde_json::Value,
) -> Result<String, String> {
    let map = match custom_options {
        serde_json::Value::Object(map) if !map.is_empty() => map,
        _ => return Ok(base_url.to_string()),
    };

    let mut url: url::Url = base_url
        .parse()
        .map_err(|e| format!("Invalid saver URL '{}': {}", base_url, e))?;

    {
        let mut params = url.query_pairs_mut();
        for (key, value) in map {
            let str_val = match value {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                _ => None, // skip nested objects/arrays/null
            };
            if let Some(val) = str_val {
                params.append_pair(key, &val);
            }
        }
    }

    Ok(url.to_string())
}

#[derive(Clone)]
pub struct ScreensaverEngine {
    is_monitoring: Arc<AtomicBool>,
    state: Arc<Mutex<ScreensaverState>>,
    /// True when a state transition has been dispatched but not yet completed
    /// on the main thread. Prevents duplicate dispatches.
    pending_transition: Arc<AtomicBool>,
}

impl Default for ScreensaverEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreensaverEngine {
    pub fn new() -> Self {
        Self {
            is_monitoring: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(ScreensaverState::Idle)),
            pending_transition: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn get_state(&self) -> ScreensaverState {
        *self.state.lock().unwrap()
    }

    fn set_state(&self, new_state: ScreensaverState) {
        let mut state = self.state.lock().unwrap();
        let old_state = *state;
        *state = new_state;
        println!("State: {:?} → {:?}", old_state, new_state);
    }

    pub fn start_engine<R: tauri::Runtime>(&self, app: AppHandle<R>) -> Result<(), String> {
        if self.is_monitoring.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.is_monitoring.store(true, Ordering::Relaxed);

        let engine = self.clone();
        let app_handle = app.clone();

        std::thread::spawn(move || {
            engine.monitoring_loop(app_handle);
        });

        println!("Screensaver engine started");
        Ok(())
    }

    fn monitoring_loop<R: tauri::Runtime>(&self, app: AppHandle<R>) {
        println!("Screensaver monitoring loop started");

        loop {
            if !self.is_monitoring.load(Ordering::Relaxed) {
                break;
            }

            match self.check_and_manage_screensaver(&app) {
                Ok(_) => {}
                Err(e) => println!("Monitoring error: {}", e),
            }

            std::thread::sleep(Duration::from_secs(1));
        }

        println!("Screensaver monitoring loop stopped");
    }

    fn check_and_manage_screensaver<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<(), String> {
        let idle_time = super::power_monitor::get_system_idle_time()
            .map_err(|e| format!("Failed to get idle time: {}", e))?;

        let state = app.state::<super::AppState>();
        let options = state.options.lock().unwrap();
        let starts_in_seconds = (options.starts_in * 60.0) as u64;
        let display_off_seconds = (options.display_off_in * 60.0) as u64;
        let require_pass_seconds = (options.require_pass_in * 60.0) as u64;
        let run_on_battery = options.run_on_battery;
        drop(options);

        if !run_on_battery {
            match super::power_monitor::is_on_battery_power() {
                Ok(on_battery) => {
                    if on_battery {
                        if self.get_state() != ScreensaverState::Idle
                            && !self.pending_transition.load(Ordering::Relaxed)
                        {
                            self.request_deactivate(app);
                        }
                        return Ok(());
                    }
                }
                Err(e) => println!("Warning: Failed to check battery status: {}", e),
            }
        }

        // Skip all checks if a transition is already in flight
        if self.pending_transition.load(Ordering::Relaxed) {
            return Ok(());
        }

        match compute_next_action(
            idle_time,
            starts_in_seconds,
            display_off_seconds,
            require_pass_seconds,
            self.get_state(),
        ) {
            Some(ScreensaverState::Locked) => self.request_lock(app),
            Some(ScreensaverState::DisplayOff) => self.request_display_off(app),
            Some(ScreensaverState::ScreensaverActive) => self.request_activate(app),
            Some(ScreensaverState::Idle) => self.request_deactivate(app),
            None => {}
        }

        Ok(())
    }

    fn request_activate<R: tauri::Runtime>(&self, app: &AppHandle<R>) {
        self.pending_transition.store(true, Ordering::Relaxed);

        let engine = self.clone();
        let app = app.clone();

        let result = app.clone().run_on_main_thread(move || {
            if let Err(e) = engine.activate_screensaver(&app) {
                eprintln!("Error activating screensaver on main thread: {}", e);
            }
            engine.pending_transition.store(false, Ordering::Relaxed);
        });

        if let Err(e) = result {
            eprintln!("Failed to dispatch activation to main thread: {}", e);
            self.pending_transition.store(false, Ordering::Relaxed);
        }
    }

    fn request_deactivate<R: tauri::Runtime>(&self, app: &AppHandle<R>) {
        self.pending_transition.store(true, Ordering::Relaxed);

        let engine = self.clone();
        let app = app.clone();

        let result = app.clone().run_on_main_thread(move || {
            if let Err(e) = engine.deactivate_screensaver(&app) {
                eprintln!("Error deactivating screensaver on main thread: {}", e);
            }
            engine.pending_transition.store(false, Ordering::Relaxed);
        });

        if let Err(e) = result {
            eprintln!("Failed to dispatch deactivation to main thread: {}", e);
            self.pending_transition.store(false, Ordering::Relaxed);
        }
    }

    fn request_display_off<R: tauri::Runtime>(&self, app: &AppHandle<R>) {
        self.pending_transition.store(true, Ordering::Relaxed);

        let engine = self.clone();
        let app = app.clone();

        let result = app.clone().run_on_main_thread(move || {
            if let Err(e) = engine.transition_to_display_off(&app) {
                eprintln!("Error transitioning to display off: {}", e);
            }
            engine.pending_transition.store(false, Ordering::Relaxed);
        });

        if let Err(e) = result {
            eprintln!("Failed to dispatch display off to main thread: {}", e);
            self.pending_transition.store(false, Ordering::Relaxed);
        }
    }

    fn request_lock<R: tauri::Runtime>(&self, app: &AppHandle<R>) {
        self.pending_transition.store(true, Ordering::Relaxed);

        let engine = self.clone();
        let app = app.clone();

        let result = app.clone().run_on_main_thread(move || {
            let current_state = engine.get_state();
            if current_state == ScreensaverState::ScreensaverActive {
                if let Err(e) = engine.close_all_savers(&app) {
                    eprintln!("Error closing savers before lock: {}", e);
                }
                let _ = super::power_monitor::allow_display_sleep_direct();
            }
            match super::power_monitor::lock_system_direct() {
                Ok(_) => {
                    engine.set_state(ScreensaverState::Locked);
                    let _ = app.emit("screensaver-locked", ());
                }
                Err(e) => {
                    println!("Failed to lock system: {}", e);
                    // Set state anyway to prevent re-triggering on every tick
                    engine.set_state(ScreensaverState::Locked);
                }
            }
            engine.pending_transition.store(false, Ordering::Relaxed);
        });

        if let Err(e) = result {
            eprintln!("Failed to dispatch lock to main thread: {}", e);
            self.pending_transition.store(false, Ordering::Relaxed);
        }
    }

    /// Transition to display-off state. MUST be called on the main thread.
    fn transition_to_display_off<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<(), String> {
        if self.get_state() == ScreensaverState::ScreensaverActive {
            let _ = super::power_monitor::allow_display_sleep_direct();
            self.close_all_savers(app)?;
        }
        match super::power_monitor::blank_screen() {
            Ok(_) => {
                self.set_state(ScreensaverState::DisplayOff);
            }
            Err(e) => println!("Failed to blank display: {}", e),
        }
        Ok(())
    }

    /// Actually activate the screensaver. MUST be called on the main thread.
    pub fn activate_screensaver<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<(), String> {
        if self.get_state() != ScreensaverState::Idle {
            println!("Screensaver not idle, skipping activation");
            return Ok(());
        }

        println!("Activating screensaver (main thread)");

        match super::power_monitor::prevent_display_sleep_direct() {
            Ok(_) => println!("Display sleep prevented"),
            Err(e) => println!("Warning: Failed to prevent display sleep: {}", e),
        }

        let monitors = super::display_manager::get_available_monitors(app.clone())
            .map_err(|e| format!("Failed to get monitors: {}", e))?;

        println!("Found {} monitors", monitors.len());

        for monitor in &monitors {
            println!("Creating saver window for monitor {:?}", monitor);
            self.create_saver_window(app, monitor)?;
        }

        // Stagger fullscreen transitions — macOS allows only one at a time
        let app_fs = app.clone();
        let window_labels: Vec<String> = monitors
            .iter()
            .map(|m| format!("saver-display-{}", m.id))
            .collect();
        tauri::async_runtime::spawn(async move {
            for (i, label) in window_labels.iter().enumerate() {
                if i > 0 {
                    tokio::time::sleep(Duration::from_millis(600)).await;
                }
                let app_handle = app_fs.clone();
                let label_clone = label.clone();
                let _ = app_fs.run_on_main_thread(move || {
                    if let Some(window) = app_handle.get_webview_window(&label_clone) {
                        match window.set_fullscreen(true) {
                            Ok(_) => println!("Set fullscreen for window {}", label_clone),
                            Err(e) => println!(
                                "Warning: Failed to set fullscreen for window {}: {}",
                                label_clone, e
                            ),
                        }
                    }
                });
            }
        });

        self.set_state(ScreensaverState::ScreensaverActive);
        let _ = app.emit("screensaver-started", ());
        println!("Screensaver activated on {} displays", monitors.len());
        Ok(())
    }

    /// Actually deactivate the screensaver. MUST be called on the main thread.
    pub fn deactivate_screensaver<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<(), String> {
        let current_state = self.get_state();
        if current_state == ScreensaverState::Idle {
            println!("Already idle, skipping deactivation");
            return Ok(());
        }

        println!(
            "Deactivating screensaver (main thread), state: {:?}",
            current_state
        );

        if current_state == ScreensaverState::ScreensaverActive {
            match super::power_monitor::allow_display_sleep_direct() {
                Ok(_) => println!("Display sleep allowed"),
                Err(e) => println!("Warning: Failed to allow display sleep: {}", e),
            }
            self.close_all_savers(app)?;
        }
        // For DisplayOff and Locked states: OS handles display/unlock, just reset state

        self.set_state(ScreensaverState::Idle);
        let _ = app.emit("screensaver-ended", ());
        println!("Screensaver deactivated");
        Ok(())
    }

    fn create_saver_window<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        monitor: &super::display_manager::MonitorInfo,
    ) -> Result<(), String> {
        let label = format!("saver-display-{}", monitor.id);

        if app.get_webview_window(&label).is_some() {
            println!("Window {} already exists, skipping", label);
            return Ok(());
        }

        let url = self.get_saver_url(app)?;
        let (instance_id, app_name) = {
            let state = app.state::<super::AppState>();
            let guard = state.options.lock().unwrap();
            (guard.instance_id.clone(), guard.app_name.clone())
        };

        println!(
            "Creating window {} with URL: {} at position ({}, {}) size {}x{}",
            label,
            url,
            monitor.position.x,
            monitor.position.y,
            monitor.size.width,
            monitor.size.height
        );

        let mut builder = tauri::webview::WebviewWindowBuilder::new(
            app,
            label.clone(),
            WebviewUrl::default(), // about:blank
        )
        .title("Screensaver")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(true)
        .initialization_script(super::build_init_script(&instance_id, &app_name));

        let scale = monitor.scale_factor;
        let logical_x = monitor.position.x as f64 / scale;
        let logical_y = monitor.position.y as f64 / scale;
        let logical_width = monitor.size.width as f64 / scale;
        let logical_height = monitor.size.height as f64 / scale;

        builder = builder
            .position(logical_x, logical_y)
            .inner_size(logical_width, logical_height);

        println!(
            "Window {} logical pos ({}, {}) size ({}, {}) scale={}",
            label, logical_x, logical_y, logical_width, logical_height, scale
        );

        let window = builder
            .build()
            .map_err(|e| format!("Failed to create window {}: {}", label, e))?;

        // Configure autoplay BEFORE navigating to the real URL.
        // On macOS, setMediaTypesRequiringUserActionForPlayback must be set
        // before any media content loads.
        super::autoplay_media::configure_autoplay_for_window(&window);

        std::thread::sleep(std::time::Duration::from_millis(50));

        let saver_url: url::Url = url
            .parse()
            .map_err(|e| format!("Invalid saver URL '{}': {}", url, e))?;
        match window.navigate(saver_url) {
            Ok(_) => println!("Navigated {} to {}", label, url),
            Err(e) => println!("Warning: Navigation failed for {}: {}", label, e),
        }

        match window.show() {
            Ok(_) => println!("Showed window {}", label),
            Err(e) => println!("Warning: Failed to show window {}: {}", label, e),
        }

        let state = app.state::<super::AppState>();
        state.active_savers.lock().unwrap().push(label.clone());

        println!("Successfully created saver window: {}", label);
        Ok(())
    }

    fn close_all_savers<R: tauri::Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let state = app.state::<super::AppState>();
        let savers = state.active_savers.lock().unwrap().clone();

        println!("Closing {} saver windows", savers.len());

        // Phase 1: Hide + stop all windows synchronously
        for label in savers.clone() {
            if let Some(window) = app.get_webview_window(&label) {
                match window.hide() {
                    Ok(_) => println!("Hid window {}", label),
                    Err(e) => println!("Failed to hide window {}: {}", label, e),
                }
                super::autoplay_media::stop_webview(&window);
            }
        }

        // Phase 2: Close after delay to allow WebKit pipeline and CoreAudio to drain
        let app_for_close = app.clone();
        let close_labels = savers.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let app_handle = app_for_close.clone();
            let _ = app_for_close.run_on_main_thread(move || {
                for label in close_labels {
                    if let Some(w) = app_handle.get_webview_window(&label) {
                        match w.close() {
                            Ok(_) => println!("Closed window {}", label),
                            Err(e) => println!("Failed to close window {}: {}", label, e),
                        }
                    }
                }
            });
        });

        state.active_savers.lock().unwrap().clear();
        println!("All saver windows queued for close");
        Ok(())
    }

    fn get_saver_url<R: tauri::Runtime>(&self, app: &AppHandle<R>) -> Result<String, String> {
        let state = app.state::<super::AppState>();
        let options = state.options.lock().unwrap();

        let base_url = if options.debug {
            options.saver_url_debug.clone()
        } else {
            options.saver_url.clone()
        };

        build_saver_url(&base_url, &options.custom_options)
    }

    pub fn get_status(&self) -> ScreensaverStatus {
        ScreensaverStatus {
            is_active: self.get_state() == ScreensaverState::ScreensaverActive,
            is_monitoring: self.is_monitoring.load(Ordering::Relaxed),
        }
    }

    pub fn is_active(&self) -> bool {
        self.get_state() == ScreensaverState::ScreensaverActive
    }

    pub fn stop_engine(&self) {
        self.is_monitoring.store(false, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── compute_next_action ──────────────────────────────────────────────────

    #[test]
    fn lock_takes_priority_over_display_off() {
        let next = compute_next_action(120, 30, 60, 90, ScreensaverState::ScreensaverActive);
        assert_eq!(next, Some(ScreensaverState::Locked));
    }

    #[test]
    fn already_locked_does_not_relock_every_tick() {
        let next = compute_next_action(120, 30, 60, 90, ScreensaverState::Locked);
        assert_eq!(next, None);
    }

    #[test]
    fn display_off_takes_priority_over_screensaver() {
        let next = compute_next_action(70, 30, 60, 0, ScreensaverState::Idle);
        assert_eq!(next, Some(ScreensaverState::DisplayOff));
    }

    #[test]
    fn screensaver_activates_when_idle_enough() {
        let next = compute_next_action(40, 30, 60, 0, ScreensaverState::Idle);
        assert_eq!(next, Some(ScreensaverState::ScreensaverActive));
    }

    #[test]
    fn deactivates_on_user_activity() {
        let next = compute_next_action(5, 30, 60, 0, ScreensaverState::ScreensaverActive);
        assert_eq!(next, Some(ScreensaverState::Idle));
    }

    #[test]
    fn deactivates_from_display_off_on_user_activity() {
        let next = compute_next_action(5, 30, 60, 0, ScreensaverState::DisplayOff);
        assert_eq!(next, Some(ScreensaverState::Idle));
    }

    #[test]
    fn no_change_when_idle_but_below_threshold() {
        let next = compute_next_action(20, 30, 60, 0, ScreensaverState::Idle);
        assert_eq!(next, None);
    }

    #[test]
    fn no_change_when_already_in_correct_state() {
        let next = compute_next_action(70, 30, 60, 0, ScreensaverState::DisplayOff);
        assert_eq!(next, None);
    }

    #[test]
    fn screensaver_does_not_activate_when_starts_in_equals_display_off() {
        // starts_in < display_off_in is required for screensaver activation
        let next = compute_next_action(60, 60, 60, 0, ScreensaverState::Idle);
        assert_eq!(next, Some(ScreensaverState::DisplayOff));
    }

    #[test]
    fn lock_disabled_when_require_pass_is_zero() {
        let next = compute_next_action(300, 30, 60, 0, ScreensaverState::ScreensaverActive);
        assert_ne!(next, Some(ScreensaverState::Locked));
    }

    #[test]
    fn screensaver_stays_active_between_thresholds() {
        let next = compute_next_action(45, 30, 60, 0, ScreensaverState::ScreensaverActive);
        assert_eq!(next, None);
    }

    // ── build_saver_url ──────────────────────────────────────────────────────

    #[test]
    fn empty_custom_options_returns_base_url_untouched() {
        let url = build_saver_url("https://example.com/saver", &serde_json::json!({})).unwrap();
        assert_eq!(url, "https://example.com/saver");
    }

    #[test]
    fn non_object_custom_options_returns_base_url() {
        let url = build_saver_url("https://example.com", &serde_json::json!(null)).unwrap();
        assert_eq!(url, "https://example.com");
    }

    #[test]
    fn primitive_custom_options_become_query_params() {
        let custom = serde_json::json!({"theme": "dark", "speed": 2, "loop": true});
        let url = build_saver_url("https://example.com/saver", &custom).unwrap();
        assert!(url.contains("theme=dark"));
        assert!(url.contains("speed=2"));
        assert!(url.contains("loop=true"));
    }

    #[test]
    fn nested_and_null_custom_options_are_skipped() {
        let custom =
            serde_json::json!({"nested": {"a": 1}, "list": [1, 2], "nothing": null, "kept": "yes"});
        let url = build_saver_url("https://example.com/saver", &custom).unwrap();
        assert!(url.contains("kept=yes"));
        assert!(!url.contains("nested"));
        assert!(!url.contains("list"));
        assert!(!url.contains("nothing"));
    }

    #[test]
    fn custom_options_are_appended_to_existing_query() {
        let custom = serde_json::json!({"b": "2"});
        let url = build_saver_url("https://example.com/saver?a=1", &custom).unwrap();
        assert!(url.contains("a=1"));
        assert!(url.contains("b=2"));
    }

    #[test]
    fn query_params_are_url_encoded() {
        let custom = serde_json::json!({"msg": "hello world & more"});
        let url = build_saver_url("https://example.com", &custom).unwrap();
        assert!(url.contains("msg=hello+world+%26+more"));
    }

    #[test]
    fn invalid_base_url_with_custom_options_errors() {
        let custom = serde_json::json!({"a": "1"});
        assert!(build_saver_url("not a url", &custom).is_err());
    }
}
