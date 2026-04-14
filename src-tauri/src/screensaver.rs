// src-tauri/src/screensaver.rs
// Simple screensaver plugin placeholder

use tauri::{AppHandle, Manager, Runtime};

// Plugin initialization
pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("screensaver")
        .setup(|_app, _api| {
            println!("Screensaver plugin initialized");
            Ok(())
        })
        .build()
}