use super::TextureShareSender;

#[cfg(target_os = "macos")]
use syphon_wgpu::SyphonWgpuOutput;

/// macOS Syphon texture sharing sender.
///
/// Streams GPU frames zero-copy using Metal `IOSurfaceRef` backed textures.
/// External applications on macOS (Resolume Arena, MadMapper, OBS Studio, CoGe)
/// read directly from the shared Metal surface.
pub struct SyphonSender {
    name: String,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    active: bool,
    published_frames: u64,
    is_hardware_accelerated: bool,
    #[cfg(target_os = "macos")]
    output: Option<SyphonWgpuOutput>,
}

unsafe impl Send for SyphonSender {}
unsafe impl Sync for SyphonSender {}

impl SyphonSender {
    /// Create a new Syphon sender with the given server name and texture properties.
    pub fn new(name: &str, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            name: name.to_string(),
            width,
            height,
            format,
            active: true,
            published_frames: 0,
            is_hardware_accelerated: false,
            #[cfg(target_os = "macos")]
            output: None,
        }
    }

    /// Whether this sender is actively backed by native macOS Metal / IOSurface hardware sharing.
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
}

impl TextureShareSender for SyphonSender {
    fn publish_texture(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        texture: &wgpu::Texture,
    ) -> Result<(), String> {
        if !self.active {
            return Err("Syphon sender is closed".to_string());
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

        #[cfg(target_os = "macos")]
        {
            if self.output.is_none() {
                match SyphonWgpuOutput::new(&self.name, _device, _queue, self.width, self.height) {
                    Ok(out) => {
                        self.output = Some(out);
                        self.is_hardware_accelerated = true;
                        log::info!("Syphon Metal IOSurface sender initialized: '{}'", self.name);
                    }
                    Err(err) => {
                        log::warn!(
                            "Syphon Metal initialization failed ({:?}), continuing in fallback mode",
                            err
                        );
                    }
                }
            }

            if let Some(ref mut out) = self.output {
                let _ = out.publish(texture, _device, _queue);
            }
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
        #[cfg(target_os = "macos")]
        {
            self.output = None;
            self.is_hardware_accelerated = false;
        }
    }
}
