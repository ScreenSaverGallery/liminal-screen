// Notification service — remote feed polling.
//
// There is no push API in the Tauri WebView model, so the app polls a JSON
// feed at VITE_NOTIFICATION_URL and shows new entries as OS notifications.
// Feed format: [{ "id": "...", "title": "...", "body": "...", "url": "..." }]
// Shown entry IDs are persisted in options.json under `shownNotificationIds`
// so each entry is shown at most once per device (until a factory reset).

use std::collections::HashSet;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;

const SHOWN_IDS_KEY: &str = "shownNotificationIds";
/// At most this many notifications are shown per poll; the rest stay unshown
/// and surface on later polls. Prevents a fresh install from flooding the
/// user with the feed's entire history at once.
const MAX_PER_POLL: usize = 5;

#[derive(serde::Deserialize)]
struct NotificationEntry {
    id: String,
    title: String,
    body: String,
    #[allow(dead_code)]
    url: Option<String>,
}

/// Spawn the polling thread. Returns immediately; the thread exits on its own
/// when no notification URL is configured.
pub fn start_notification_service<R: Runtime>(app: AppHandle<R>) {
    std::thread::Builder::new()
        .name("notification-poll".into())
        .spawn(move || polling_loop(app))
        .ok();
}

/// How often to re-check the consent flag while notifications are disabled,
/// so enabling the option in the UI takes effect without an app restart.
const CONSENT_RECHECK_SECS: u64 = 60;

fn polling_loop<R: Runtime>(app: AppHandle<R>) {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .build(),
    );

    loop {
        let (notification_url, interval_secs, enabled) = {
            let state = app.state::<super::AppState>();
            let opts = state.options.lock().unwrap();
            (
                opts.notification_url.clone(),
                opts.notification_check_interval_secs,
                opts.notifications_enabled,
            )
        };

        if notification_url.is_empty() {
            return; // feature not configured by the fork — stop the thread
        }

        // User consent: never fetch or show anything until the user has
        // enabled notifications in the options window. Keep the thread alive
        // and re-check, since consent can be granted at runtime.
        if !enabled {
            std::thread::sleep(std::time::Duration::from_secs(CONSENT_RECHECK_SECS));
            continue;
        }

        if let Err(e) = check_and_notify(&app, &agent, &notification_url) {
            eprintln!("[notifications] Poll error: {}", e);
        }

        std::thread::sleep(std::time::Duration::from_secs(interval_secs.max(60)));
    }
}

fn check_and_notify<R: Runtime>(
    app: &AppHandle<R>,
    agent: &ureq::Agent,
    url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut response = agent.get(url).call()?;
    let feed: Vec<NotificationEntry> = response.body_mut().read_json::<Vec<NotificationEntry>>()?;

    let store = app.store("options.json")?;
    let shown: HashSet<String> = store
        .get(SHOWN_IDS_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let mut newly_shown: Vec<String> = Vec::new();

    for entry in feed.iter().filter(|e| !shown.contains(&e.id)) {
        if newly_shown.len() >= MAX_PER_POLL {
            break;
        }
        show_notification(app, &entry.title, &entry.body);
        newly_shown.push(entry.id.clone());
    }

    if !newly_shown.is_empty() {
        let all: Vec<&String> = shown.iter().chain(newly_shown.iter()).collect();
        store.set(SHOWN_IDS_KEY, serde_json::to_value(&all)?);
        store.save()?;
    }

    Ok(())
}

// notify-rust is used directly instead of tauri-plugin-notification: the
// plugin injects a `window.Notification` shim into every webview, which fires
// a blocked (and console-error-spamming) IPC call from the remote saver and
// preview windows. Notifications here are shown from Rust only, so the JS
// side isn't needed at all. Platform handling below mirrors the plugin's
// desktop backend (which also just wraps notify-rust).
#[cfg_attr(target_os = "linux", allow(unused_variables))]
pub fn show_notification<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    // macOS notifications are attributed to a bundle identifier; in dev
    // (unbundled binary) fall back to Terminal's like the plugin did.
    // set_application only succeeds once per process — later calls error.
    #[cfg(target_os = "macos")]
    let _ = notify_rust::set_application(if tauri::is_dev() {
        "com.apple.Terminal"
    } else {
        &app.config().identifier
    });

    let mut notification = notify_rust::Notification::new();
    notification.summary(title).body(body).auto_icon();

    // Set the notification's System.AppUserModel.ID only when running the
    // installed app — the AUMID isn't registered when run from target/.
    #[cfg(windows)]
    {
        use std::path::MAIN_SEPARATOR as SEP;
        let installed = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.display().to_string()))
            .is_some_and(|dir| {
                !dir.ends_with(&format!("{SEP}target{SEP}debug"))
                    && !dir.ends_with(&format!("{SEP}target{SEP}release"))
            });
        if installed {
            notification.app_id(&app.config().identifier);
        }
    }

    if let Err(e) = notification.show() {
        eprintln!("[notifications] Failed to show notification: {}", e);
    }
}
