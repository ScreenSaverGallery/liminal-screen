// Autoplay configuration for saver webviews.
//
// Screensaver content must be able to start audio/video without a user
// gesture, so each platform's webview needs its autoplay policy relaxed
// BEFORE any media content loads.

use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("autoplay")
        .on_webview_ready(|webview| {
            let label = webview.label().to_string();
            println!("Configuring autoplay for window: {}", label);
            if let Err(e) = webview.with_webview(move |pw| apply_autoplay_config(pw, &label)) {
                eprintln!("Failed to configure autoplay: {}", e);
            }
        })
        .build()
}

/// Configure autoplay for a specific webview window. This should be called
/// immediately after creating the window, BEFORE any content loads.
/// The plugin's on_webview_ready callback might fire too late for dynamically
/// created windows.
pub fn configure_autoplay_for_window<R: Runtime>(window: &tauri::webview::WebviewWindow<R>) {
    let label = window.label().to_string();
    let err_label = label.clone();
    if let Err(e) = window.with_webview(move |pw| apply_autoplay_config(pw, &label)) {
        eprintln!(
            "Failed to configure autoplay for window {}: {}",
            err_label, e
        );
    }
}

/// Shared platform implementation used by both the plugin callback and the
/// direct per-window path.
fn apply_autoplay_config(webview: tauri::webview::PlatformWebview, label: &str) {
    #[cfg(target_os = "macos")]
    unsafe {
        // WKWebView: mediaTypesRequiringUserActionForPlayback = WKAudiovisualMediaTypeNone
        let wkwebview = &*(webview.inner() as *mut AnyObject);
        let config: *mut AnyObject = msg_send![wkwebview, configuration];
        let _: () = msg_send![&*config, setMediaTypesRequiringUserActionForPlayback: 0_usize];
        let preferences: *mut AnyObject = msg_send![&*config, preferences];
        let _: () = msg_send![&*preferences, setJavaScriptCanOpenWindowsAutomatically: true];
        println!("macOS autoplay configured for window {}", label);
    }

    #[cfg(target_os = "windows")]
    unsafe {
        // WebView2 exposes no runtime autoplay switch; the autoplay policy is set
        // via --autoplay-policy in additionalBrowserArguments (see the
        // WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS env var set in lib.rs::run).
        // Here we only make sure scripting is enabled.
        if let Ok(core_webview) = webview.controller().CoreWebView2() {
            if let Ok(settings) = core_webview.Settings() {
                let _ = settings.SetIsScriptEnabled(true);
                let _ = settings.SetAreDefaultScriptDialogsEnabled(true);
                println!("Windows autoplay configured for window {}", label);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::{SettingsExt, WebViewExt};

        let wv = webview.inner();
        if let Some(settings) = wv.settings() {
            settings.set_media_playback_requires_user_gesture(false);
            settings.set_enable_media_stream(true);
            settings.set_enable_mediasource(true);
            settings.set_javascript_can_open_windows_automatically(true);
            println!("Linux autoplay configured for window {}", label);
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (webview, label);
        eprintln!("Autoplay configuration not supported on this platform");
    }
}

/// Stop all loading and media playback in a webview using platform-native APIs.
///
/// This uses a layered approach because WebKit's audio pipeline is notoriously
/// hard to kill on macOS — a single method is never enough:
///
/// Layer 1: JavaScript mute+pause — directly silences HTML5 media elements
///          from within the page
/// Layer 2: platform-native stop — stops the webview's network/rendering pipeline
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

    // Layer 2: Platform-native stop — kills the webview's loading/rendering pipeline.
    let err_label = label.clone();
    if let Err(e) = window.with_webview(move |pw| stop_platform_webview(pw, &label)) {
        eprintln!("Failed to stop webview {}: {}", err_label, e);
    }
}

fn stop_platform_webview(webview: tauri::webview::PlatformWebview, label: &str) {
    #[cfg(target_os = "macos")]
    unsafe {
        let wkwebview = &*(webview.inner() as *mut AnyObject);
        let _: () = msg_send![wkwebview, stopLoading];
        println!("macOS: Called [WKWebView stopLoading] on {}", label);
    }

    #[cfg(target_os = "windows")]
    unsafe {
        if let Ok(core_webview) = webview.controller().CoreWebView2() {
            let _ = core_webview.Stop();
            println!("Windows: Called CoreWebView2.Stop() on {}", label);
        }
    }

    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::WebViewExt;

        let wv = webview.inner();
        wv.stop_loading();
        wv.load_uri("about:blank");
        println!(
            "Linux: Stopped loading and navigated {} to about:blank",
            label
        );
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (webview, label);
        eprintln!("stop_webview not supported on this platform");
    }
}
