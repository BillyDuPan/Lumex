pub mod spout;
pub mod syphon;

pub use spout::SpoutSender;
pub use syphon::SyphonSender;

/// Core trait for professional VJ texture sharing senders (Spout on Windows, Syphon on macOS).
///
/// Implementations stream GPU textures zero-copy to external visual production software
/// such as Resolume Arena, OBS Studio, TouchDesigner, Notch, and MadMapper.
pub trait TextureShareSender: Send + Sync {
    /// Publish a GPU texture frame to connected receivers.
    ///
    /// Validates texture format and dimensions against sender configuration,
    /// performs zero-copy GPU memory sharing, and synchronizes with external receivers.
    fn publish_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
    ) -> Result<(), String>;

    /// Returns the registered sender/server name visible to receivers.
    fn server_name(&self) -> &str;

    /// Returns whether this sender is actively streaming frames.
    fn is_active(&self) -> bool;

    /// Closes the sender and unregisters from platform shared memory / IPC directories.
    fn close(&mut self);
}

/// Mock texture sharing sender for headless testing, CI execution, and validation.
///
/// Tracks published frame counts, dimensions, format, and lifecycle state genuine in-memory.
#[derive(Debug, Clone)]
pub struct MockTextureSender {
    name: String,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    active: bool,
    published_frames: u64,
    last_texture_size: (u32, u32),
    last_format: Option<wgpu::TextureFormat>,
}

impl MockTextureSender {
    pub fn new(name: &str, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            name: name.to_string(),
            width,
            height,
            format,
            active: true,
            published_frames: 0,
            last_texture_size: (0, 0),
            last_format: None,
        }
    }

    pub fn published_frames(&self) -> u64 {
        self.published_frames
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn last_texture_size(&self) -> (u32, u32) {
        self.last_texture_size
    }

    pub fn last_format(&self) -> Option<wgpu::TextureFormat> {
        self.last_format
    }
}

impl TextureShareSender for MockTextureSender {
    fn publish_texture(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        texture: &wgpu::Texture,
    ) -> Result<(), String> {
        if !self.active {
            return Err("MockTextureSender is closed".to_string());
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
        self.last_texture_size = (texture.width(), texture.height());
        self.last_format = Some(texture.format());
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
    }
}

/// Fallback texture sharing sender when native GPU sharing APIs (Spout / Syphon)
/// are unavailable or running on unsupported platforms.
#[derive(Debug, Clone)]
pub struct FallbackTextureSender {
    name: String,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    active: bool,
    frames_sent: u64,
}

impl FallbackTextureSender {
    pub fn new(name: &str, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            name: name.to_string(),
            width,
            height,
            format,
            active: true,
            frames_sent: 0,
        }
    }

    pub fn frames_sent(&self) -> u64 {
        self.frames_sent
    }
}

impl TextureShareSender for FallbackTextureSender {
    fn publish_texture(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        texture: &wgpu::Texture,
    ) -> Result<(), String> {
        if !self.active {
            return Err("FallbackTextureSender is closed".to_string());
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
        self.frames_sent += 1;
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
    }
}

/// Factory function to instantiate the platform-native texture share sender.
///
/// On Windows, instantiates a Spout2 DirectX 12 NT shared resource handle sender (`SpoutSender`).
/// On macOS, instantiates a Syphon Metal `IOSurfaceRef` shared texture sender (`SyphonSender`).
/// On other platforms or fallback, instantiates a `FallbackTextureSender`.
pub fn create_texture_sender(
    name: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> Box<dyn TextureShareSender> {
    #[cfg(target_os = "windows")]
    {
        Box::new(SpoutSender::new(name, width, height, format))
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(SyphonSender::new(name, width, height, format))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Box::new(FallbackTextureSender::new(name, width, height, format))
    }
}
