//! Real-time sliding window FFT spectrum analysis and beat onset detection engine.
//!
//! Uses `realfft` for forward real-to-complex FFT with precomputed Hann windowing,
//! 4-band frequency decomposition (Bass, Mid, High, Beat), adaptive asymmetric peak
//! tracking, and 16-bin log-spaced spectrum analysis.

use crate::audio::uniforms::AudioUniforms;
use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::f32::consts::PI;
use std::sync::Arc;

pub const FFT_WINDOW_SIZE: usize = 2048;
pub const FFT_HOP_SIZE: usize = 256;
pub const GATE: f32 = 0.003;
pub const NOISE_FLOOR: f32 = 0.005;
pub const TAU_RISE: f32 = 0.08;
pub const TAU_DECAY: f32 = 5.0;
pub const TAU_BEAT_DECAY: f32 = 0.07;
pub const REFRACTORY_PERIOD: f32 = 0.20;

pub struct AudioAnalysisEngine {
    sample_rate: u32,
    window_size: usize,
    sliding_window: Vec<f32>,
    hann_window: Vec<f32>,
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    fft_output: Vec<Complex<f32>>,
    fft_input: Vec<f32>,
    magnitudes: Vec<f32>,

    // Frequency bin bounds for the 3 main bands
    k_bass_min: usize,
    k_bass_max: usize,
    k_mid_min: usize,
    k_mid_max: usize,
    k_high_min: usize,
    k_high_max: usize,

    // 16 sub-band frequency bounds
    sub_band_bounds: [(usize, usize); 16],

    // Adaptive peak tracking
    bass_peak: f32,
    mid_peak: f32,
    high_peak: f32,
    spectrum_peaks: [f32; 16],

    // Beat onset detection
    bass_history_avg: f32,
    prev_bass_level: f32,
    beat_impulse: f32,
    time_since_beat: f32,
    beat_count: f32,
    bpm: f32,
}

impl AudioAnalysisEngine {
    /// Create a new AudioAnalysisEngine initialized for the given sample rate (e.g. 48000 Hz).
    pub fn new(sample_rate: u32) -> Self {
        let window_size = FFT_WINDOW_SIZE;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(window_size);
        let fft_scratch = fft_forward.make_scratch_vec();
        let fft_output = fft_forward.make_output_vec();

        // Precompute Hann window: w[n] = 0.5 * (1 - cos(2*pi*n / (N-1)))
        let mut hann_window = Vec::with_capacity(window_size);
        let denom = (window_size - 1) as f32;
        for n in 0..window_size {
            let w = 0.5 * (1.0 - (2.0 * PI * n as f32 / denom).cos());
            hann_window.push(w);
        }

        let df = sample_rate as f32 / window_size as f32; // e.g. 23.4375 Hz

        // Bass: 30 - 150 Hz
        let k_bass_min = (30.0 / df).round().max(1.0) as usize;
        let k_bass_max = (150.0 / df).round() as usize;

        // Mid: 150 - 2000 Hz
        let k_mid_min = k_bass_max + 1;
        let k_mid_max = (2000.0 / df).round() as usize;

        // High: 2000 - 12000 Hz
        let k_high_min = k_mid_max + 1;
        let k_high_max = (12000.0 / df).round().min((window_size / 2) as f32) as usize;

        // Precompute 16 log-spaced sub-band bin ranges from 30 Hz to 16000 Hz
        let mut sub_band_bounds = [(0, 0); 16];
        let f_min = 30.0f32;
        let f_max = 16000.0f32.min(sample_rate as f32 * 0.49);
        let log_ratio = (f_max / f_min).ln() / 16.0;

        for i in 0..16 {
            let f_start = f_min * (log_ratio * i as f32).exp();
            let f_end = f_min * (log_ratio * (i + 1) as f32).exp();
            let k_start = (f_start / df).floor().max(1.0) as usize;
            let k_end = (f_end / df).ceil().min((window_size / 2) as f32) as usize;
            sub_band_bounds[i] = (k_start, k_end.max(k_start));
        }

        let num_bins = window_size / 2 + 1; // 1025

        Self {
            sample_rate,
            window_size,
            sliding_window: vec![0.0; window_size],
            hann_window,
            fft_forward,
            fft_scratch,
            fft_output,
            fft_input: vec![0.0; window_size],
            magnitudes: vec![0.0; num_bins],

            k_bass_min,
            k_bass_max,
            k_mid_min,
            k_mid_max,
            k_high_min,
            k_high_max,
            sub_band_bounds,

            bass_peak: 0.5,
            mid_peak: 0.5,
            high_peak: 0.5,
            spectrum_peaks: [0.5; 16],

            bass_history_avg: 0.0,
            prev_bass_level: 0.0,
            beat_impulse: 0.0,
            time_since_beat: 1.0, // Start ready to trigger
            beat_count: 0.0,
            bpm: 120.0,
        }
    }

