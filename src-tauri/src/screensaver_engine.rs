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

#[derive(Clone)]
pub struct ScreensaverEngine {
    is_monitoring: Arc<AtomicBool>,
    state: Arc<Mutex<ScreensaverState>>,
    /// True when a state transition has been dispatched but not yet completed
    /// on the main thread. Prevents duplicate dispatches.
    pending_transition: Arc<AtomicBool>,
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
                        if self.get_state() != ScreensaverState::Idle {
                            if !self.pending_transition.load(Ordering::Relaxed) {
                                self.request_deactivate(app);
                            }
                        }
                        return Ok(());
                    }
                }
                Err(e) => println!("Warning: Failed to check battery status: {}", e),
            }
        }

        let current_state = self.get_state();

        // Skip all checks if a transition is already in flight
        if self.pending_transition.load(Ordering::Relaxed) {
            return Ok(());
        }

        // === PRIORITY 1: LOCK (Security) ===
        if require_pass_seconds > 0 && idle_time >= require_pass_seconds {
            if current_state != ScreensaverState::Locked {
                self.request_lock(app);
            }
            return Ok(());
        }

        // === PRIORITY 2: DISPLAY OFF (Power Saving) ===
        if idle_time >= display_off_seconds && current_state != ScreensaverState::DisplayOff {
            self.request_display_off(app);
            return Ok(());
        }

        // === PRIORITY 3: SCREENSAVER ACTIVATION (Visual) ===
        if idle_time >= starts_in_seconds
            && current_state == ScreensaverState::Idle
            && starts_in_seconds < display_off_seconds
        {
            self.request_activate(app);
            return Ok(());
        }

        // === PRIORITY 4: DEACTIVATION (User Activity) ===
        if idle_time < starts_in_seconds && current_state != ScreensaverState::Idle {
            self.request_deactivate(app);
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
        .initialization_script(&super::build_init_script(&instance_id, &app_name));

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

        if !options.custom_options.is_object() {
            return Ok(base_url);
        }

        let custom = options.custom_options.clone();
        drop(options);

        let mut url: url::Url = base_url
            .parse()
            .map_err(|e| format!("Invalid saver URL: {}", e))?;

        if let serde_json::Value::Object(map) = custom {
            if !map.is_empty() {
                let mut params = url.query_pairs_mut();
                for (key, value) in &map {
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
        }

        Ok(url.to_string())
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
