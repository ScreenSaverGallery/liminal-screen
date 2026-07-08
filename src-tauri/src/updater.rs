// Liminal Screen - Auto-Updater Module
// Handles automatic update checking, downloading, and installation.
//
// Events emitted (listened to by the options window and liminal-api):
//   update-available         { version, notes }
//   update-not-available     {}
//   update-download-progress { downloaded, total }
//   update-installed         {}

use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

/// Payload for the `update-available` event and the `check_for_updates` command.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// Silent background check run at startup — checks + installs without user
/// interaction. Restarts the application after successful installation.
pub async fn update_silent<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> tauri_plugin_updater::Result<()> {
    if app.updater()?.check().await?.is_some() {
        download_and_install(app).await?;
    }
    Ok(())
}

/// User-triggered check — emits `update-available` with an UpdateInfo payload,
/// or `update-not-available`. Returns the info so callers (command, liminal-api)
/// get the result directly as well.
pub async fn check_update<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> tauri_plugin_updater::Result<Option<UpdateInfo>> {
    match app.updater()?.check().await? {
        Some(update) => {
            let info = UpdateInfo {
                version: update.version.clone(),
                notes: update.body.clone(),
            };
            let _ = app.emit("update-available", info.clone());
            Ok(Some(info))
        }
        None => {
            let _ = app.emit("update-not-available", serde_json::json!({}));
            Ok(None)
        }
    }
}

/// Download + install — emits `update-download-progress` while downloading,
/// then `update-installed`. Restarts the app on completion.
pub async fn download_and_install<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> tauri_plugin_updater::Result<()> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(());
    };

    let progress_app = app.clone();
    let installed_app = app.clone();
    let mut downloaded: usize = 0;
    let mut last_emitted: usize = 0;

    update
        .download_and_install(
            move |chunk_length, content_length| {
                downloaded += chunk_length;
                // Throttle events to roughly every 512 KiB to avoid flooding the IPC bus
                if downloaded - last_emitted >= 512 * 1024 {
                    last_emitted = downloaded;
                    let _ = progress_app.emit(
                        "update-download-progress",
                        serde_json::json!({ "downloaded": downloaded, "total": content_length }),
                    );
                }
            },
            move || {
                println!("[updater] download finished");
                let _ = installed_app.emit("update-installed", serde_json::json!({}));
            },
        )
        .await?;

    println!("[updater] update installed, restarting");
    app.restart();
}
