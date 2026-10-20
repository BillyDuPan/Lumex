//! Deterministic mock audio feed tests verifying FFT band separation,
//! beat onset detection, refractory holdoff, and lock-free SPSC ring buffer streaming.

use app_lib::audio::analysis::AudioAnalysisEngine;
use app_lib::audio::uniforms::AudioUniforms;
use rtrb::RingBuffer;
use std::f32::consts::PI;

/// Synthesize a pure sine wave PCM chunk without phase discontinuity.
fn generate_sine_chunk(
    freq: f32,
    sample_rate: u32,
    num_samples: usize,
    phase: &mut f32,
) -> Vec<f32> {
    let mut samples = Vec::with_capacity(num_samples);
    let phase_step = 2.0 * PI * freq / (sample_rate as f32);
    for _ in 0..num_samples {
        samples.push(phase.sin());
        *phase += phase_step;
        if *phase > 2.0 * PI {
            *phase -= 2.0 * PI;
        }
    }
    samples
}

#[test]
fn test_audio_uniforms_contract_and_alignment() {
    assert_eq!(
        std::mem::size_of::<AudioUniforms>(),
        96,
        "AudioUniforms must be exactly 96 bytes std140"
    );
    assert_eq!(
        std::mem::align_of::<AudioUniforms>(),
        16,
        "AudioUniforms must be 16-byte aligned"
    );

    let mut u = AudioUniforms::default();
    u.set_bands(0.9, 0.45, 0.15, 1.0);
    assert_eq!(u.bass(), 0.9);
    assert_eq!(u.mid(), 0.45);
    assert_eq!(u.high(), 0.15);
    assert_eq!(u.beat(), 1.0);
    assert_eq!(u.bands[0], 0.9);
    assert_eq!(u.bands[1], 0.45);
    assert_eq!(u.bands[2], 0.15);
    assert_eq!(u.bands[3], 1.0);
}

#[test]
fn test_fft_band_separation() {
    let sample_rate = 48000;
    let chunk_size = 256;
    let dt = chunk_size as f32 / sample_rate as f32; // ~5.33 ms
    let mut engine = AudioAnalysisEngine::new(sample_rate);

    // 1. Test Bass Isolation (60 Hz pure tone)
    let mut phase = 0.0;
    let mut uniforms = AudioUniforms::default();
    // Warm up analysis window over 2048 samples (16 chunks = 4096 samples)
    for _ in 0..16 {
        let chunk = generate_sine_chunk(60.0, sample_rate, chunk_size, &mut phase);
        uniforms = engine.process_chunk(&chunk, dt);
    }
    assert!(
        uniforms.bass() > 0.7,
        "Bass band must detect 60 Hz tone: bass={}",
        uniforms.bass()
    );
    assert!(
        uniforms.mid() < 0.15,
        "Mid band must reject 60 Hz tone: mid={}",
        uniforms.mid()
    );
    assert!(
        uniforms.high() < 0.10,
        "High band must reject 60 Hz tone: high={}",
        uniforms.high()
    );

    // 2. Test Mid Isolation (1000 Hz pure tone)
    phase = 0.0;
    for _ in 0..16 {
        let chunk = generate_sine_chunk(1000.0, sample_rate, chunk_size, &mut phase);
        uniforms = engine.process_chunk(&chunk, dt);
    }
    assert!(
        uniforms.mid() > 0.7,
        "Mid band must detect 1000 Hz tone: mid={}",
        uniforms.mid()
    );
    assert!(
        uniforms.bass() < 0.15,
        "Bass band must reject 1000 Hz tone: bass={}",
        uniforms.bass()
    );
    assert!(
        uniforms.high() < 0.15,
        "High band must reject 1000 Hz tone: high={}",
        uniforms.high()
    );

    // 3. Test High Isolation (6000 Hz pure tone)
    phase = 0.0;
    for _ in 0..16 {
        let chunk = generate_sine_chunk(6000.0, sample_rate, chunk_size, &mut phase);
        uniforms = engine.process_chunk(&chunk, dt);
    }
    assert!(
        uniforms.high() > 0.7,
        "High band must detect 6000 Hz tone: high={}",
        uniforms.high()
    );
    assert!(
        uniforms.bass() < 0.10,
        "Bass band must reject 6000 Hz tone: bass={}",
        uniforms.bass()
    );
    assert!(
        uniforms.mid() < 0.15,
        "Mid band must reject 6000 Hz tone: mid={}",
        uniforms.mid()
    );
}

