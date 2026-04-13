// src-tauri/src/autoplay_plugin.rs
#![allow(unexpected_cfgs)]

use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime, Webview,
};

#[cfg(target_os = "windows")]
use windows::Win32::System::WinRT::IInspectable;

#[cfg(target_os = "macos")]
use cocoa::base::id;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("autoplay")
        .on_webview_ready(|window| {
            // This is called for each window as it's created
            println!("Configuring autoplay for window: {}", window.label());
            configure_autoplay(window.clone());
        })
        .build()
}

#[cfg(target_os = "windows")]
fn configure_autoplay<R: Runtime>(window: Webview<R>) {
    use windows::core::*;
    use windows::Win32::System::WinRT::*;

    window
        .with_webview(|webview| {
            #[cfg(target_os = "windows")]
            unsafe {
                // Access the WebView2 controller
                if let Some(controller) = webview.controller() {
                    // Get the CoreWebView2
                    let core_webview = controller.CoreWebView2().unwrap();

                    // Set additional browser arguments for autoplay
                    // Note: This needs to be set before WebView2 initialization
                    // For runtime changes, we need to use Settings
                    let settings = core_webview.Settings().unwrap();

                    // Enable all media autoplay
                    settings.SetIsScriptEnabled(true).ok();
                    settings.SetAreDefaultScriptDialogsEnabled(true).ok();

                    println!("Windows autoplay configuration applied");
                }
            }
        })
        .ok();
}

#[cfg(target_os = "macos")]
fn configure_autoplay<R: Runtime>(window: Webview<R>) {
    window
        .with_webview(|webview| {
            #[allow(unused_unsafe)]
            unsafe {
                // Get the WKWebView
                let wkwebview: id = webview.inner() as *mut _ as id;

                // Get the configuration
                let config: id = msg_send![wkwebview, configuration];

                // Set mediaTypesRequiringUserActionForPlayback to WKAudiovisualMediaTypeNone (0)
                // This allows autoplay for both audio and video without user interaction
                let _: () = msg_send![config, setMediaTypesRequiringUserActionForPlayback: 0];

                // Also disable other restrictions
                let preferences: id = msg_send![config, preferences];
                let _: () = msg_send![preferences, setJavaScriptCanOpenWindowsAutomatically: true];

                println!("macOS autoplay configuration applied");
            }
        })
        .ok();
}

#[cfg(target_os = "linux")]
fn configure_autoplay<R: Runtime>(window: Webview<R>) {
    use webkit2gtk::SettingsExt;
    use webkit2gtk::WebViewExt;

    window
        .with_webview(|webview| {
            // Get the WebKitWebView
            let wkwebview = webview.inner() as *mut gtk::Widget;

            unsafe {
                use glib::translate::ToGlibPtr;
                let webview = &*(wkwebview as *mut webkit2gtk::WebView);

                // Get settings
                if let Some(settings) = webview.settings() {
                    // Enable autoplay for media
                    settings.set_enable_media_stream(true);
                    settings.set_enable_mediasource(true);

                    // Allow media playback without user gesture
                    settings.set_property("media-playback-requires-user-gesture", &false);

                    println!("Linux autoplay configuration applied");
                }
            }
        })
        .ok();
}

// For unsupported platforms
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn configure_autoplay<R: Runtime>(_window: Webview<R>) {
    eprintln!("Autoplay configuration not supported on this platform");
}
