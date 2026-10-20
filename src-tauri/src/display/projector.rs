//! Projector Output Window Architecture
//!
//! Provides an independent projector window loading output.html on the
//! target monitor in borderless fullscreen with hardware acceleration.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, Monitor, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use crate::graphics::context::WgpuContext;

pub const PROJECTOR_WINDOW_LABEL: &str = "projector";

pub fn is_escape_key_down() -> bool {
    false
}

/// Active surface state for the projector window.
pub struct ProjectorSurface {
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: Option<wgpu::SurfaceConfiguration>,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

impl ProjectorSurface {
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let safe_w = width.max(1);
        let safe_h = height.max(1);
        if self.width != safe_w || self.height != safe_h {
            self.width = safe_w;
            self.height = safe_h;
            if let (Some(s), Some(c)) = (&self.surface, &mut self.config) {
                c.width = safe_w;
                c.height = safe_h;
                s.configure(device, c);
            }
        }
    }
}

/// Configuration parameters for creating the projector window.
#[derive(Debug, Clone)]
pub struct ProjectorConfig {
    pub title: String,
    pub monitor_index: usize,
    pub borderless_fullscreen: bool,
    pub always_on_top: bool,
    pub vsync: bool,
}

impl Default for ProjectorConfig {
    fn default() -> Self {
        Self {
            title: "Lumex Output".into(),
            monitor_index: 0,
            borderless_fullscreen: true,
            always_on_top: true,
            vsync: true,
        }
    }
}

/// Managed handle to an open projector window.
pub struct ProjectorHandle<R: Runtime> {
    pub window: Arc<WebviewWindow<R>>,
    pub surface: Arc<Mutex<Option<ProjectorSurface>>>,
    pub is_open: Arc<AtomicBool>,
    pub is_fullscreen: Arc<AtomicBool>,
    pub monitor_index: usize,
    pub monitor_name: String,
    pub resolution: Arc<Mutex<(u32, u32)>>,
}

impl<R: Runtime> ProjectorHandle<R> {
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    pub fn is_fullscreen(&self) -> bool {
        self.is_fullscreen.load(Ordering::SeqCst)
    }

    pub fn resolution(&self) -> (u32, u32) {
        *self.resolution.lock().unwrap()
    }

    pub fn toggle_fullscreen(&self) -> Result<bool, String> {
        let current = self.is_fullscreen.load(Ordering::SeqCst);
        let next = !current;
        self.window
            .set_fullscreen(next)
            .map_err(|e| format!("Failed to set fullscreen {}: {}", next, e))?;
        self.is_fullscreen.store(next, Ordering::SeqCst);
        Ok(next)
    }

    pub fn close(&self) -> Result<(), String> {
        self.is_open.store(false, Ordering::SeqCst);
        self.window
            .destroy()
            .map_err(|e| format!("Failed to destroy projector window: {}", e))?;
        Ok(())
    }
}

pub trait ProjectorControl: Send + Sync {
    fn is_open(&self) -> bool;
    fn is_fullscreen(&self) -> bool;
    fn resolution(&self) -> (u32, u32);
    fn toggle_fullscreen(&self) -> Result<bool, String>;
    fn close(&self) -> Result<(), String>;
    fn monitor_name(&self) -> String;
    fn monitor_index(&self) -> usize;
}

impl<R: Runtime> ProjectorControl for ProjectorHandle<R> {
    fn is_open(&self) -> bool {
        ProjectorHandle::is_open(self)
    }

    fn is_fullscreen(&self) -> bool {
        ProjectorHandle::is_fullscreen(self)
    }

    fn resolution(&self) -> (u32, u32) {
        ProjectorHandle::resolution(self)
    }

    fn toggle_fullscreen(&self) -> Result<bool, String> {
        ProjectorHandle::toggle_fullscreen(self)
    }

    fn close(&self) -> Result<(), String> {
        ProjectorHandle::close(self)
    }

    fn monitor_name(&self) -> String {
        self.monitor_name.clone()
    }

    fn monitor_index(&self) -> usize {
        self.monitor_index
    }
}

/// Create and launch the projector window loading output.html on the target monitor.
pub fn create_projector_window<R: Runtime>(
    app: &AppHandle<R>,
    monitor: &Monitor,
    config: ProjectorConfig,
    _wgpu_ctx: &WgpuContext,
) -> Result<ProjectorHandle<R>, String> {
    // If an existing projector window is already open, destroy it first
    if let Some(existing) = app.get_webview_window(PROJECTOR_WINDOW_LABEL) {
        let _ = existing.destroy();
    }

    let pos = monitor.position();
    let size = monitor.size();
    let safe_w = size.width.max(1);
    let safe_h = size.height.max(1);
    let mon_name = monitor
        .name()
        .cloned()
        .unwrap_or_else(|| format!("Display {}", config.monitor_index + 1));

    let webview_builder = WebviewWindowBuilder::new(
        app,
        PROJECTOR_WINDOW_LABEL,
        WebviewUrl::App("output.html".into()),
    )
    .title(&config.title)
    .inner_size(safe_w as f64, safe_h as f64)
    .position(pos.x as f64, pos.y as f64)
    .decorations(false)
    .resizable(false)
    .always_on_top(config.always_on_top)
    .skip_taskbar(true);

    let window = webview_builder
        .build()
        .map_err(|e| format!("Failed to build projector window: {}", e))?;

    if config.borderless_fullscreen {
        window
            .set_fullscreen(true)
            .map_err(|e| format!("Failed to set borderless fullscreen on projector: {}", e))?;
    }

    let window_arc = Arc::new(window);
    let is_open = Arc::new(AtomicBool::new(true));
    let is_fullscreen = Arc::new(AtomicBool::new(config.borderless_fullscreen));
    let resolution = Arc::new(Mutex::new((safe_w, safe_h)));
    let surface_mutex = Arc::new(Mutex::new(None));

    // Hook lifecycle and resize events
    let is_open_evt = is_open.clone();
    let resolution_evt = resolution.clone();

    window_arc.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
            is_open_evt.store(false, Ordering::SeqCst);
        }
        WindowEvent::Resized(new_size) => {
            if new_size.width > 0 && new_size.height > 0 {
                if let Ok(mut res_guard) = resolution_evt.lock() {
                    *res_guard = (new_size.width, new_size.height);
                }
            }
        }
        _ => {}
    });

    Ok(ProjectorHandle {
        window: window_arc,
        surface: surface_mutex,
        is_open,
        is_fullscreen,
        monitor_index: config.monitor_index,
        monitor_name: mon_name,
        resolution,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    pub fn test_projector_config_defaults() {
        let cfg = ProjectorConfig::default();
        assert_eq!(cfg.monitor_index, 0);
        assert!(cfg.borderless_fullscreen);
        assert!(cfg.always_on_top);
        assert!(cfg.vsync);
    }

    #[test]
    pub fn test_escape_key_query_does_not_panic() {
        assert!(!is_escape_key_down());
    }
}