#[test]
fn test_beat_onset_detection_and_refractory_period() {
    let sample_rate = 48000;
    let mut engine = AudioAnalysisEngine::new(sample_rate);
    let dt = 256.0 / (sample_rate as f32); // ~5.33 ms
    let mut beat_triggers = 0;

    // Simulate 2 seconds of audio with kick transients spaced at 500 ms (120 BPM)
    let total_frames = (2.0 * sample_rate as f32 / 256.0) as usize;

    for frame_idx in 0..total_frames {
        let t = frame_idx as f32 * dt;
        let time_since_beat = t % 0.5; // Kick hits every 0.5s

        // Synthesize kick: loud 80Hz exponential burst during first 40ms of each beat
        let chunk = if time_since_beat < 0.04 {
            let mut p = 0.0;
            generate_sine_chunk(80.0, sample_rate, 256, &mut p)
                .iter()
                .map(|&s| s * (1.0 - time_since_beat / 0.04))
                .collect()
        } else {
            vec![0.0; 256]
        };

        let uniforms = engine.process_chunk(&chunk, dt);
        if uniforms.beat() > 0.95 {
            beat_triggers += 1;
        }
    }

    // Over 2 seconds at 120 BPM, exactly 3-4 beats should trigger
    assert!(
        beat_triggers >= 3 && beat_triggers <= 4,
        "Expected 3-4 beat triggers over 2.0s at 120 BPM, got {}",
        beat_triggers
    );
}

#[test]
fn test_silence_produces_zero_uniforms_without_nan() {
    let sample_rate = 48000;
    let mut engine = AudioAnalysisEngine::new(sample_rate);
    let dt = 256.0 / (sample_rate as f32);

    let silence = vec![0.0f32; 256];
    for _ in 0..32 {
        let u = engine.process_chunk(&silence, dt);
        assert_eq!(u.bass(), 0.0);
        assert_eq!(u.mid(), 0.0);
        assert_eq!(u.high(), 0.0);
        assert_eq!(u.beat(), 0.0);
        assert_eq!(u.raw_rms(), 0.0);
        assert!(!u.bass().is_nan());
        assert!(!u.mid().is_nan());
        assert!(!u.high().is_nan());
        assert!(!u.beat().is_nan());
    }
}

#[test]
fn test_lockfree_ring_buffer_streaming() {
    let sample_rate = 48000;
    let (mut prod, mut cons) = RingBuffer::<f32>::new(4096);
    let mut engine = AudioAnalysisEngine::new(sample_rate);
    let dt = 256.0 / sample_rate as f32;

    // Simulate audio thread pushing 1024 samples
    let mut phase = 0.0;
    let audio_data = generate_sine_chunk(440.0, sample_rate, 1024, &mut phase);
    for &sample in &audio_data {
        assert!(prod.push(sample).is_ok());
    }

    // Worker thread pops 256-sample chunks and analyzes them
    let mut chunks_processed = 0;
    let mut chunk_buf = [0.0f32; 256];

    while cons.slots() >= 256 {
        for sample in &mut chunk_buf {
            *sample = cons.pop().expect("Sample available");
        }
        let u = engine.process_chunk(&chunk_buf, dt);
        assert!(u.input_level() > 0.0);
        chunks_processed += 1;
    }

    assert_eq!(chunks_processed, 4);
    assert_eq!(cons.slots(), 0);
}
