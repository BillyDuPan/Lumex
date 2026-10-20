//! Std140-compliant 96-byte AudioUniforms struct matching PROJECT.md interface contract.

use serde::{Deserialize, Serialize};

/// Std140-compliant 96-byte uniform buffer layout for audio reactivity.
///
/// Layout in memory:
/// - Offset  0: `bands: [f32; 4]`      (16 bytes) -> [bass, mid, high, beat]
/// - Offset 16: `raw_levels: [f32; 4]` (16 bytes) -> [raw_bass, raw_mid, raw_high, input_level]
/// - Offset 32: `spectrum: [[f32; 4]; 4]` (64 bytes) -> 16 log-spaced normalized frequency bins (4 x vec4)
/// Total size: 96 bytes, 16-byte aligned.
#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct AudioUniforms {
    pub bands: [f32; 4],
    pub raw_levels: [f32; 4],
    pub spectrum: [[f32; 4]; 4],
}

impl AudioUniforms {
    /// Create a zero-initialized AudioUniforms struct.
    #[inline]
    pub const fn new() -> Self {
        Self {
            bands: [0.0; 4],
            raw_levels: [0.0; 4],
            spectrum: [[0.0; 4]; 4],
        }
    }

    /// Normalized Bass energy in `[0.0, 1.0]` (30–150 Hz).
    #[inline]
    pub fn bass(&self) -> f32 {
        self.bands[0]
    }

    /// Normalized Mid energy in `[0.0, 1.0]` (150–2000 Hz).
    #[inline]
    pub fn mid(&self) -> f32 {
        self.bands[1]
    }

    /// Normalized High energy in `[0.0, 1.0]` (2000–12000 Hz).
    #[inline]
    pub fn high(&self) -> f32 {
        self.bands[2]
    }

    /// Decaying beat onset impulse in `[0.0, 1.0]`.
    #[inline]
    pub fn beat(&self) -> f32 {
        self.bands[3]
    }

    /// Unnormalized raw RMS bass energy.
    #[inline]
    pub fn raw_bass(&self) -> f32 {
        self.raw_levels[0]
    }

    /// Unnormalized raw RMS mid energy.
    #[inline]
    pub fn raw_mid(&self) -> f32 {
        self.raw_levels[1]
    }

    /// Unnormalized raw RMS high energy.
    #[inline]
    pub fn raw_high(&self) -> f32 {
        self.raw_levels[2]
    }

    /// Overall input RMS energy.
    #[inline]
    pub fn input_level(&self) -> f32 {
        self.raw_levels[3]
    }

    /// Alias for input_level() matching raw_rms nomenclature.
    #[inline]
    pub fn raw_rms(&self) -> f32 {
        self.raw_levels[3]
    }

    /// Set the normalized reactivity bands [bass, mid, high, beat].
    #[inline]
    pub fn set_bands(&mut self, bass: f32, mid: f32, high: f32, beat: f32) {
        self.bands = [bass, mid, high, beat];
    }

    /// Set the unnormalized raw levels [raw_bass, raw_mid, raw_high, input_level].
    #[inline]
    pub fn set_raw_levels(&mut self, raw_bass: f32, raw_mid: f32, raw_high: f32, input_level: f32) {
        self.raw_levels = [raw_bass, raw_mid, raw_high, input_level];
    }

    /// Set the 16 log-spaced spectrum bin magnitudes.
    pub fn set_spectrum_from_slice(&mut self, bins: &[f32; 16]) {
        for i in 0..4 {
            for j in 0..4 {
                self.spectrum[i][j] = bins[i * 4 + j];
            }
        }
    }
}

impl Default for AudioUniforms {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time checks for exact std140 layout compliance
const _: () = assert!(std::mem::size_of::<AudioUniforms>() == 96);
const _: () = assert!(std::mem::align_of::<AudioUniforms>() == 16);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<AudioUniforms>(), 96);
        assert_eq!(std::mem::align_of::<AudioUniforms>(), 16);

        let mut u = AudioUniforms::default();
        u.set_bands(0.8, 0.4, 0.2, 1.0);
        assert_eq!(u.bass(), 0.8);
        assert_eq!(u.mid(), 0.4);
        assert_eq!(u.high(), 0.2);
        assert_eq!(u.beat(), 1.0);

        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 96);
    }
}
