//! Lumex WGPU Graphics Context
//!
//! Provides headless and windowed WGPU initialization with support for
//! DirectX 12, Vulkan, and Metal backends.

use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct WgpuContext {
    pub instance: Arc<wgpu::Instance>,
    pub adapter: Arc<wgpu::Adapter>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}

impl WgpuContext {
    /// Initialize a headless WGPU context (for testing, benchmarks, or offscreen rendering).
    pub async fn init_headless() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "Failed to find suitable WGPU adapter".to_string())?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Lumex Headless Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to request WGPU device: {}", e))?;

        Ok(Self {
            instance: Arc::new(instance),
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
        })
    }

    /// Synchronous initialization of headless WGPU context via pollster.
    pub fn init_headless_sync() -> Result<Self, String> {
        pollster::block_on(Self::init_headless())
    }

    /// Initialize WGPU context compatible with a specific presentation surface.
    pub async fn init_with_surface(surface: &wgpu::Surface<'_>) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "Failed to find WGPU adapter compatible with surface".to_string())?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Lumex Surface Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to request WGPU device: {}", e))?;

        Ok(Self {
            instance: Arc::new(instance),
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
        })
    }

    /// Create a WGPU presentation surface from any window handle.
    pub fn create_surface<'a, W>(&self, window: W) -> Result<wgpu::Surface<'a>, String>
    where
        W: raw_window_handle::HasWindowHandle + raw_window_handle::HasDisplayHandle + Send + Sync + 'a,
    {
        self.instance
            .create_surface(window)
            .map_err(|e| format!("Failed to create surface: {}", e))
    }
}
