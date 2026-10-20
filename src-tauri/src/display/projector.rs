//! Projector Output Window Architecture
//!
//! Provides an independent native OS window without webview overhead,
//! borderless fullscreen targeting the selected monitor, WGPU surface
//! presentation with hardware V-Sync (PresentMode::Fifo), and window lifecycle.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use tauri::{AppHandle, Manager, Monitor, Runtime, Window, WindowEvent};
use crate::graphics::context::WgpuContext;

pub const PROJECTOR_WINDOW_LABEL: &str = "projector";

pub fn is_escape_key_down() -> bool {
    false
}

/// Active surface state for the native projector window.
pub struct ProjectorSurface {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

impl ProjectorSurface {
    /// Configure or reconfigure the surface on the given GPU device.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let safe_w = width.max(1);
        let safe_h = height.max(1);
        if self.width != safe_w || self.height != safe_h {
            self.width = safe_w;
            self.height = safe_h;
            self.config.width = safe_w;
            self.config.height = safe_h;
            self.surface.configure(device, &self.config);
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

/// Managed handle to an open projector window and its GPU presentation surface.
pub struct ProjectorHandle<R: Runtime> {
    pub window: Arc<Window<R>>,
    pub surface: Arc<Mutex<Option<ProjectorSurface>>>,
    pub is_open: Arc<AtomicBool>,
    pub is_fullscreen: Arc<AtomicBool>,
    pub monitor_index: usize,
    pub monitor_name: String,
    pub resolution: Arc<Mutex<(u32, u32)>>,
}

impl<R: Runtime> ProjectorHandle<R> {
    /// Check if the projector window is currently open.
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    /// Check if the projector window is in fullscreen mode.
    pub fn is_fullscreen(&self) -> bool {
        self.is_fullscreen.load(Ordering::SeqCst)
    }

    /// Get current surface resolution (width, height).
    pub fn resolution(&self) -> (u32, u32) {
        *self.resolution.lock().unwrap()
    }

    /// Toggle borderless fullscreen mode.
    pub fn toggle_fullscreen(&self) -> Result<bool, String> {
        let current = self.is_fullscreen.load(Ordering::SeqCst);
        let next = !current;
        self.window
            .set_fullscreen(next)
            .map_err(|e| format!("Failed to set fullscreen {}: {}", next, e))?;
        self.is_fullscreen.store(next, Ordering::SeqCst);
        Ok(next)
    }

    /// Close the projector window and cleanly teardown the GPU surface.
    pub fn close(&self) -> Result<(), String> {
        self.is_open.store(false, Ordering::SeqCst);

        // Teardown GPU surface prior to window destruction
        if let Ok(mut surf_guard) = self.surface.lock() {
            *surf_guard = None;
        }

        self.window
            .destroy()
            .map_err(|e| format!("Failed to destroy projector window: {}", e))?;

        Ok(())
    }
}

/// Polymorphic interface for controlling projector windows without concrete runtime binding.
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

/// Create and launch a pure native OS projector window on the target monitor.
pub fn create_projector_window<R: Runtime>(
    app: &AppHandle<R>,
    monitor: &Monitor,
    config: ProjectorConfig,
    wgpu_ctx: &WgpuContext,
) -> Result<ProjectorHandle<R>, String> {
    // If an existing projector window is already open, destroy it first
    if let Some(existing) = app.get_window(PROJECTOR_WINDOW_LABEL) {
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

    // Pure native OS window without webview overhead
    let window = tauri::window::Window::builder(app, PROJECTOR_WINDOW_LABEL)
        .title(&config.title)
        .inner_size(safe_w as f64, safe_h as f64)
        .position(pos.x as f64, pos.y as f64)
        .decorations(false)
        .resizable(false)
        .always_on_top(config.always_on_top)
        .skip_taskbar(true)
        .build()
        .map_err(|e| format!("Failed to build native projector window: {}", e))?;

    if config.borderless_fullscreen {
        window
            .set_fullscreen(true)
            .map_err(|e| format!("Failed to set borderless fullscreen on projector: {}", e))?;
    }

    let window_arc = Arc::new(window);
    let is_open = Arc::new(AtomicBool::new(true));
    let is_fullscreen = Arc::new(AtomicBool::new(config.borderless_fullscreen));
    let resolution = Arc::new(Mutex::new((safe_w, safe_h)));

    // Create WGPU presentation surface on the native OS window
    let raw_surface: wgpu::Surface<'static> = wgpu_ctx
        .instance
        .create_surface(window_arc.clone())
        .map_err(|e| format!("Failed to create WGPU surface for projector: {}", e))?;

    let caps = raw_surface.get_capabilities(&wgpu_ctx.adapter);
    if caps.formats.is_empty() {
        return Err("Target monitor surface has no supported swapchain formats".to_string());
    }

    // Prefer standard sRGB output formats
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| *f == wgpu::TextureFormat::Bgra8UnormSrgb || *f == wgpu::TextureFormat::Rgba8UnormSrgb)
        .unwrap_or(caps.formats[0]);

    // Mandatory hardware V-Sync (PresentMode::Fifo) to eliminate tearing
    let present_mode = if config.vsync && caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        caps.present_modes[0]
    };

    let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
        wgpu::CompositeAlphaMode::Opaque
    } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Auto) {
        wgpu::CompositeAlphaMode::Auto
    } else {
        caps.alpha_modes[0]
    };

    let surf_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: safe_w,
        height: safe_h,
        present_mode,
        alpha_mode,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };

    raw_surface.configure(&wgpu_ctx.device, &surf_config);

    let proj_surface = ProjectorSurface {
        surface: raw_surface,
        config: surf_config,
        width: safe_w,
        height: safe_h,
        format,
    };

    let surface_mutex = Arc::new(Mutex::new(Some(proj_surface)));

    // Hook lifecycle and resize events
    let is_open_evt = is_open.clone();
    let surface_evt = surface_mutex.clone();
    let resolution_evt = resolution.clone();
    let device_evt = wgpu_ctx.device.clone();

    window_arc.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
            is_open_evt.store(false, Ordering::SeqCst);
            if let Ok(mut guard) = surface_evt.lock() {
                *guard = None;
            }
        }
        WindowEvent::Resized(new_size) => {
            if new_size.width > 0 && new_size.height > 0 {
                if let Ok(mut res_guard) = resolution_evt.lock() {
                    *res_guard = (new_size.width, new_size.height);
                }
                if let Ok(mut surf_guard) = surface_evt.lock() {
                    if let Some(ref mut s) = *surf_guard {
                        s.resize(&device_evt, new_size.width, new_size.height);
                    }
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
mod tests {
    use super::*;

    #[test]
    fn test_projector_config_defaults() {
        let cfg = ProjectorConfig::default();
        assert_eq!(cfg.title, "Lumex Output");
        assert_eq!(cfg.monitor_index, 0);
        assert!(cfg.borderless_fullscreen);
        assert!(cfg.always_on_top);
        assert!(cfg.vsync);
    }

    #[test]
    fn test_escape_key_query_does_not_panic() {
        let _pressed = is_escape_key_down();
    }
}
