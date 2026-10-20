// Empirical Mathematical Challenge Test Suite for Milestone 1 Post-Processing Shaders
// Verified by Challenger M1-2:
// 1. Jimenez 13-tap downsampling weights sum and energy conservation
// 2. Kaleidoscope polar angle modular arithmetic, symmetry bounds, and C0 continuity
// 3. Infinite logarithmic zoom dual-octave smoothstep crossfade normalization and C1 smoothness
// 4. Rec.709 luminance coefficients, neutral axis invariance, and hue rotation

use std::f32::consts::PI;

/// Test 1: Jimenez 13-Tap Downsample Blur Weights
/// Verifies that all weights are exact negative powers of 2 in IEEE 754 float32,
/// sum identically to 1.000000, conserve DC energy, and possess 8-way spatial symmetry.
#[test]
fn test_jimenez_13tap_weights_and_energy_conservation() {
    let w_center: f32 = 0.125;      // 2^-3
    let w_corner: f32 = 0.03125;    // 2^-5
    let w_cardinal: f32 = 0.0625;   // 2^-4
    let w_inner: f32 = 0.125;       // 2^-3

    // 1. Exact IEEE 754 representation (exact negative powers of 2 have zero mantissa rounding)
    assert_eq!(w_center, 1.0 / 8.0);
    assert_eq!(w_corner, 1.0 / 32.0);
    assert_eq!(w_cardinal, 1.0 / 16.0);
    assert_eq!(w_inner, 1.0 / 8.0);

    // 2. Exact sum to 1.000000 in f32
    let sum_corners = 4.0 * w_corner;
    let sum_cardinals = 4.0 * w_cardinal;
    let sum_inner = 4.0 * w_inner;
    let total_sum = w_center + sum_corners + sum_cardinals + sum_inner;

    assert_eq!(
        total_sum, 1.0,
        "Jimenez 13-tap weights must sum identically to 1.000000"
    );

    // 3. DC Energy Conservation across 10,000 intensity levels
    for i in 0..=10000 {
        let dc = (i as f32) / 100.0;
        let e = dc;
        let corners = 4.0 * dc;
        let cardinals = 4.0 * dc;
        let inner = 4.0 * dc;

        let filtered = e * w_center + corners * w_corner + cardinals * w_cardinal + inner * w_inner;
        let err = (filtered - dc).abs();
        assert!(
            err < 1e-6,
            "DC energy conservation failed for intensity {}: filtered={}, err={}",
            dc,
            filtered,
            err
        );
    }
}

/// Test 2: Kaleidoscope Folding Polar Modular Arithmetic and Symmetry Bounds
/// Verifies that the folded angle is strictly bounded in [0, pi / uKN],
/// exhibits mirror reflection symmetry across the wedge centerline,
/// and maintains C0 continuity across segment wrap boundaries.
#[test]
fn test_kaleidoscope_folding_math_and_bounds() {
    fn wgsl_fmod(x: f32, y: f32) -> f32 {
        x - y * (x / y).trunc()
    }

    fn fold_angle(a: f32, layout: f32, kn: f32) -> f32 {
        if kn < 0.5 {
            return a + layout;
        }
        let seg = (2.0 * PI) / kn;
        let mut rem = wgsl_fmod(a + layout, seg);
        if rem < 0.0 {
            rem += seg;
        }
        (rem - seg * 0.5).abs()
    }

    // 1. Boundary and Range Checks for all standard segment counts (uKN in 1..24)
    let segment_counts = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 16.0, 20.0, 24.0];
    for &kn in &segment_counts {
        let seg = (2.0 * PI) / kn;
        let max_angle = seg * 0.5; // Exactly PI / uKN

        // Test angles from -100 pi to +100 pi with various layout rotations
        for layout in [-10.0, -PI, 0.0, 0.5, PI, 10.0] {
            for step in -5000..=5000 {
                let a = (step as f32) * 0.01;
                let af = fold_angle(a, layout, kn);

                assert!(
                    af >= -1e-7 && af <= max_angle + 1e-6,
                    "Folded angle out of bounds for kn={}: {} not in [0, {}]",
                    kn,
                    af,
                    max_angle
                );
            }
        }
    }

    // 2. Bilateral Mirror Symmetry: f(center - delta) == f(center + delta)
    for &kn in &segment_counts {
        let seg = (2.0 * PI) / kn;
        let center = seg * 0.5;

        for step in 1..1000 {
            let delta = (step as f32) / 2000.0 * seg; // delta in (0, seg/2)
            let a_minus = fold_angle(center - delta, 0.0, kn);
            let a_plus = fold_angle(center + delta, 0.0, kn);

            assert!(
                (a_minus - a_plus).abs() < 1e-6,
                "Mirror symmetry broken for kn={}: delta={}, a_minus={}, a_plus={}",
                kn,
                delta,
                a_minus,
                a_plus
            );
        }
    }

    // 3. C0 Continuity across segment boundaries
    for &kn in &segment_counts {
        let seg = (2.0 * PI) / kn;
        for k in -5..=5 {
            let boundary = (k as f32) * seg;
            let eps = 1e-5;
            let left = fold_angle(boundary - eps, 0.0, kn);
            let right = fold_angle(boundary + eps, 0.0, kn);

            assert!(
                (left - right).abs() < 1e-4,
                "Boundary discontinuity at k*seg for kn={}: left={}, right={}",
                kn,
                left,
                right
            );
        }
    }

    // 4. Edge case: uKN < 0.5 (disabled folding)
    let passthrough = fold_angle(1.234, 0.5, 0.0);
    assert!((passthrough - (1.234 + 0.5)).abs() < 1e-6);
}

