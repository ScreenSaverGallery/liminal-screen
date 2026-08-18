// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// WebKitGTK has a history of rendering/DMA-BUF bugs under native Wayland;
// defaulting to X11 (via XWayland on Wayland sessions) avoids them unless the
// user opts back in. Must be set before GTK/webkit2gtk initializes, so this
// runs first thing in main() rather than in liminal_screen_lib::run(). Idle
// detection's X11-vs-Wayland split (power_monitor.rs) keys off
// WAYLAND_DISPLAY/session type, not GDK_BACKEND, so it is unaffected.
#[cfg(target_os = "linux")]
fn set_default_gdk_backend() {
    if std::env::var("GDK_BACKEND").is_err() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    set_default_gdk_backend();

    liminal_screen_lib::run()
}
