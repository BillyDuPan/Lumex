//! Real-time audio capture engine using `cpal` and lock-free SPSC `rtrb` ring buffer.
//!
//! Designed for zero allocation on the OS audio thread (WASAPI on Windows, CoreAudio on macOS).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, InputCallbackInfo, SampleFormat, Stream, StreamConfig};
use rtrb::Producer;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug)]
pub enum AudioError {
    DeviceNotFound(String),
    NoDefaultDevice,
    StreamBuildError(String),
    StreamPlayError(String),
    UnsupportedSampleFormat(SampleFormat),
    DeviceEnumerationError(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeviceNotFound(name) => write!(f, "Audio input device '{}' not found", name),
            Self::NoDefaultDevice => write!(f, "No default audio input device available"),
            Self::StreamBuildError(err) => write!(f, "Failed to build audio stream: {}", err),
            Self::StreamPlayError(err) => write!(f, "Failed to play audio stream: {}", err),
            Self::UnsupportedSampleFormat(fmt) => write!(f, "Unsupported sample format: {:?}", fmt),
            Self::DeviceEnumerationError(err) => write!(f, "Failed to enumerate audio devices: {}", err),
        }
    }
}

impl std::error::Error for AudioError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

#[derive(Clone, Debug)]
pub struct AudioDeviceConfig {
    pub device_name: Option<String>,
    pub sample_rate: u32,
    pub buffer_size_frames: u32,
}

impl Default for AudioDeviceConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            sample_rate: 48000,
            buffer_size_frames: 256,
        }
    }
}

pub struct AudioCaptureEngine {
    stream: Option<Stream>,
    is_running: Arc<AtomicBool>,
}

impl AudioCaptureEngine {
    pub fn new() -> Self {
        Self {
            stream: None,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Enumerate available input devices.
    pub fn enumerate_devices() -> Result<Vec<AudioDeviceInfo>, AudioError> {
        let host = cpal::default_host();
        let default_device_name = host
            .default_input_device()
            .and_then(|d| d.description().ok().map(|desc| desc.name().to_string()));

        let devices = host
            .input_devices()
            .map_err(|e| AudioError::DeviceEnumerationError(e.to_string()))?;

        let mut list = Vec::new();
        for dev in devices {
            if let Ok(desc) = dev.description() {
                let name = desc.name().to_string();
                let is_default = default_device_name.as_deref() == Some(&name);
                list.push(AudioDeviceInfo { name, is_default });
            }
        }

        Ok(list)
    }

    /// Returns whether the capture stream is currently active.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Start audio capture stream, pushing mono-summed PCM samples to the ring buffer producer.
    pub fn start(
        &mut self,
        config: AudioDeviceConfig,
        mut producer: Producer<f32>,
    ) -> Result<(u32, u16), AudioError> {
        let host = cpal::default_host();

        let device = match config.device_name {
            Some(ref name) => host
                .input_devices()
                .map_err(|e| AudioError::DeviceEnumerationError(e.to_string()))?
                .find(|d| {
                    d.description()
                        .map(|desc| desc.name() == name)
                        .unwrap_or(false)
                })
                .ok_or_else(|| AudioError::DeviceNotFound(name.clone()))?,
            None => host
                .default_input_device()
                .ok_or(AudioError::NoDefaultDevice)?,
        };

        let default_config = device
            .default_input_config()
            .map_err(|e| AudioError::StreamBuildError(e.to_string()))?;

        let channels = default_config.channels();
        let sample_rate = config.sample_rate;

        let stream_config = StreamConfig {
            channels,
            sample_rate,
            buffer_size: BufferSize::Fixed(config.buffer_size_frames),
        };

        let err_fn = |err| {
            log::error!("Audio stream error: {}", err);
        };

        let is_running_clone = self.is_running.clone();

        let stream = match default_config.sample_format() {
            SampleFormat::F32 => device
                .build_input_stream(
                    stream_config,
                    move |data: &[f32], _: &InputCallbackInfo| {
                        Self::process_f32_samples(data, channels, &mut producer);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamBuildError(e.to_string()))?,
            SampleFormat::I16 => device
                .build_input_stream(
                    stream_config,
                    move |data: &[i16], _: &InputCallbackInfo| {
                        Self::process_i16_samples(data, channels, &mut producer);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamBuildError(e.to_string()))?,
            sample_format => {
                return Err(AudioError::UnsupportedSampleFormat(sample_format));
            }
        };

        stream
            .play()
            .map_err(|e| AudioError::StreamPlayError(e.to_string()))?;

        self.stream = Some(stream);
        is_running_clone.store(true, Ordering::SeqCst);

        Ok((sample_rate, channels))
    }

    /// Stop audio capture and drop the stream.
    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
        }
        self.is_running.store(false, Ordering::SeqCst);
    }

    #[inline(always)]
    fn process_f32_samples(data: &[f32], channels: u16, producer: &mut Producer<f32>) {
        if channels == 1 {
            for &sample in data {
                let _ = producer.push(sample);
            }
        } else {
            let ch = channels as usize;
            for frame in data.chunks_exact(ch) {
                let sum: f32 = frame.iter().sum();
                let mono = sum / (channels as f32);
                let _ = producer.push(mono);
            }
        }
    }

    #[inline(always)]
    fn process_i16_samples(data: &[i16], channels: u16, producer: &mut Producer<f32>) {
        const SCALE: f32 = 1.0 / i16::MAX as f32;
        if channels == 1 {
            for &sample in data {
                let mono = sample as f32 * SCALE;
                let _ = producer.push(mono);
            }
        } else {
            let ch = channels as usize;
            for frame in data.chunks_exact(ch) {
                let sum: f32 = frame.iter().map(|&s| s as f32 * SCALE).sum();
                let mono = sum / (channels as f32);
                let _ = producer.push(mono);
            }
        }
    }
}

impl Default for AudioCaptureEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioCaptureEngine {
    fn drop(&mut self) {
        self.stop();
    }
}