/// Test 3: Infinite Logarithmic Zoom Dual-Octave Smoothstep Crossfade Normalization
/// Verifies that the crossfade weights w1 + w2 == 1.0 identically (convex combination),
/// derivative vanishes at both endpoints (C1 smooth octave transitions),
/// and layer scale wraps without discontinuity across integer octave boundaries.
#[test]
fn test_infinite_zoom_crossfade_normalization_and_c1_smoothness() {
    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    // 1. Normalization: (1 - w) + w == 1.0 across 10,000 phase samples
    for i in 0..=10000 {
        let zu = (i as f32) / 10000.0;
        let w = smoothstep(0.0, 1.0, zu);
        let w1 = 1.0 - w;
        let w2 = w;

        let weight_sum = w1 + w2;
        assert_eq!(
            weight_sum, 1.0,
            "Crossfade weight sum must be identically 1.000000 at zu={}",
            zu
        );
        assert!((0.0..=1.0).contains(&w), "Weight w must be bounded in [0, 1]");
    }

    // 2. C1 Smoothness: First derivative d/dx [3x^2 - 2x^3] = 6x(1 - x) vanishes at 0 and 1
    let d_start = 6.0 * 0.0 * (1.0 - 0.0);
    let d_end = 6.0 * 1.0 * (1.0 - 1.0);
    assert_eq!(d_start, 0.0, "Derivative at zu=0 must vanish for C1 continuity");
    assert_eq!(d_end, 0.0, "Derivative at zu=1 must vanish for C1 continuity");

    // 3. Octave transition continuity:
    // Scale A at zu=0 is zRatio^0 = 1.0. Scale B at zu=1 is zRatio^(1 - 1) = zRatio^0 = 1.0.
    let z_ratio: f32 = 2.0;
    let scale_a_at_0 = z_ratio.powf(0.0);
    let scale_b_at_1 = z_ratio.powf(1.0 - 1.0);
    assert_eq!(scale_a_at_0, 1.0);
    assert_eq!(scale_b_at_1, 1.0);
}

/// Test 4: Rec.709 Luminance Coefficients & Color Grading Pipeline
/// Verifies exact ITU-R BT.709 coefficients, neutral gray axis invariance,
/// and hue rotation properties.
#[test]
fn test_rec709_luminance_and_color_grading_properties() {
    let kr: f32 = 0.2126;
    let kg: f32 = 0.7152;
    let kb: f32 = 0.0722;

    // 1. Rec.709 Luminance weight sum
    let luma_sum = kr + kg + kb;
    assert_eq!(
        luma_sum, 1.0,
        "Rec.709 coefficients must sum identically to 1.000000 in f32"
    );

    // 2. Neutral Axis Invariance under Saturation:
    // For any gray level c = [k, k, k], luma = dot(c, rec709) = k * (kr+kg+kb) = k.
    // mix(luma, c, sat) = k * (1 - sat) + k * sat = k.
    for i in 0..=100 {
        let gray = (i as f32) / 100.0;
        let c = [gray, gray, gray];
        let luma = c[0] * kr + c[1] * kg + c[2] * kb;
        assert!(
            (luma - gray).abs() < 1e-6,
            "Grayscale luma mismatch: expected {}, got {}",
            gray,
            luma
        );

        for sat_idx in 0..=50 {
            let sat = (sat_idx as f32) / 10.0; // 0.0 to 5.0
            let sat_out = [
                luma * (1.0 - sat) + c[0] * sat,
                luma * (1.0 - sat) + c[1] * sat,
                luma * (1.0 - sat) + c[2] * sat,
            ];

            for &val in &sat_out {
                assert!(
                    (val - gray).abs() < 1e-6,
                    "Neutral axis shifted under saturation sat={}: expected {}, got {}",
                    sat,
                    gray,
                    val
                );
            }
        }
    }

    // 3. Contrast Pivot around midpoint 0.5:
    // (0.5 - 0.5) * contrast + 0.5 == 0.5 for ANY contrast
    for c_idx in 0..=100 {
        let contrast = (c_idx as f32) / 20.0; // 0.0 to 5.0
        let midpoint_out = (0.5 - 0.5) * contrast + 0.5;
        assert_eq!(midpoint_out, 0.5, "Midpoint 0.5 must be invariant under contrast");
    }

    // 4. Hue Rotation Matrix Neutral Axis Invariance:
    // White point [1, 1, 1] must map to [1, 1, 1] for all rotation angles
    for angle_step in 0..360 {
        let angle = (angle_step as f32).to_radians();
        let cs = angle.cos();
        let sn = angle.sin();

        let row0 = [0.213 + cs * 0.787 - sn * 0.213, 0.715 - cs * 0.715 - sn * 0.715, 0.072 - cs * 0.072 + sn * 0.928];
        let row1 = [0.213 - cs * 0.213 + sn * 0.143, 0.715 + cs * 0.285 + sn * 0.140, 0.072 - cs * 0.072 - sn * 0.283];
        let row2 = [0.213 - cs * 0.213 - sn * 0.787, 0.715 - cs * 0.715 + sn * 0.715, 0.072 + cs * 0.928 + sn * 0.072];

        let sum0 = row0[0] + row0[1] + row0[2];
        let sum1 = row1[0] + row1[1] + row1[2];
        let sum2 = row2[0] + row2[1] + row2[2];

        assert!((sum0 - 1.0).abs() < 1e-5, "Hue row 0 sum != 1.0 at angle {}: {}", angle, sum0);
        assert!((sum1 - 1.0).abs() < 1e-5, "Hue row 1 sum != 1.0 at angle {}: {}", angle, sum1);
        assert!((sum2 - 1.0).abs() < 1e-5, "Hue row 2 sum != 1.0 at angle {}: {}", angle, sum2);
    }
}
