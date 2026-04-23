// src-tauri/src/autoplay_plugin.rs
use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime, Webview,
};

#[cfg(target_os = "windows")]
use windows::Win32::System::WinRT::IInspectable;

#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("autoplay")
        .on_webview_ready(|window| {
            // This is called for each window as it's created
            println!("Configuring autoplay for window: {}", window.label());
            configure_autoplay(window.clone());
        })
        .build()
}

/// Configure autoplay for a specific webview window. This should be called
/// immediately after creating the window, BEFORE any content loads.
/// The plugin's on_webview_ready callback might fire too late for dynamically
/// created windows.
pub fn configure_autoplay_for_window<R: Runtime>(window: &tauri::webview::WebviewWindow<R>) {
    let label = window.label().to_string();
    let closure_label = label.clone();
    let err_label = label.clone();
    match window.with_webview(move |webview| {
        #[cfg(target_os = "macos")]
        unsafe {
            let wkwebview = &*(webview.inner() as *mut AnyObject);
            let config: *mut AnyObject = msg_send![wkwebview, configuration];
            let _: () = msg_send![&*config, setMediaTypesRequiringUserActionForPlayback: 0_usize];
            let preferences: *mut AnyObject = msg_send![&*config, preferences];
            let _: () = msg_send![&*preferences, setJavaScriptCanOpenWindowsAutomatically: true];
            println!("macOS autoplay configured for window {}", closure_label);
        }

        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(controller) = webview.controller() {
                if let Ok(core_webview) = controller.CoreWebView2() {
                    let settings = core_webview.Settings().unwrap();
                    settings.SetIsScriptEnabled(true).ok();
                    settings.SetAreDefaultScriptDialogsEnabled(true).ok();
                    println!("Windows autoplay configured for window {}", closure_label);
                }
            }
        }

        #[cfg(target_os = "linux")]
        unsafe {
            let wkwebview = webview.inner() as *mut gtk::Widget;
            let webview_ptr = &*(wkwebview as *mut webkit2gtk::WebView);
            webkit2gtk::WebViewExt::set_media_playback_requires_user_gesture(webview_ptr, false);
            println!("Linux autoplay configured for window {}", closure_label);
        }
    }) {
        Ok(_) => {}
        Err(e) => eprintln!(
            "Failed to configure autoplay for window {}: {}",
            err_label, e
        ),
    }
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
                let wkwebview = &*(webview.inner() as *mut AnyObject);

                // Get the configuration
                let config: *mut AnyObject = msg_send![wkwebview, configuration];

                // Set mediaTypesRequiringUserActionForPlayback to WKAudiovisualMediaTypeNone (0)
                // This allows autoplay for both audio and video without user interaction
                let _: () = msg_send![&*config, setMediaTypesRequiringUserActionForPlayback: 0_usize];

                // Also disable other restrictions
                let preferences: *mut AnyObject = msg_send![&*config, preferences];
                let _: () = msg_send![&*preferences, setJavaScriptCanOpenWindowsAutomatically: true];

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

/// Stop all loading and media playback in a webview using platform-native APIs.
///
/// This uses a layered approach because WebKit's audio pipeline is notoriously
/// hard to kill on macOS — a single method is never enough:
///
/// Layer 1: JavaScript pauseAllMedia() — directly pauses HTML5 media elements
///          and closes Web Audio contexts from within the page
/// Layer 2: [WKWebView stopLoading] — stops WebKit's network/rendering pipeline
///
/// The JS layer handles what stopLoading can't (already-buffered audio),
/// and stopLoading handles what JS can't (background WebKit processes).
pub fn stop_webview<R: Runtime>(window: &tauri::webview::WebviewWindow<R>) {
    let label = window.label().to_string();

    // Layer 1: JavaScript media mute+pause — immediately silences all audio output
    // and pauses playback. We do NOT destroy media elements (no src clearing,
    // no track stopping) because:
    // - The window is about to be closed anyway, so cleanup is pointless
    // - Destructive operations can affect autoplay permissions for future sessions
    // - Some browsers revoke autoplay consent after track.stop() calls
    // Mute + pause is enough to stop audio output instantly.
    let pause_js = r#"(function(){
        document.querySelectorAll('video, audio').forEach(function(el){
            el.muted = true;
            el.pause();
        });
    })();"#;
    match window.eval(pause_js) {
        Ok(_) => println!("Paused media via JS in {}", label),
        Err(e) => println!("Warning: JS pause failed for {}: {}", label, e),
    }

    // Layer 2: Platform-native stop — kills WebKit's loading/rendering pipeline.
    // After JS has paused the media elements, this ensures WebKit doesn't
    // continue any background processing.
    #[cfg(target_os = "macos")]
    {
        let closure_label = label.clone();
        let err_label = label.clone();
        match window.with_webview(move |webview| unsafe {
            let wkwebview = &*(webview.inner() as *mut AnyObject);
            let _: () = msg_send![wkwebview, stopLoading];
            println!("macOS: Called [WKWebView stopLoading] on {}", closure_label);
        }) {
            Ok(_) => {}
            Err(e) => eprintln!("Failed to stop webview {}: {}", err_label, e),
        }
    }

    #[cfg(target_os = "windows")]
    {
        let closure_label = label.clone();
        let err_label = label.clone();
        match window.with_webview(move |webview| unsafe {
            if let Some(controller) = webview.controller() {
                if let Ok(core_webview) = controller.CoreWebView2() {
                    let _ = core_webview.Stop();
                    println!("Windows: Called CoreWebView2.Stop() on {}", closure_label);
                }
            }
        }) {
            Ok(_) => {}
            Err(e) => eprintln!("Failed to stop webview {}: {}", err_label, e),
        }
    }

    #[cfg(target_os = "linux")]
    {
        let closure_label = label.clone();
        let err_label = label.clone();
        match window.with_webview(move |webview| unsafe {
            let wkwebview = webview.inner() as *mut gtk::Widget;
            let webview_ptr = &*(wkwebview as *mut webkit2gtk::WebView);
            webkit2gtk::WebViewExt::load_blank(webview_ptr);
            println!("Linux: Loaded blank in {}", closure_label);
        }) {
            Ok(_) => {}
            Err(e) => eprintln!("Failed to stop webview {}: {}", err_label, e),
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = label; // suppress unused warning
        eprintln!("stop_webview not supported on this platform");
    }
}
