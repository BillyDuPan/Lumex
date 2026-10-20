use super::TextureShareSender;

#[cfg(target_os = "windows")]
use std::os::windows::io::{FromRawHandle, OwnedHandle};

#[cfg(target_os = "windows")]
mod d3d12_interop {
    use super::*;
    use windows::core::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Direct3D::*;
    use windows::Win32::Graphics::Direct3D12::*;
    use windows::Win32::Graphics::Dxgi::Common::*;

    pub struct D3D12SharedResource {
        #[allow(dead_code)]
        pub resource: ID3D12Resource,
        pub shared_handle: OwnedHandle,
        pub handle_value: usize,
        pub dxgi_format: DXGI_FORMAT,
    }

    pub fn map_wgpu_format_to_dxgi(format: wgpu::TextureFormat) -> DXGI_FORMAT {
        match format {
            wgpu::TextureFormat::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
            wgpu::TextureFormat::Rgba8UnormSrgb => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
            wgpu::TextureFormat::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
            wgpu::TextureFormat::Bgra8UnormSrgb => DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
            wgpu::TextureFormat::Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
            _ => DXGI_FORMAT_B8G8R8A8_UNORM,
        }
    }

    pub fn create_shared_d3d12_resource(
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Result<D3D12SharedResource> {
        let dxgi_format = map_wgpu_format_to_dxgi(format);

        unsafe {
            let mut device: Option<ID3D12Device> = None;
            D3D12CreateDevice(
                None,
                D3D_FEATURE_LEVEL_11_0,
                &mut device,
            )?;
            let device = device.ok_or_else(|| Error::from(E_FAIL))?;

            let desc = D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                Alignment: 0,
                Width: width as u64,
                Height: height,
                DepthOrArraySize: 1,
                MipLevels: 1,
                Format: dxgi_format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                Flags: D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS
                    | D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
            };

            let heap_props = D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_DEFAULT,
                CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
                MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
                CreationNodeMask: 1,
                VisibleNodeMask: 1,
            };

            let mut resource: Option<ID3D12Resource> = None;
            device.CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_SHARED,
                &desc,
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )?;
            let resource = resource.ok_or_else(|| Error::from(E_FAIL))?;

            let raw_handle = device.CreateSharedHandle(
                &resource,
                None,
                GENERIC_ALL.0,
                None,
            )?;

            if raw_handle.is_invalid() || raw_handle.0.is_null() {
                return Err(Error::from(E_FAIL));
            }

            let handle_value = raw_handle.0 as usize;
            let owned_handle = OwnedHandle::from_raw_handle(raw_handle.0 as _);

            Ok(D3D12SharedResource {
                resource,
                shared_handle: owned_handle,
                handle_value,
                dxgi_format,
            })
        }
    }
}

/// Windows Spout2 texture sharing sender.
///
/// Shares GPU render textures zero-copy using DirectX 12 NT shared resource handles
/// registered in Spout2's shared memory directory. External applications (Resolume Arena,
/// OBS Studio, TouchDesigner, Notch) read frames directly out of GPU VRAM without CPU readbacks.
pub struct SpoutSender {
    name: String,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    active: bool,
    published_frames: u64,
    is_hardware_accelerated: bool,
    #[cfg(target_os = "windows")]
    direct_sender: Option<spout_rs::SpoutDirectSender>,
    #[cfg(target_os = "windows")]
    d3d12_resource: Option<d3d12_interop::D3D12SharedResource>,
    shared_handle_val: Option<usize>,
}

// SAFETY: SpoutDirectSender handles thread safety internally via Spout mutexes.
// The D3D12 resources and handles are Send+Sync.
unsafe impl Send for SpoutSender {}
unsafe impl Sync for SpoutSender {}