    /// Process a new chunk of audio samples and return the updated std140 AudioUniforms.
    ///
    /// - `chunk`: slice of mono PCM samples (normalized to [-1.0, 1.0]).
    /// - `dt`: elapsed time in seconds for this chunk (e.g. 256 / 48000 ≈ 0.00533s).
    pub fn process_chunk(&mut self, chunk: &[f32], dt: f32) -> AudioUniforms {
        let chunk_len = chunk.len();
        if chunk_len == 0 {
            return AudioUniforms::default();
        }

        // 1. Calculate raw chunk RMS
        let mut sum_sq = 0.0f32;
        for &s in chunk {
            sum_sq += s * s;
        }
        let raw_rms = (sum_sq / chunk_len as f32).sqrt();

        // 2. Slide incoming samples into FIFO buffer
        if chunk_len >= self.window_size {
            self.sliding_window
                .copy_from_slice(&chunk[chunk_len - self.window_size..]);
        } else {
            self.sliding_window.copy_within(chunk_len..self.window_size, 0);
            self.sliding_window[self.window_size - chunk_len..].copy_from_slice(chunk);
        }

        // 3. Apply Hann window into FFT input buffer
        for i in 0..self.window_size {
            self.fft_input[i] = self.sliding_window[i] * self.hann_window[i];
        }

        // 4. Execute Real-to-Complex FFT
        let _ = self.fft_forward.process_with_scratch(
            &mut self.fft_input,
            &mut self.fft_output,
            &mut self.fft_scratch,
        );

        // 5. Calculate normalized magnitudes with coherent gain correction
        // Coherent gain for Hann window = 0.5 * N = 1024 / 2 = 512
        const NORM_FACTOR: f32 = 1.0 / 512.0;
        for k in 0..self.magnitudes.len() {
            let c = self.fft_output[k];
            self.magnitudes[k] = (c.re * c.re + c.im * c.im).sqrt() * NORM_FACTOR;
        }

        // 6. Integrate energy across 3 core frequency bands
        let raw_bass = Self::band_energy(&self.magnitudes, self.k_bass_min, self.k_bass_max);
        let raw_mid = Self::band_energy(&self.magnitudes, self.k_mid_min, self.k_mid_max);
        let raw_high = Self::band_energy(&self.magnitudes, self.k_high_min, self.k_high_max);

        // 7. Adaptive peak tracking with asymmetric attack / decay
        let alpha_rise = 1.0 - (-dt / TAU_RISE).exp();
        let alpha_decay = (-dt / TAU_DECAY).exp();

        let norm_bass = Self::update_band_peak(
            raw_bass,
            &mut self.bass_peak,
            alpha_rise,
            alpha_decay,
        );
        let norm_mid = Self::update_band_peak(
            raw_mid,
            &mut self.mid_peak,
            alpha_rise,
            alpha_decay,
        );
        let norm_high = Self::update_band_peak(
            raw_high,
            &mut self.high_peak,
            alpha_rise,
            alpha_decay,
        );

        // 8. Beat onset detection
        self.time_since_beat += dt;

        // Decay active beat impulse: e^(-dt / TAU_BEAT_DECAY)
        let beat_decay = (-dt / TAU_BEAT_DECAY).exp();
        self.beat_impulse *= beat_decay;

        // Update short-term bass moving average
        let alpha_avg = 1.0 - (-dt / 0.5).exp();
        self.bass_history_avg += (norm_bass - self.bass_history_avg) * alpha_avg;

        // Transient detection conditions
        let is_transient = self.time_since_beat > REFRACTORY_PERIOD
            && norm_bass > 0.30
            && norm_bass > self.prev_bass_level
            && norm_bass > (self.bass_history_avg * 1.20 + 0.05);

        if is_transient {
            self.beat_impulse = 1.0;
            if self.time_since_beat < 2.0 && self.time_since_beat > 0.25 {
                let detected_bpm = 60.0 / self.time_since_beat;
                if (60.0..=240.0).contains(&detected_bpm) {
                    self.bpm = self.bpm * 0.8 + detected_bpm * 0.2;
                }
            }
            self.time_since_beat = 0.0;
            self.beat_count += 1.0;
        }

        self.prev_bass_level = norm_bass;

        // 9. Calculate 16 log-spaced spectrum bins
        let mut spectrum_16 = [0.0f32; 16];
        for i in 0..16 {
            let (k_start, k_end) = self.sub_band_bounds[i];
            let raw_sub = Self::band_energy(&self.magnitudes, k_start, k_end);
            spectrum_16[i] = Self::update_band_peak(
                raw_sub,
                &mut self.spectrum_peaks[i],
                alpha_rise,
                alpha_decay,
            );
        }

        // 10. Construct std140 AudioUniforms
        let mut uniforms = AudioUniforms::new();
        uniforms.set_bands(norm_bass, norm_mid, norm_high, self.beat_impulse);
        uniforms.set_raw_levels(raw_bass, raw_mid, raw_high, raw_rms);
        uniforms.set_spectrum_from_slice(&spectrum_16);

        uniforms
    }

    /// Returns the sample rate configured for this engine.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Estimated tempo in beats per minute.
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Cumulative count of detected beat onset pulses.
    pub fn beat_count(&self) -> f32 {
        self.beat_count
    }

    /// Reset internal state, history, and peak tracking.
    pub fn reset(&mut self) {
        self.sliding_window.fill(0.0);
        self.bass_peak = 0.5;
        self.mid_peak = 0.5;
        self.high_peak = 0.5;
        self.spectrum_peaks.fill(0.5);
        self.bass_history_avg = 0.0;
        self.prev_bass_level = 0.0;
        self.beat_impulse = 0.0;
        self.time_since_beat = 1.0;
        self.beat_count = 0.0;
    }

    #[inline]
    fn band_energy(mags: &[f32], low: usize, high: usize) -> f32 {
        let mut sum = 0.0f32;
        let low_clamped = low.min(mags.len() - 1);
        let high_clamped = high.min(mags.len() - 1);
        for k in low_clamped..=high_clamped {
            sum += mags[k] * mags[k];
        }
        sum.sqrt()
    }

    #[inline]
    fn update_band_peak(
        raw: f32,
        peak: &mut f32,
        alpha_rise: f32,
        alpha_decay: f32,
    ) -> f32 {
        if raw > *peak {
            *peak += (raw - *peak) * alpha_rise;
        } else {
            *peak = (*peak * alpha_decay).max(GATE);
        }

        if raw < NOISE_FLOOR {
            0.0
        } else {
            (raw / peak.max(GATE)).clamp(0.0, 1.0)
        }
    }
}
