//! Lumex FX Uniforms & State Machine
//!
//! Provides std140-compliant 128-byte `FxUniforms` and host-side easing state machine
//! with tau relaxation and idle infinite zoom snapping.

use serde::{Deserialize, Serialize};

/// std140 compliant 128-byte FX Uniform Buffer layout matching PROJECT.md interface contract.
#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable, Serialize, Deserialize)]
pub struct FxUniforms {
    pub aspect_time: [f32; 4],   // x: aspect, y: time, z: delta_time, w: seed
    pub feedback: [f32; 4],      // x: echo_amount, y: echo_zoom, z: echo_rot, w: feedback_decay
    pub blur_glow: [f32; 4],     // x: blur_amount, y: glow_amount, z: glow_threshold, w: glow_intensity
    pub warp: [f32; 4],          // x: warp_amount, y: warp_scale, z: warp_time, w: warp_speed
    pub kaleidoscope: [f32; 4],  // x: segments (uKN), y: layout (uKLayout), z: spin (uKSpin), w: zoom (uKZoom)
    pub zoom: [f32; 4],          // x: zoom_ratio (uZRatio), y: zoom_u (uZu), z: zoom_twist (uZTwist), w: zoom_on
    pub grading: [f32; 4],       // x: exposure, y: contrast, z: saturation, w: vignette
    pub color_shift: [f32; 4],   // x: hue_rot, y: rgb_split, z: blackout, w: center_offset
}

impl Default for FxUniforms {
    fn default() -> Self {
        Self {
            aspect_time: [16.0 / 9.0, 0.0, 1.0 / 60.0, 0.0],
            feedback: [0.0, 1.0, 0.0, 0.0],
            blur_glow: [0.0, 0.0, 0.16, 1.6],
            warp: [0.0, 2.5, 0.0, 0.5],
            kaleidoscope: [0.0, 0.0, 0.0, 1.0],
            zoom: [2.0, 0.0, 0.0, 0.0],
            grading: [1.0, 1.0, 1.0, 0.25],
            color_shift: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

/// High-level effect parameters matching the frontend control UI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FxParams {
    pub speed: f32,
    pub zoom_speed: f32,
    pub zoom_depth: f32,
    pub zoom_twist: f32,
    pub k_n: f32,
    pub k_spin: f32,
    pub k_rot: f32,
    pub k_zoom: f32,
    pub k_cx: f32,
    pub k_cy: f32,
    pub warp: f32,
    pub warp_scale: f32,
    pub warp_speed: f32,
    pub blur: f32,
    pub glow: f32,
    pub sat: f32,
    pub hue: f32,
    pub hue_spin: f32,
    pub exposure: f32,
    pub contrast: f32,
    pub echo: f32,
    pub echo_zoom: f32,
    pub echo_twist: f32,
    pub rgb: f32,
    pub vig: f32,
}

impl Default for FxParams {
    fn default() -> Self {
        Self {
            speed: 1.0,
            zoom_speed: 0.0,
            zoom_depth: 1.0,
            zoom_twist: 0.0,
            k_n: 0.0,
            k_spin: 0.0,
            k_rot: 0.0,
            k_zoom: 1.0,
            k_cx: 0.0,
            k_cy: 0.0,
            warp: 0.0,
            warp_scale: 2.5,
            warp_speed: 0.5,
            blur: 0.0,
            glow: 0.0,
            sat: 1.0,
            hue: 0.0,
            hue_spin: 0.0,
            exposure: 1.0,
            contrast: 1.0,
            echo: 0.0,
            echo_zoom: 0.0,
            echo_twist: 0.0,
            rgb: 0.0,
            vig: 0.25,
        }
    }
}

impl FxParams {
    pub fn ease_towards(&mut self, target: &Self, k: f32) {
        self.speed += (target.speed - self.speed) * k;
        self.zoom_speed += (target.zoom_speed - self.zoom_speed) * k;
        self.zoom_depth += (target.zoom_depth - self.zoom_depth) * k;
        self.zoom_twist += (target.zoom_twist - self.zoom_twist) * k;
        // k_n is discrete step
        self.k_n = target.k_n;
        self.k_spin += (target.k_spin - self.k_spin) * k;
        self.k_rot += (target.k_rot - self.k_rot) * k;
        self.k_zoom += (target.k_zoom - self.k_zoom) * k;
        self.k_cx += (target.k_cx - self.k_cx) * k;
        self.k_cy += (target.k_cy - self.k_cy) * k;
        self.warp += (target.warp - self.warp) * k;
        self.warp_scale += (target.warp_scale - self.warp_scale) * k;
        self.warp_speed += (target.warp_speed - self.warp_speed) * k;
        self.blur += (target.blur - self.blur) * k;
        self.glow += (target.glow - self.glow) * k;
        self.sat += (target.sat - self.sat) * k;
        self.hue += (target.hue - self.hue) * k;
        self.hue_spin += (target.hue_spin - self.hue_spin) * k;
        self.exposure += (target.exposure - self.exposure) * k;
        self.contrast += (target.contrast - self.contrast) * k;
        self.echo += (target.echo - self.echo) * k;
        self.echo_zoom += (target.echo_zoom - self.echo_zoom) * k;
        self.echo_twist += (target.echo_twist - self.echo_twist) * k;
        self.rgb += (target.rgb - self.rgb) * k;
        self.vig += (target.vig - self.vig) * k;
    }
}

/// Host-side FX State Machine with tau relaxation and phase accumulation.
#[derive(Clone, Debug)]
pub struct FxStateMachine {
    pub target: FxParams,
    pub cur: FxParams,
    pub spin_acc: f32,
    pub hue_acc: f32,
    pub zoom_acc: f32,
    pub warp_t: f32,
    pub black: f32,
    pub black_target: f32,
    pub tau: f32,
    pub time: f32,
    pub seed: f32,
    pub echo_warm: bool,
}

impl Default for FxStateMachine {
    fn default() -> Self {
        Self {
            target: FxParams::default(),
            cur: FxParams::default(),
            spin_acc: 0.0,
            hue_acc: 0.0,
            zoom_acc: 0.0,
            warp_t: 0.0,
            black: 0.0,
            black_target: 0.0,
            tau: 0.08,
            time: 0.0,
            seed: 0.0,
            echo_warm: false,
        }
    }
}

impl FxStateMachine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_target(&mut self, target: FxParams, glide: Option<f32>) {
        self.target = target;
        if let Some(g) = glide {
            self.tau = g;
        }
    }

    pub fn set_blackout(&mut self, black: f32) {
        self.black_target = black.clamp(0.0, 1.0);
    }

    /// Advance simulation by delta time `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        let sim_dt = dt * self.cur.speed;
        let k = 1.0 - (-dt / self.tau.max(0.001)).exp();
        self.cur.ease_towards(&self.target, k);
        self.tau += (0.08 - self.tau) * (1.0 - (-dt / 1.5).exp());

        self.spin_acc += self.cur.k_spin * sim_dt;
        self.hue_acc += self.cur.hue_spin * sim_dt;
        self.warp_t += self.cur.warp_speed * sim_dt;

        // Infinite zoom phase accumulation & idle snapping
        let zs = self.cur.zoom_speed;
        self.zoom_acc += zs * sim_dt;
        let idle = 1.0 - (zs.abs() / 0.01).clamp(0.0, 1.0);
        if idle > 0.0 {
            self.zoom_acc += (self.zoom_acc.round() - self.zoom_acc) * (1.0 - (-sim_dt * 2.0 * idle).exp());
        }

        self.black += (self.black_target - self.black) * (1.0 - (-dt / 0.12).exp());
        self.time += dt;
        self.seed = (self.seed + 7.13) % 1000.0;

        if self.cur.echo > 0.001 {
            self.echo_warm = true;
        } else {
            self.echo_warm = false;
        }
    }

