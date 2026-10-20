//! Windowing & Dual Display Architecture
//!
//! Provides display management, native OS projector output windows,
//! embedded GPU preview presentation, and unified engine display state.

pub mod manager;
pub mod preview;
pub mod projector;

pub use manager::{DisplayManager, MonitorInfo};
pub use preview::{
    DualPresentationResult, DualSurfacePresenter, GpuBlitPipeline, PreviewSurface, ViewportRect,
};
pub use projector::{
    create_projector_window, is_escape_key_down, ProjectorConfig, ProjectorControl,
    ProjectorHandle, ProjectorSurface, PROJECTOR_WINDOW_LABEL,
};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use crate::graphics::context::WgpuContext;

/// Status information of the native projector window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectorStatus {
    pub is_open: bool,
    pub is_fullscreen: bool,
    pub monitor_index: usize,
    pub monitor_name: String,
    pub resolution: [u32; 2],
}

/// Comprehensive status of the dual-display rendering engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineStatus {
    pub projector_active: bool,
    pub preview_active: bool,
    pub current_monitor: Option<MonitorInfo>,
    pub fullscreen: bool,
    pub surface_resolution: Option<[u32; 2]>,
    pub vsync_mode: String,
    pub fps: f32,
    pub frame_count: u64,
}

/// Managed application state holding WGPU context, display manager, and surface handles.
pub struct EngineState {
    pub context: WgpuContext,
    pub display_manager: Mutex<DisplayManager>,
    pub projector: Mutex<Option<Box<dyn ProjectorControl>>>,
    pub preview: Mutex<Option<PreviewSurface>>,
    pub presenter: Mutex<DualSurfacePresenter>,
    pub frame_count: AtomicU64,
}

impl EngineState {
    pub fn new(context: WgpuContext) -> Self {
        let presenter =
            DualSurfacePresenter::new(&context.device, wgpu::TextureFormat::Rgba8UnormSrgb);
        Self {
            context,
            display_manager: Mutex::new(DisplayManager::new()),
            projector: Mutex::new(None),
            preview: Mutex::new(None),
            presenter: Mutex::new(presenter),
            frame_count: AtomicU64::new(0),
        }
    }

    pub fn new_headless() -> Result<Self, String> {
        let ctx = WgpuContext::init_headless_sync()?;
        Ok(Self::new(ctx))
    }

    /// Enumerate all connected monitors and update display manager.
    pub fn get_monitors<R: Runtime>(&self, app: &AppHandle<R>) -> Result<Vec<MonitorInfo>, String> {
        let mut dm = self.display_manager.lock().map_err(|e| e.to_string())?;
        dm.refresh(app)?;
        Ok(dm.monitors().to_vec())
    }

    /// Open native OS projector window on the chosen or default secondary monitor.
    pub fn open_projector<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        monitor_index: Option<usize>,
    ) -> Result<ProjectorStatus, String> {
        let mut dm = self.display_manager.lock().map_err(|e| e.to_string())?;
        dm.refresh(app)?;

        if let Some(idx) = monitor_index {
            dm.select_monitor(idx)?;
        }

        let monitor = dm.target_monitor_for_projector(app)?;
        let chosen_idx = dm.selected_index();

        let config = ProjectorConfig {
            title: "Lumex Projector Output".into(),
            monitor_index: chosen_idx,
            borderless_fullscreen: true,
            always_on_top: true,
            vsync: true,
        };

        let handle = create_projector_window(
            app,
            &monitor,
            config,
            &self.context,
        )?;

        let status = ProjectorStatus {
            is_open: handle.is_open(),
            is_fullscreen: handle.is_fullscreen(),
            monitor_index: handle.monitor_index,
            monitor_name: handle.monitor_name.clone(),
            resolution: [handle.resolution().0, handle.resolution().1],
        };

        if let Ok(mut proj_lock) = self.projector.lock() {
            *proj_lock = Some(Box::new(handle));
        }

        Ok(status)
    }

    /// Close the active projector window.
    pub fn close_projector(&self) -> Result<ProjectorStatus, String> {
        let mut proj_lock = self.projector.lock().map_err(|e| e.to_string())?;
        if let Some(ref handle) = *proj_lock {
            let status = ProjectorStatus {
                is_open: false,
                is_fullscreen: handle.is_fullscreen(),
                monitor_index: handle.monitor_index(),
                monitor_name: handle.monitor_name(),
                resolution: [handle.resolution().0, handle.resolution().1],
            };
            let _ = handle.close();
            *proj_lock = None;
            Ok(status)
        } else {
            Ok(ProjectorStatus {
                is_open: false,
                is_fullscreen: false,
                monitor_index: 0,
                monitor_name: "None".into(),
                resolution: [0, 0],
            })
        }
    }

    /// Toggle borderless fullscreen mode on active projector.
    pub fn toggle_projector_fullscreen(&self) -> Result<bool, String> {
        let proj_lock = self.projector.lock().map_err(|e| e.to_string())?;
        if let Some(ref handle) = *proj_lock {
            handle.toggle_fullscreen()
        } else {
            Err("No active projector window to toggle fullscreen".to_string())
        }
    }

    /// Query current status of display engine and outputs.
    pub fn get_status(&self) -> EngineStatus {
        let (proj_active, fullscreen, mon_idx, mon_name, res) = {
            if let Ok(guard) = self.projector.lock() {
                if let Some(ref h) = *guard {
                    let r = h.resolution();
                    (
                        h.is_open(),
                        h.is_fullscreen(),
                        Some(h.monitor_index()),
                        Some(h.monitor_name()),
                        Some([r.0, r.1]),
                    )
                } else {
                    (false, false, None, None, None)
                }
            } else {
                (false, false, None, None, None)
            }
        };

        let preview_active = if let Ok(guard) = self.preview.lock() {
            guard.as_ref().map_or(false, |p| p.surface.is_some())
        } else {
            false
        };

        let current_monitor = if let (Some(idx), Some(name)) = (mon_idx, mon_name) {
            Some(MonitorInfo {
                index: idx,
                id: format!("display-{}", idx),
                name,
                primary: idx == 0,
                width: res.map_or(1920, |r| r[0]),
                height: res.map_or(1080, |r| r[1]),
                x: 0,
                y: 0,
                scale_factor: 1.0,
                refresh_rate_millihertz: Some(60_000),
            })
        } else if let Ok(dm) = self.display_manager.lock() {
            dm.get_selected().cloned()
        } else {
            None
        };

        EngineStatus {
            projector_active: proj_active,
            preview_active,
            current_monitor,
            fullscreen,
            surface_resolution: res,
            vsync_mode: "Fifo".into(),
            fps: 60.0,
            frame_count: self.frame_count.load(Ordering::Relaxed),
        }
    }
}
