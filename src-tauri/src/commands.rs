//! Tauri IPC Commands for Lumex Control UI
//!
//! Exposes Beamer/Projector window management, monitor enumeration,
//! fullscreen toggling, and graphics engine status queries to the frontend.

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use crate::display::{EngineState, EngineStatus, MonitorInfo, ProjectorStatus};

/// Retrieve all connected displays with geometry, primary flag, and refresh rates.
#[tauri::command]
pub fn get_monitors<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, EngineState>,
) -> Result<Vec<MonitorInfo>, String> {
    state.get_monitors(&app)
}

/// Open native OS projector window on chosen monitor (or default primary display).
#[tauri::command]
pub fn open_projector<R: Runtime>(
    app: AppHandle<R>,
    monitor_index: Option<usize>,
    state: State<'_, EngineState>,
) -> Result<ProjectorStatus, String> {
    if let Some(proj_win) = app.get_webview_window("projector") {
        if let Ok(monitors) = app.available_monitors() {
            let target_monitor = if let Some(idx) = monitor_index {
                monitors.get(idx).cloned()
            } else {
                app.primary_monitor().ok().flatten().or_else(|| monitors.get(0).cloned())
            };

            if let Some(m) = target_monitor {
                let pos = m.position();
                let size = m.size();
                let w = (size.width as f64 * 0.70).clamp(800.0, 1440.0);
                let h = (w * 9.0 / 16.0).clamp(450.0, 900.0);
                let x = pos.x as f64 + ((size.width as f64 - w) / 2.0);
                let y = pos.y as f64 + ((size.height as f64 - h) / 2.0);
                let _ = proj_win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x as i32, y as i32)));
                let _ = proj_win.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(w as u32, h as u32)));
            }
        }

        let _ = proj_win.unminimize();
        let _ = proj_win.show();
        let _ = proj_win.set_focus();
        let _ = app.emit("projector:opened", ());
        let size = proj_win.inner_size().unwrap_or(tauri::PhysicalSize::new(1280, 720));
        let is_fs = proj_win.is_fullscreen().unwrap_or(false);
        return Ok(ProjectorStatus {
            is_open: true,
            is_fullscreen: is_fs,
            monitor_index: monitor_index.unwrap_or(0),
            monitor_name: "Output".into(),
            resolution: [size.width, size.height],
        });
    }

    let status = state.open_projector(&app, monitor_index)?;
    let _ = app.emit("projector:opened", &status);
    let _ = app.emit("projector:status", &status);
    Ok(status)
}

/// Close active projector window and release GPU surface.
#[tauri::command]
pub fn close_projector<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, EngineState>,
) -> Result<ProjectorStatus, String> {
    if let Some(proj_win) = app.get_webview_window("projector") {
        let _ = proj_win.hide();
        let _ = app.emit("projector:closed", ());
        return Ok(ProjectorStatus {
            is_open: false,
            is_fullscreen: false,
            monitor_index: 0,
            monitor_name: "Output".into(),
            resolution: [1280, 720],
        });
    }

    let status = state.close_projector()?;
    let _ = app.emit("projector:closed", ());
    let _ = app.emit("projector:status", &status);
    Ok(status)
}

/// Toggle borderless fullscreen mode on active projector.
#[tauri::command]
pub fn toggle_projector_fullscreen<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, EngineState>,
) -> Result<bool, String> {
    if let Some(proj_win) = app.get_webview_window("projector") {
        let is_fs = proj_win.is_fullscreen().unwrap_or(false);
        let next_fs = !is_fs;
        proj_win.set_fullscreen(next_fs).map_err(|e| e.to_string())?;
        let _ = app.emit("projector:fullscreen", next_fs);
        return Ok(next_fs);
    }
    let is_fullscreen = state.toggle_projector_fullscreen()?;
    let _ = app.emit("projector:fullscreen", is_fullscreen);
    Ok(is_fullscreen)
}

/// Query current display and rendering engine status.
#[tauri::command]
pub fn get_engine_status<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, EngineState>,
) -> Result<EngineStatus, String> {
    Ok(state.get_status())
}

/// Save a self-contained Lumex project bundle file (.lumex / .json) to disk.
#[tauri::command]
pub fn save_project_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|e| format!("Failed to save project to {}: {}", path, e))
}

/// Read a Lumex project bundle file from disk.
#[tauri::command]
pub fn read_project_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Failed to read project from {}: {}", path, e))
}
