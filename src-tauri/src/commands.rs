//! Tauri IPC Commands for Lumex Control UI
//!
//! Exposes Beamer/Projector window management, monitor enumeration,
//! fullscreen toggling, and graphics engine status queries to the frontend.

use tauri::{AppHandle, Emitter, Runtime, State};
use crate::display::{EngineState, EngineStatus, MonitorInfo, ProjectorStatus};

/// Retrieve all connected displays with geometry, primary flag, and refresh rates.
#[tauri::command]
pub fn get_monitors<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, EngineState>,
) -> Result<Vec<MonitorInfo>, String> {
    state.get_monitors(&app)
}

/// Open native OS projector window on chosen monitor (or default secondary display).
#[tauri::command]
pub fn open_projector<R: Runtime>(
    app: AppHandle<R>,
    monitor_index: Option<usize>,
    state: State<'_, EngineState>,
) -> Result<ProjectorStatus, String> {
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