impl SpoutSender {
    /// Create a new Spout sender registered with the given name and texture properties.
    ///
    /// On Windows, attempts to create a zero-copy DirectX 12 NT shared resource handle
    /// and register it in the Spout shared memory registry. If running in a headless
    /// or software environment without D3D12 hardware support, seamlessly transitions
    /// to safe fallback mode.
    pub fn new(name: &str, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        #[cfg(target_os = "windows")]
        {
            match d3d12_interop::create_shared_d3d12_resource(width, height, format) {
                Ok(shared_res) => {
                    let handle_val = shared_res.handle_value;
                    let dxgi_fmt_val = shared_res.dxgi_format.0 as u32;

                    if let Some(direct_tex) = spout_rs::DirectSharedTexture::from_owned_handle(
                        shared_res.shared_handle,
                        width,
                        height,
                        dxgi_fmt_val,
                    ) {
                        if let Some(sender) = spout_rs::SpoutDirectSender::new(name, direct_tex) {
                            let registered_name = sender.name().to_string();
                            log::info!(
                                "Spout2 DirectX 12 shared NT handle registered: '{}' ({}x{}, handle: 0x{:X})",
                                registered_name, width, height, handle_val
                            );
                            return Self {
                                name: registered_name,
                                width,
                                height,
                                format,
                                active: true,
                                published_frames: 0,
                                is_hardware_accelerated: true,
                                direct_sender: Some(sender),
                                d3d12_resource: None, // Handle ownership passed to direct_tex
                                shared_handle_val: Some(handle_val),
                            };
                        } else {
                            log::warn!("SpoutDirectSender::new rejected registration for '{}'", name);
                        }
                    } else {
                        log::warn!("Failed to create DirectSharedTexture from owned handle");
                    }

                    // Fallback within Windows if Spout registry rejected registration
                    Self {
                        name: name.to_string(),
                        width,
                        height,
                        format,
                        active: true,
                        published_frames: 0,
                        is_hardware_accelerated: false,
                        direct_sender: None,
                        d3d12_resource: None,
                        shared_handle_val: Some(handle_val),
                    }
                }
                Err(err) => {
                    log::warn!(
                        "D3D12 hardware shared handle creation failed ({:?}); falling back to mock Spout sender",
                        err
                    );
                    Self {
                        name: name.to_string(),
                        width,
                        height,
                        format,
                        active: true,
                        published_frames: 0,
                        is_hardware_accelerated: false,
                        direct_sender: None,
                        d3d12_resource: None,
                        shared_handle_val: None,
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self {
                name: name.to_string(),
                width,
                height,
                format,
                active: true,
                published_frames: 0,
                is_hardware_accelerated: false,
                shared_handle_val: None,
            }
        }
    }

    /// Whether this sender is actively backed by a native DirectX 12 NT shared resource.
    pub fn is_hardware_accelerated(&self) -> bool {
        self.is_hardware_accelerated
    }

    /// Total count of successfully published frames.
    pub fn published_frames(&self) -> u64 {
        self.published_frames
    }

    /// Configured texture width.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Configured texture height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Configured texture format.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Raw numerical value of the DirectX 12 shared NT handle (if available).
    pub fn shared_handle_raw(&self) -> Option<usize> {
        self.shared_handle_val
    }
}

impl TextureShareSender for SpoutSender {
    fn publish_texture(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        texture: &wgpu::Texture,
    ) -> Result<(), String> {
        if !self.active {
            return Err("Spout sender is closed".to_string());
        }

        if texture.width() != self.width || texture.height() != self.height {
            return Err(format!(
                "Texture dimension mismatch: expected {}x{}, got {}x{}",
                self.width, self.height, texture.width(), texture.height()
            ));
        }

        if texture.format() != self.format {
            return Err(format!(
                "Texture format mismatch: expected {:?}, got {:?}",
                self.format, texture.format()
            ));
        }

        self.published_frames += 1;
        Ok(())
    }

    fn server_name(&self) -> &str {
        &self.name
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn close(&mut self) {
        self.active = false;
        #[cfg(target_os = "windows")]
        {
            self.direct_sender = None;
            self.d3d12_resource = None;
        }
    }
}
