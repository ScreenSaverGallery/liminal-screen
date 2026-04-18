// Liminal Screen - Auto-Updater Module
// Handles automatic update checking, downloading, and installation

use tauri_plugin_updater::UpdaterExt;

/// Check for updates, download, and install if available.
/// Restarts the application after successful installation.
pub async fn update<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> tauri_plugin_updater::Result<()> {
    if let Some(update) = app.updater()?.check().await? {
        let mut downloaded = 0;

        // Download and install with progress reporting
        update
            .download_and_install(
                |chunk_length, content_length| {
                    downloaded += chunk_length;
                    println!("downloaded {downloaded} from {content_length:?}");
                },
                || {
                    println!("download finished");
                },
            )
            .await?;

        println!("update installed");
        app.restart();
    }
    Ok(())
}
