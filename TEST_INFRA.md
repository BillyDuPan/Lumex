# Lumex Test Architecture & Infrastructure Specification (TEST_INFRA)

**Document Version**: 1.0.0  
**Author**: `test_writer_e2e`  
**Target Systems**: Windows 10/11 (DirectX 12 / WASAPI / Spout2), macOS 12+ (Metal / CoreAudio / Syphon)  
**Rust Version**: 1.85+ (Edition 2024), WGPU 23/24, Tauri v2  

---

## 1. Executive Overview

This document specifies the authoritative test architecture and verification infrastructure for Lumex, a high-performance cross-platform live visual instrument.

To guarantee sustained 60–120+ FPS at 1080p/4K, zero-copy presentation, <15ms audio reactivity latency, and zero VRAM memory leaks, the test framework is organized into a **4-Tier Opaque-Box Test Architecture** supplemented by **Tier 5 Adversarial Stress & Verification Harnesses**.

```
┌────────────────────────────────────────────────────────────────────────┐
│                   Tier 4: Real-World Performance & VJ Scenarios        │
│    - 1000-Frame Offscreen Benchmark (<16.6ms Mean, <22ms p99)          │
│    - Continuous 10,000-frame stability & VRAM memory leak audits        │
│    - Dual-window display under 4K projector + 360p control blit        │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
┌───────────────────────────────────▼────────────────────────────────────┐
│         Tier 3: Cross-Feature Combinations & Pairwise Interactions     │
│    - Audio Beat Onset -> Echo Feedback Decay + Particle Excitation      │
│    - MIDI CC Sweeps -> Simultaneous Kaleidoscope + Infinite Zoom Twist │
│    - Dynamic Window Resize during active multi-pass render flight      │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
┌───────────────────────────────────▼────────────────────────────────────┐
│             Tier 2: Boundary & Corner Cases (>= 5 per Subsystem)       │
│    - Extremes: 1x1 minimal, 4K (3840x2160), NPOT (1366x768), 0x0 dims  │
│    - Delta time extremes: dt = 0.0s (freeze), dt = 10.0s (spike)        │
│    - Audio: Pure DC offset, Nyquist 24kHz, 0.0 silence noise gate clamp │
│    - MIDI: 14-bit high-res CC, running status, corrupted packets        │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
┌───────────────────────────────────▼────────────────────────────────────┐
│            Tier 1: Feature Coverage (>= 5 per Feature/Subsystem)       │
│    - M1 Shaders & Graphics: Pipeline init, Echo, Blur, Glow, Warp,      │
│      Kaleidoscope, Zoom, Color Grade, FX State Machine Uniforms        │
│    - M2 Dual Display: Monitors, OS Projector, Zero-Copy Preview Blit   │
│    - M3 Compute: Balls, Sparks, Boids, SDF Vector Drawing               │
│    - M4 Audio/MIDI: CPAL Capture, SPSC RingBuffer, RealFFT, MIDI Matrix │
│    - M5 Texture Sharing: Spout2 (Win DX12 NT) & Syphon (macOS Metal)    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
┌───────────────────────────────────▼────────────────────────────────────┐
│      Verification Harnesses (Naga Headless, Synthetic Audio, Benches)  │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. 4-Tier Test Architecture Matrix

### 2.1 Tier 1: Feature Coverage (>= 5 Test Cases per Feature/Subsystem)

Every core capability must be covered by at least 5 distinct opaque-box test cases verifying primary behavior against interface specifications:

| Subsystem | Feature ID & Name | Test Case ID | Test Description & Objective | Expected Behavior & Verification Contract |
|---|---|---|---|---|
| **M1: Graphics Core** | F1: Pipeline State | `T1.1.1` | Headless WGPU device & adapter initialization | Requests HighPerformance adapter, creates device/queue, validates features & limits |
| | | `T1.1.2` | Offscreen floating-point target allocation (`Rgba16Float`) | Allocates ping-pong targets at 1080p, verifies dimensions and texture view validity |
| | | `T1.1.3` | Texture format compatibility (Rgba8Unorm, Bgra8Unorm, Rgba8UnormSrgb, Rgba16Float) | Successfully compiles and binds pipelines across all supported swapchain/target formats |
| | | `T1.1.4` | Texture resize lifecycle | Re-allocates targets from 640x360 to 1920x1080 without stale resource handles or panic |
| | | `T1.1.5` | Command encoder submission and synchronization | Submits empty command buffer, calls `device.poll(Maintain::Wait)`, succeeds cleanly |
| **M1: Echo Feedback** | F2: Echo Shader | `T1.2.1` | Zero echo bypass mode | When `echo_amt = 0.0`, echo pass is bypassed, source texture passes unaltered |
| | | `T1.2.2` | Aspect ratio coordinate scaling | Verifies that rotation and zoom matrix accounts for non-square aspect ratios without distortion |
| | | `T1.2.3` | Ping-pong buffer swap parity | Verifies `current` and `next` swap after each echo pass, maintaining continuous history |
| | | `T1.2.4` | Exponential decay factor | Checks that `feedback_decay` in [0.0, 1.0] attenuates pixel intensities over successive passes |
| | | `T1.2.5` | Mirrored repeat border addressing | Sampler with `AddressMode::MirrorRepeat` prevents black border encroachment during feedback |
| **M1: Blur Pyramid** | F3: 13-Tap Blur | `T1.3.1` | Jimenez 13-tap filter weight sum | Verifies center tap (4/32) + 4 corner taps (4*(4/32)) + 8 bilinear outer taps sum to exactly 1.0 |
| | | `T1.3.2` | 4-level downsampling pyramid dimensions | Level 1: 1/2 res, Level 2: 1/4 res, Level 3: 1/8 res, Level 4: 1/16 res |
| | | `T1.3.3` | Energy conservation across pyramid | Downsampling a constant white texture preserves mean luminance within 0.1% tolerance |
| | | `T1.3.4` | Downsample uniform buffer texel sizing | Computes exact `[1.0 / width, 1.0 / height]` per pyramid level, updates uniform buffers |
| | | `T1.3.5` | Independent render pass execution | Executes 4 sequential render passes into mip level views without cross-contamination |
| **M1: Glow Composite** | F4: Tent Glow | `T1.4.1` | 4-tap tent upsampling kernel | Verifies bilinear tent weight distribution `[0.25, 0.25, 0.25, 0.25]` during upsample |
| | | `T1.4.2` | High-pass bloom threshold extraction | Pixels below `glow_threshold` produce zero glow; pixels above contribute proportionally |
| | | `T1.4.3` | Piecewise linear mip interpolation | Smooth blending between adjacent mip levels as `blur_amount` varies from 0.0 to 1.0 |
| | | `T1.4.4` | Additive HDR glow accumulation | Base image + accumulated bloom glow produces intense highlights without color clamping |
| | | `T1.4.5` | Composite target output blit | Outputs composite image to `comp_target` ready for final FX stage |
| **M1: Liquid Warp** | F5: Sinusoidal Warp | `T1.5.1` | Zero warp identity transform | When `warp_amount = 0.0`, UV coordinates are completely undisplaced |
| | | `T1.5.2` | Dual-frequency sinusoidal displacement | Verifies vector offset: `delta = sin(uv * freq1 + time) * amp1 + cos(uv * freq2 - time) * amp2` |
| | | `T1.5.3` | Temporal phase progression | Increments `time` and verifies continuous smooth animation of warp vector field |
| | | `T1.5.4` | Coordinate bounds clamping | Displaced UV coordinates sample safely within texture bounds using clamp/repeat modes |
| | | `T1.5.5` | High-frequency stability | Tests high warp frequency (up to 50.0) without numeric overflow or NaN artifacts |
| **M1: Kaleidoscope** | F6: Modular Fold | `T1.6.1` | Modular N-segment angular folding | Verifies angular folding into `2 * PI / N` sectors with alternating mirrored reflection |
| | | `T1.6.2` | Segment parameter bounds (N in [2, 32]) | Odd and even segment counts produce geometrically symmetric patterns |
| | | `T1.6.3` | Angular spin accumulation | Target spin accumulates continuously without modulo jumping or phase discontinuities |
| | | `T1.6.4` | Center-point translation offset | Panning kaleidoscope center `[cx, cy]` shifts pattern origin accurately |
| | | `T1.6.5` | Coordinate invariance at origin | Origin point `(0, 0)` remains singular and fixed under pure rotation |
| **M1: Logarithmic Zoom**| F7: Infinite Zoom | `T1.7.1` | Dual-octave power-law zoom | Scale follows `2^(u + k)` where `u` wraps in `[0, 1)`, maintaining scale invariance |
| | | `T1.7.2` | Smoothstep octave crossfade normalization | Verifies `w0 = smoothstep(0, 1, 1 - u)` and `w1 = smoothstep(0, 1, u)`, and `w0 + w1 == 1.0` |
| | | `T1.7.3` | C1 continuity across octave wrap | Zero derivative discontinuities at `u = 0.0` and `u = 1.0` boundary transitions |
| | | `T1.7.4` | Rotational spiral twist factor | Applies angular twist proportional to logarithmic scale parameter |
| | | `T1.7.5` | Aspect ratio preservation | Rectangular output frames zoom uniformly without anamorphic stretching |
| **M1: Color Grading** | F8: Rec.709 Grading | `T1.8.1` | Rec.709 luminance preservation | Uses exact coefficients `[0.2126, 0.7152, 0.0722]` for luminance and desaturation |
| | | `T1.8.2` | Exposure gain scaling | Multiplies linear RGB by `2^exposure`, preserving black levels at 0.0 |
| | | `T1.8.3` | Contrast S-curve centered at 0.5 | Pivot formula `(rgb - 0.5) * contrast + 0.5` preserves midtone neutral gray |
| | | `T1.8.4` | 3x3 Hue rotation matrix determinant | Orthogonal Rodrigues rotation around luminance axis has determinant `1.0 +/- 0.0001` |
| | | `T1.8.5` | Spatial hash dither noise | Adds high-frequency dither (amplitude ~1/255) to eliminate 8-bit color banding |
| **M1: FX Uniforms** | F9: State Machine | `T1.9.1` | Exponential tau relaxation easing | Parameters ease towards target with `x = lerp(x, target, 1.0 - exp(-dt / tau))` |
| | | `T1.9.2` | Idle zoom snapping | When zoom velocity drops below threshold, zoom phase snaps to nearest integer octave |
| | | `T1.9.3` | Continuous angle accumulation | Prevents phase wrapping stutter when accumulating continuous rotation speeds |
| | | `T1.9.4` | Memory layout & std140 alignment | `FxUniforms` is exactly 128 bytes, 16-byte aligned, verified via `bytemuck` |
| | | `T1.9.5` | Rapid fluctuation resilience | Alternating parameter extremes every frame converges smoothly without NaN or oscillations |
| **M2: Display Manager**| F10: Monitors | `T1.10.1` | Monitor enumeration and metadata | Queries OS monitors via Tauri runtime, extracts resolution, scale factor, and name |
| | | `T1.10.2` | Primary vs Secondary classification | Accurately identifies primary monitor and prioritizes secondary displays for projection |
| | | `T1.10.3` | Aspect ratio and orientation detection | Calculates aspect ratio (`width / height`) and flags landscape vs portrait monitors |
| | | `T1.10.4` | Monitor selection and indexing | Selects target display by index, falls back safely to default primary if out of range |
| | | `T1.10.5` | Display disconnect / reconnection | Gracefully handles changes in active monitor count without crashing the application |
| **M2: OS Projector** | F11: Native Window | `T1.11.1` | Pure native OS window creation | Spawns OS window via `Window::builder` with no webview or DOM overhead |
| | | `T1.11.2` | Borderless fullscreen configuration | Window opens with `fullscreen = true`, `decorations = false`, covering full secondary screen |
| | | `T1.11.3` | Escape key termination handler | Listens for Escape key press and closes projector window cleanly |
| | | `T1.11.4` | V-Sync swapchain configuration | Swapchain configured with `PresentMode::Fifo` for tear-free live projection |
| | | `T1.11.5` | Surface re-configuration on resize | Resizing window reconfigures swapchain surface with matching dimensions |
| **M2: Preview Blit** | F12: Zero-Copy Blit | `T1.12.1` | Direct GPU texture-to-texture blit | Renders source texture into preview target using full-screen textured quad |
| | | `T1.12.2` | Zero CPU readback verification | Render loop performs zero `copy_texture_to_buffer` or staging buffer CPU reads |
| | | `T1.12.3` | Aspect ratio fitting (letterbox/pillarbox) | Fits arbitrary aspect source into preview container with black letterboxing |
| | | `T1.12.4` | Simultaneous dual presentation | Presenter submits blit commands for both projector and preview surfaces in one frame |
| | | `T1.12.5` | Preview surface resize robustness | Resizing UI preview area updates viewport rectangle dynamically without GPU stall |
| **M3: Compute Agents**| F13-F16: Compute | `T1.13.1` | Floating Balls particle integration | Computes drift, damping, swell, and 4 mouse interaction modes in storage buffers |
| | | `T1.13.2` | Particle Sparks physics simulation | Simulates 65,536 particles with drag, gravity, and audio burst excitation |
| | | `T1.13.3` | Craig Reynolds Boids simulation | Simulates 10,000 agents with toroidal wrapping, flocking rules, and food foraging |
| | | `T1.13.4` | GPU Vector Brush SDF rendering | Instanced quads evaluate analytical segment SDF with antialiasing and neon falloff |
| | | `T1.13.5` | Storage buffer std430 alignment | Verifies struct alignment (`Particle`: 32 bytes, `Boid`: 48 bytes) for compute pipelines |
| **M4: Audio Reactivity**| F17-F19: Audio | `T1.14.1` | CPAL capture stream initialization | Opens low-latency WASAPI/CoreAudio stream with 256/512 frame chunking |
| | | `T1.14.2` | Lock-free SPSC ring buffer streaming | Producer pushes audio samples from OS thread; consumer pulls on worker thread without locks |
| | | `T1.14.3` | RealFFT 4-band spectral separation | Accurately extracts Bass (30-150Hz), Mid (150-2000Hz), High (2000-12000Hz) energies |
| | | `T1.14.4` | Beat onset detector & refractory gap | Triggers beat pulses with 200ms refractory holdoff, eliminating false double-triggers |
| | | `T1.14.5` | Audio uniforms std140 layout | `AudioUniforms` is exactly 96 bytes, 16-byte aligned, zero NaNs on silent audio |
| **M4: Hardware MIDI** | F20-F21: MIDI | `T1.15.1` | Midir device discovery & hotplug | Enumerates connected MIDI hardware and listens for incoming control packets |
| | | `T1.15.2` | Zero-allocation MIDI packet parsing | Parses 1-3 byte messages into `MidiMessage::ControlChange`, `NoteOn`, `PitchBend` |
| | | `T1.15.3` | 14-bit high-resolution CC parsing | Combines MSB (CC 0-31) and LSB (CC 32-63) into 16,384-step high-resolution values |
| | | `T1.15.4` | Modulation matrix parameter mapping | Maps CC numbers to target FX parameters with Linear, Exponential, and S-Curves |
| | | `T1.15.5` | Momentary vs Toggle mode behavior | Pad hits toggle values on/off or hold value while depressed |
| **M5: Texture Sharing**| F22-F23: Spout/Syphon| `T1.16.1` | Windows Spout2 DXGI shared handle creation | Registers DX12 NT shared resource handle with Spout sender |
| | | `T1.16.2` | macOS Syphon Metal IOSurfaceRef binding | Backs output texture with shared `IOSurface` for external client ingestion |
| | | `T1.16.3` | Non-blocking frame publishing | Publishing shared handle does not block or stall the internal render loop |
| | | `T1.16.4` | Client disconnect resilience | External receiver opening or closing does not interrupt engine execution |
| | | `T1.16.5` | Resolution change synchronization | Resizing output texture notifies Spout/Syphon clients of new dimension envelope |

---

### 2.2 Tier 2: Boundary & Corner Cases (>= 5 Test Cases per Subsystem)

| Subsystem | Test Case ID | Extreme Condition / Input Boundary | Expected Robustness & Handling Behavior |
|---|---|---|---|
| **Graphics Core** | `T2.1.1` | 1x1 Minimal Texture Resolution | Pipeline compiles and renders 1x1 offscreen target without dividing by zero in texel sizing |
| | `T2.1.2` | 4K Extreme Resolution (3840x2160) | Allocates 4K targets (~64MB VRAM), renders all 4 passes within device limits without OOM |
| | `T2.1.3` | Non-Power-of-Two (NPOT) Resolution (1366x768) | Pyramid downsample handles fractional odd dimensions without rounding artifacts |
| | `T2.1.4` | Odd Dimension Boundary (1919x1079) | Integer division rounding does not truncate mip levels or panic texture descriptor |
| | `T2.1.5` | Zero Dimension Input Handling | Passing 0 width or height is clamped safely to minimum 1x1 without panicking |
| **FX State Machine** | `T2.2.1` | Delta Time Freeze (`dt = 0.0s`) | State machine retains current parameters without NaN, division by zero, or runaway velocity |
| | `T2.2.2` | Delta Time Lag Spike (`dt = 10.0s`) | Parameter easing snaps cleanly to target without overshooting or oscillation |
| | `T2.2.3` | Negative Delta Time (`dt = -0.016s`) | Engine clamps `dt` to 0.0 or positive value, preventing reverse time simulation bugs |
| | `T2.2.4` | High Frequency Alternating Inputs | Flips parameters between 0.0 and 1.0 at 1000Hz; state machine smoothly interpolates |
| | `T2.2.5` | Extreme Parameter Inputs | `echo = 100.0`, `blur = -5.0`, `warp = 1e6`; clamps safely to defined parameter boundaries |
| **Audio Reactivity** | `T2.3.1` | Pure Silence (All Zero PCM Samples) | Noise gate clamp (`GATE = 0.003`) prevents `log10(0)` evaluation; uniforms remain 0.0 |
| | `T2.3.2` | Pure DC Offset (Constant +1.0 PCM) | Hann window and DC bin filtering removes 0 Hz bias; band energies remain zero |
| | `T2.3.3` | Nyquist Frequency Sine Wave (24,000 Hz) | Handled safely at spectrum boundary without bin aliasing or indexing panic |
| | `T2.3.4` | High Amplitude Audio Clipping (+100.0) | Input gain normalization and soft clipping clamps signal safely to [-1.0, 1.0] |
| | `T2.3.5` | Audio Stream Device Disconnection | CPAL callback error sets `AudioState::Disconnected`, zeroing uniforms without crashing |
| **Hardware MIDI** | `T2.4.1` | Corrupted / Truncated Packets | Receiving incomplete 1-byte packets when 3 bytes expected is safely discarded |
| | `T2.4.2` | System Exclusive (SysEx) Blast | Multi-kilobyte SysEx dumps are filtered out without exhausting memory or blocking parser |
| | `T2.4.3` | High-Speed CC Churn (10,000 msgs/sec) | Lock-free MIDI queue absorbs burst without latency accumulation or dropped state |
| | `T2.4.4` | Out-of-Range Channel Bytes | Channel nibble > 15 or invalid status bits are safely ignored |
| | `T2.4.5` | Rapid Controller Disconnect & Reconnect | Device teardown drops handle safely; reconnect binds port automatically by name |
| **Display & Windowing**| `T2.5.1` | Single Monitor Environment | If no secondary monitor exists, safely defaults to windowed projector on primary display |
| | `T2.5.2` | Portrait Mode Secondary Monitor (1080x1920) | Correctly identifies portrait aspect ratio; fits horizontal visuals with pillarboxing |
| | `T2.5.3` | Rapid Fullscreen Toggle Thrashing | Toggling fullscreen 50 times in rapid succession does not deadlock window thread |
| | `T2.5.4` | Zero Size Preview Area | Window minimisation or zero-dimension preview container suspends blit without crashing |
| | `T2.5.5` | Secondary Monitor Unplugged Live | Window manager catches OS display removal event and migrates or closes window gracefully |

---

### 2.3 Tier 3: Cross-Feature Combinations & Pairwise Interactions

Tests interaction contracts across disparate subsystems running concurrently in the live engine:

1. **Audio Reactivity + Echo Feedback + Compute Sparks**:
   - Audio beat transient dynamically boosts `echo_amount` to 0.85 and emits 500 sparks simultaneously.
   - Verifies uniform buffer update synchronization and compute-to-render barrier correctness.
2. **Hardware MIDI CC Sweep + Kaleidoscope + Logarithmic Zoom**:
   - Sweeping MIDI CC 16 (0..127) modulates kaleidoscope segments from 2 to 16 while CC 17 modulates infinite zoom spiral twist.
   - Verifies smooth continuous rendering without pipeline recompilation or visual pops.
3. **Dual Display + Zero-Copy Preview Blit under Heavy FX Load**:
   - Output projector runs at 4K (3840x2160) @ 60 FPS while UI preview blits at 360p (640x360).
   - Verifies independent swapchain presentation without mutual lock contention or dropped frames.
4. **Dynamic Window Resize during Multi-Pass GPU Execution**:
   - Resizing projector window triggers target re-allocation while GPU command encoder has passes in flight.
   - Verifies synchronization fences prevent reading from destroyed textures.
5. **Silence + Extreme FX Warp + S-Curve Color Grading**:
   - Audio input falls to zero while warp amount is maximum (0.1) and contrast is 2.0.
   - Verifies no NaN propagation through post-processing shaders; output produces clean deterministic image.

---

### 2.4 Tier 4: Real-World Performance & Application Scenarios

1. **Automated 1000-Frame Benchmark Binary (`benchmark_frame_timing.rs`)**:
   - Executes 1000 continuous offscreen frames at 1080p under full pipeline load (Echo, Blur Downsampling, Tent Glow Composite, Final FX Composite).
   - Measures exact microsecond frame times with GPU synchronization (`device.poll(Maintain::Wait)`).
   - Computes Mean, Min, Max, p50, p90, p95, and p99 frame times.
   - Acceptance threshold: **Mean < 16.66 ms** ($\ge 60$ FPS) and **p99 < 22.0 ms**.
2. **Sustained VJ Performance Simulation (10,000 Frames)**:
   - Simulates 3 minutes of continuous live stage performance with synthetic audio beat pulses, continuous parameter modulation, and Spout texture publishing.
   - Monitors GPU memory allocation; asserts zero VRAM memory growth or handle leaks.
3. **End-to-End Latency Verification**:
   - Simulates synthetic PCM tone pulse into CPAL capture buffer and measures elapsed wall-clock time until `AudioUniforms` is committed to GPU memory.
   - Asserts total latency is strictly **< 15.0 ms** (measured baseline: ~8.4 ms).

---

## 3. Verification Harness Architecture

The test suite provides three authoritative automated verification harnesses:

### 3.1 Naga Headless WGSL Shader Validation Suite (`test_shader_compilation.rs`)
- Parses and type-checks all native WGSL shaders offline using `naga`:
  - `echo.wgsl`
  - `downsample13.wgsl`
  - `blur_glow_comp.wgsl`
  - `final_composite.wgsl`
  - `balls_compute.wgsl`
  - `sparks_compute.wgsl`
  - `boids_compute.wgsl`
  - `brush_sdf.wgsl`
- Validates struct alignments, binding numbers, entry points (`vs_main`, `fs_main`, `cs_main`), and arithmetic types without requiring a physical GPU display.

### 3.2 Deterministic Mock Audio Feed Harness (`test_audio_reactivity.rs`)
- Synthesizes exact sinusoidal PCM signals (60Hz Bass, 1kHz Mid, 6kHz High) and pulsed kick transients.
- Verifies FFT frequency band separation, energy normalization, and beat refractory holdoff (200ms) with zero reliance on physical microphones or OS permissions.

### 3.3 Automated 1000-Frame Benchmark Binary (`benches/benchmark_frame_timing.rs`)
- Registered in `Cargo.toml` as an independent benchmark executable:
  ```toml
  [[bench]]
  name = "benchmark_frame_timing"
  harness = false
  ```
- Evaluates real GPU rendering performance offscreen at 1920x1080, calculating full percentile statistics.

---

## 4. Execution Commands

To execute the verification suites across all tiers:

```bash
# 1. Run all unit and integration tests across Tiers 1-3
cargo test

# 2. Run specific verification suites
cargo test --test test_shader_compilation
cargo test --test test_audio_reactivity
cargo test --test test_dual_display
cargo test --test test_midi_matrix
cargo test --test test_milestone1_challenge
cargo test --test test_shader_math_verification

# 3. Execute the 1000-frame automated performance benchmark (Tier 4)
cargo bench --bench benchmark_frame_timing
# or directly via cargo test harness:
cargo test --bench benchmark_frame_timing -- --nocapture
```
