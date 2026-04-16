use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl};

#[derive(serde::Serialize, Clone, Debug)]
pub struct ScreensaverStatus {
    pub is_active: bool,
    pub is_monitoring: bool,
}

#[derive(Clone)]
pub struct ScreensaverEngine {
    is_monitoring: Arc<AtomicBool>,
    is_active: Arc<AtomicBool>,
    /// True when an activation/deactivation has been requested but not yet processed
    /// on the main thread. Prevents duplicate dispatches.
    pending_transition: Arc<AtomicBool>,
}

impl ScreensaverEngine {
    pub fn new() -> Self {
        Self {
            is_monitoring: Arc::new(AtomicBool::new(false)),
            is_active: Arc::new(AtomicBool::new(false)),
            pending_transition: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_engine<R: tauri::Runtime>(&self, app: AppHandle<R>) -> Result<(), String> {
        if self.is_monitoring.load(Ordering::Relaxed) {
            return Ok(());
        }

        self.is_monitoring.store(true, Ordering::Relaxed);

        let engine = self.clone();
        let app_handle = app.clone();

        // Start background monitoring thread
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
        // Get idle time directly (this is safe to call from a background thread -
        // it only reads system state, does not touch Tauri windows/webviews)
        let idle_time = super::power_monitor::get_system_idle_time()
            .map_err(|e| format!("Failed to get idle time: {}", e))?;

        // Get current options from app state
        // State access via app.state() is thread-safe (uses internal Arc + Mutex)
        let state = app.state::<super::AppState>();
        let options = state.options.lock().unwrap();
        let starts_in_seconds = (options.starts_in * 60.0) as u64;
        let display_off_seconds = (options.display_off_in * 60.0) as u64;
        let run_on_battery = options.run_on_battery;
        drop(options); // Release lock ASAP

        // Check battery status if needed
        if !run_on_battery {
            match super::power_monitor::is_on_battery_power() {
                Ok(on_battery) => {
                    if on_battery {
                        // Don't run screensaver on battery - deactivate if active
                        if self.is_active.load(Ordering::Relaxed) {
                            self.request_deactivate(app);
                        }
                        return Ok(()); // Early return when on battery
                    }
                }
                Err(e) => {
                    println!("Warning: Failed to check battery status: {}", e);
                }
            }
        }

        let currently_active = self.is_active.load(Ordering::Relaxed);

        // Handle activation: idle time exceeded threshold and not yet active
        if idle_time >= starts_in_seconds && !currently_active {
            // Only dispatch if no transition is already pending
            if !self.pending_transition.load(Ordering::Relaxed) {
                println!(
                    "Idle threshold reached ({}s >= {}s), requesting activation",
                    idle_time, starts_in_seconds
                );
                self.request_activate(app);
            } else {
                println!("Activation already pending, skipping dispatch");
            }
        }
        // Handle deactivation: user became active again
        else if idle_time < starts_in_seconds && currently_active {
            // Only dispatch if no transition is already pending
            if !self.pending_transition.load(Ordering::Relaxed) {
                println!(
                    "User activity detected ({}s < {}s), requesting deactivation",
                    idle_time, starts_in_seconds
                );
                self.request_deactivate(app);
            } else {
                println!("Deactivation already pending, skipping dispatch");
            }
        }
        // Handle display blank for extended idle
        else if idle_time >= display_off_seconds && currently_active {
            match super::power_monitor::blank_screen() {
                Ok(_) => println!("Display blanked due to extended idle"),
                Err(e) => println!("Failed to blank display: {}", e),
            }
        }

        Ok(())
    }

    /// Request activation by dispatching to the main thread.
    /// This is the CRITICAL fix: all window operations MUST run on the main thread.
    fn request_activate<R: tauri::Runtime>(&self, app: &AppHandle<R>) {
        // Mark transition as pending to prevent duplicate dispatches
        self.pending_transition.store(true, Ordering::Relaxed);

        let engine = self.clone();
        let app = app.clone();

        // Tauri v2: run_on_main_thread schedules the closure on the main event loop.
        // This is essential because WebviewWindowBuilder::new().build() MUST be called
        // from the main thread - calling it from a background thread silently fails.
        // Clone app before calling run_on_main_thread so the method borrows the clone
        // while the closure moves the original.
        let result = app.clone().run_on_main_thread(move || {
            if let Err(e) = engine.activate_screensaver(&app) {
                eprintln!("Error activating screensaver on main thread: {}", e);
            }
            // Clear pending flag regardless of success/failure
            engine.pending_transition.store(false, Ordering::Relaxed);
        });

        if let Err(e) = result {
            eprintln!("Failed to dispatch activation to main thread: {}", e);
            // Clear pending flag since the dispatch failed
            self.pending_transition.store(false, Ordering::Relaxed);
        }
    }

    /// Request deactivation by dispatching to the main thread.
    fn request_deactivate<R: tauri::Runtime>(&self, app: &AppHandle<R>) {
        // Mark transition as pending to prevent duplicate dispatches
        self.pending_transition.store(true, Ordering::Relaxed);

        let engine = self.clone();
        let app = app.clone();

        let result = app.clone().run_on_main_thread(move || {
            if let Err(e) = engine.deactivate_screensaver(&app) {
                eprintln!("Error deactivating screensaver on main thread: {}", e);
            }
            // Clear pending flag regardless of success/failure
            engine.pending_transition.store(false, Ordering::Relaxed);
        });

        if let Err(e) = result {
            eprintln!("Failed to dispatch deactivation to main thread: {}", e);
            // Clear pending flag since the dispatch failed
            self.pending_transition.store(false, Ordering::Relaxed);
        }
    }

    /// Actually activate the screensaver. MUST be called on the main thread.
    pub fn activate_screensaver<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<(), String> {
        // Double-check under "lock" to prevent double-activation
        if self.is_active.load(Ordering::Relaxed) {
            println!("Screensaver already active, skipping activation");
            return Ok(());
        }

        println!("Activating screensaver (main thread)");

        // Prevent display sleep using the direct platform function
        match super::power_monitor::prevent_display_sleep_direct() {
            Ok(_) => println!("Display sleep prevented"),
            Err(e) => println!("Warning: Failed to prevent display sleep: {}", e),
        }

        // Get monitors - this is safe on the main thread
        let monitors = super::display_manager::get_available_monitors(app.clone())
            .map_err(|e| format!("Failed to get monitors: {}", e))?;

        println!("Found {} monitors", monitors.len());

        // Create all windows first (positioned, visible, but NOT fullscreen yet).
        // This gets every window placed on its correct monitor before any
        // fullscreen transitions begin.
        for monitor in &monitors {
            println!("Creating saver window for monitor {:?}", monitor);
            self.create_saver_window(app, monitor)?;
        }

        // Now stagger the fullscreen transitions. macOS limits one concurrent
        // fullscreen animation at a time — calling set_fullscreen on two windows
        // simultaneously causes the second to fail with the "funk" sound.
        // We use async delays to let each animation complete before starting the next.
        // The macOS fullscreen animation takes ~500ms, so we wait 600ms between each.
        // On other platforms (Windows, Linux) set_fullscreen is typically instant,
        // so this delay is harmless.
        let app_fs = app.clone();
        let window_labels: Vec<String> = monitors
            .iter()
            .map(|m| format!("saver-display-{}", m.id))
            .collect();
        tauri::async_runtime::spawn(async move {
            for (i, label) in window_labels.iter().enumerate() {
                if i > 0 {
                    // Wait for previous fullscreen animation to complete
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

        self.is_active.store(true, Ordering::Relaxed);
        let _ = app.emit("screensaver-started", ());
        println!("Screensaver activated on {} displays", monitors.len());
        Ok(())
    }

    /// Actually deactivate the screensaver. MUST be called on the main thread.
    pub fn deactivate_screensaver<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
    ) -> Result<(), String> {
        if !self.is_active.load(Ordering::Relaxed) {
            println!("Screensaver not active, skipping deactivation");
            return Ok(());
        }

        println!("Deactivating screensaver (main thread)");

        // Allow display sleep using the direct platform function
        match super::power_monitor::allow_display_sleep_direct() {
            Ok(_) => println!("Display sleep allowed"),
            Err(e) => println!("Warning: Failed to allow display sleep: {}", e),
        }

        // Close all saver windows - MUST be on main thread (we are now)
        self.close_all_savers(app)?;

        self.is_active.store(false, Ordering::Relaxed);
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

        // Check if a window with this label already exists (prevent duplicates)
        if app.get_webview_window(&label).is_some() {
            println!("Window {} already exists, skipping", label);
            return Ok(());
        }

        let url = self.get_saver_url(app)?;

        println!(
            "Creating window {} with URL: {} at position ({}, {}) size {}x{}",
            label,
            url,
            monitor.position.x,
            monitor.position.y,
            monitor.size.width,
            monitor.size.height
        );

        // Build window with about:blank FIRST - we'll navigate to the real URL
        // after configuring autoplay. This ensures autoplay is set up before
        // any media content loads.
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
        .focused(true);

        // Build window as a borderless, always-on-top window positioned on the
        // correct monitor. For single-monitor setups, set_fullscreen works fine.
        // For multi-monitor, the caller (activate_screensaver) will stagger
        // fullscreen transitions to avoid macOS's one-at-a-time limitation.
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

        // Navigate to the actual screensaver URL now that autoplay is configured.
        let saver_url: url::Url = url.parse().unwrap();
        match window.navigate(saver_url) {
            Ok(_) => println!("Navigated {} to {}", label, url),
            Err(e) => println!("Warning: Navigation failed for {}: {}", label, e),
        }

        // Show the window — it appears with inner_size covering the monitor.
        match window.show() {
            Ok(_) => println!("Showed window {}", label),
            Err(e) => println!("Warning: Failed to show window {}: {}", label, e),
        }

        // Store reference
        let state = app.state::<super::AppState>();
        state.active_savers.lock().unwrap().push(label.clone());

        println!("Successfully created saver window: {}", label);
        Ok(())
    }

    fn close_all_savers<R: tauri::Runtime>(&self, app: &AppHandle<R>) -> Result<(), String> {
        let state = app.state::<super::AppState>();
        let savers = state.active_savers.lock().unwrap().clone();

        println!("Closing {} saver windows", savers.len());

        // Phase 1: Hide + stop all windows (synchronous, on main thread)
        for label in savers.clone() {
            if let Some(window) = app.get_webview_window(&label) {
                // Hide immediately — user sees desktop right away
                match window.hide() {
                    Ok(_) => println!("Hid window {}", label),
                    Err(e) => println!("Failed to hide window {}: {}", label, e),
                }

                // Multi-layer stop: JS pause + platform-native stopLoading
                super::autoplay_media::stop_webview(&window);
            }
        }

        // Phase 2: Close all windows after a delay. We need ~500ms for:
        // - JS eval to execute and pause all media elements
        // - stopLoading to propagate through WebKit's pipeline
        // - CoreAudio to drain already-buffered audio
        // After close(), the WebContent process should terminate on its own.
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

        if options.debug {
            Ok(options.saver_url_debug.clone())
        } else {
            Ok(options.saver_url.clone())
        }
    }

    // Public methods for status and control
    pub fn get_status(&self) -> ScreensaverStatus {
        ScreensaverStatus {
            is_active: self.is_active.load(Ordering::Relaxed),
            is_monitoring: self.is_monitoring.load(Ordering::Relaxed),
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    pub fn stop_engine(&self) {
        self.is_monitoring.store(false, Ordering::Relaxed);
    }
}
