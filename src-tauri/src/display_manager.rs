// src-tauri/src/display/display_manager.rs
use tauri::{command, AppHandle, Runtime};
#[derive(serde::Serialize, Clone)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(serde::Serialize, Clone)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(serde::Serialize, Clone)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub position: Position,
    pub size: Size,
    pub scale_factor: f64,
}

#[command]
pub fn get_available_monitors<R: Runtime>(app: AppHandle<R>) -> Result<Vec<MonitorInfo>, String> {
    let monitors = app
        .available_monitors()
        .map_err(|e| format!("Failed to get monitors: {}", e))?;

    let monitor_infos: Vec<MonitorInfo> = monitors
        .into_iter()
        .enumerate()
        .map(|(index, m)| {
            let pos = m.position();
            let sz = m.size();
            MonitorInfo {
                id: index as u32,
                name: m
                    .name()
                    .map(|s| s.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                position: Position { x: pos.x, y: pos.y },
                size: Size {
                    width: sz.width,
                    height: sz.height,
                },
                scale_factor: m.scale_factor(),
            }
        })
        .collect();

    Ok(monitor_infos)
}

// Plugin initialization
pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("display-manager")
        .invoke_handler(tauri::generate_handler![get_available_monitors])
        .build()
}