    /// Build std140-aligned uniform buffer payload for current frame.
    pub fn build_uniforms(&self, width: u32, height: u32, dt: f32) -> FxUniforms {
        let aspect = width as f32 / height.max(1) as f32;
        let frames = (dt.min(0.1) * 60.0).max(0.0);

        let echo_amt = if self.echo_warm && self.cur.echo > 0.001 {
            self.cur.echo.powf(frames)
        } else {
            0.0
        };
        let echo_zoom = 1.0 + self.cur.echo_zoom * frames;
        let echo_rot = self.cur.echo_twist * frames;

        let zu = self.zoom_acc - self.zoom_acc.floor();
        let zoom_on = if zu > 0.002 && zu < 0.998 { 1.0 } else { 0.0 };
        let z_ratio = 2.0f32.powf(self.cur.zoom_depth);
        let z_twist = self.cur.zoom_twist.to_radians() * self.cur.zoom_depth;

        FxUniforms {
            aspect_time: [aspect, self.time, dt, self.seed],
            feedback: [echo_amt, echo_zoom, echo_rot, 0.0],
            blur_glow: [self.cur.blur, self.cur.glow, 0.16, 1.6],
            warp: [self.cur.warp, self.cur.warp_scale, self.warp_t, self.cur.warp_speed],
            kaleidoscope: [
                self.cur.k_n,
                self.cur.k_rot.to_radians(),
                self.spin_acc.to_radians(),
                self.cur.k_zoom,
            ],
            zoom: [z_ratio, zu, z_twist, zoom_on],
            grading: [self.cur.exposure, self.cur.contrast, self.cur.sat, self.cur.vig],
            color_shift: [
                (self.cur.hue + self.hue_acc).to_radians(),
                self.cur.rgb,
                self.black,
                0.0,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fx_uniforms_layout_and_size() {
        assert_eq!(std::mem::size_of::<FxUniforms>(), 128);
        assert_eq!(std::mem::align_of::<FxUniforms>(), 16);
    }

    #[test]
    fn test_state_machine_easing_and_idle_zoom() {
        let mut sm = FxStateMachine::new();
        sm.target.zoom_speed = 0.5;
        sm.step(0.016);
        assert!(sm.cur.zoom_speed > 0.0);
        assert!(sm.zoom_acc > 0.0);

        // Turn zoom speed to 0 and verify snapping towards whole cycle
        sm.target.zoom_speed = 0.0;
        for _ in 0..100 {
            sm.step(0.016);
        }
        let frac = sm.zoom_acc - sm.zoom_acc.round();
        assert!(frac.abs() < 0.05, "Fraction {} should snap towards whole cycle", frac);
    }
}
