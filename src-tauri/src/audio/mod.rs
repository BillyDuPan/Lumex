//! Low-Latency Audio Reactivity Subsystem
//!
//! Provides cross-platform audio capture via `cpal` with lock-free `rtrb` ring buffering,
//! sliding window `realfft` spectral analysis, 4 frequency bands (Bass, Mid, High, Beat),
//! adaptive peak tracking, and std140-compliant 96-byte `AudioUniforms`.

pub mod analysis;
pub mod capture;
pub mod uniforms;

pub use analysis::{AudioAnalysisEngine, FFT_HOP_SIZE, FFT_WINDOW_SIZE};
pub use capture::{AudioCaptureEngine, AudioDeviceConfig, AudioDeviceInfo, AudioError};
pub use uniforms::AudioUniforms;
