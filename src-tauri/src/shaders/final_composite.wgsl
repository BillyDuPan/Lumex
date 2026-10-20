// Lumex Native Post-Processing Pipeline - Final Composite Shader
// Features: Liquid Warp, Kaleidoscope Folding, Infinite Logarithmic Zoom,
// Radial RGB Chromatic Aberration, Filmic Color Grading, Vignette, Dither

struct FxUniforms {
    aspect_time: vec4<f32>,   // x: aspect, y: time, z: delta_time, w: seed
    feedback: vec4<f32>,      // x: echo_amount, y: echo_zoom, z: echo_rot, w: feedback_decay
    blur_glow: vec4<f32>,     // x: blur_amount, y: glow_amount, z: glow_threshold, w: glow_intensity
    warp: vec4<f32>,          // x: warp_amount, y: warp_scale, z: warp_time, w: warp_speed
    kaleidoscope: vec4<f32>,  // x: segments (uKN), y: layout (uKLayout), z: spin (uKSpin), w: zoom (uKZoom)
    zoom: vec4<f32>,          // x: zoom_ratio (uZRatio), y: zoom_u (uZu), z: zoom_twist (uZTwist), w: zoom_on
    grading: vec4<f32>,       // x: exposure, y: contrast, z: saturation, w: vignette
    color_shift: vec4<f32>,   // x: hue_rot, y: rgb_split, z: blackout, w: center_offset
};

@group(0) @binding(0) var<uniform> fx: FxUniforms;
@group(0) @binding(1) var u_sampler: sampler;
@group(0) @binding(2) var u_src: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = -1.0 + f32((vertex_index & 1u) << 2u);
    let y = -1.0 + f32((vertex_index & 2u) << 1u);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    // Origin top-left, y down convention
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Liquid ripple on the source picture. Applied after the fold, so mirror symmetry survives.
fn warpUv(s_in: vec2<f32>) -> vec2<f32> {
    let uWarp = fx.warp.x;
    let uWarpScale = fx.warp.y;
    let uWarpT = fx.warp.z;
    let uAspect = fx.aspect_time.x;

    if (uWarp < 0.0001) {
        return s_in;
    }
    var s = s_in;
    let q = s * vec2<f32>(uAspect, 1.0) * uWarpScale;
    s.x += uWarp * (sin(q.y * 2.1 + uWarpT) + 0.5 * sin(q.y * 4.3 - uWarpT * 1.3 + q.x));
    s.y += uWarp * (sin(q.x * 1.9 - uWarpT * 0.9) + 0.5 * sin(q.x * 3.7 + uWarpT * 1.1 + q.y));
    return s;
}

// Maps screen position uv to source picture coordinate through kaleidoscope folding and zoom.
// Mirror lines are fixed on screen; spin and twist turn the picture inside each segment.
fn sourceUv(uv: vec2<f32>, zoom: f32, twist: f32) -> vec2<f32> {
    let uAspect = fx.aspect_time.x;
    let uKN = fx.kaleidoscope.x;
    let uKLayout = fx.kaleidoscope.y;
    let uKSpin = fx.kaleidoscope.z;
    let uKZoom = select(1.0, fx.kaleidoscope.w, fx.kaleidoscope.w > 0.0001);
    let uKC = vec2<f32>(0.0, 0.0); // Center offset

    let asp = vec2<f32>(uAspect, 1.0);
    let piv = vec2<f32>(0.5, 0.5) + uKC;
    let p = (uv - piv) * asp / uKZoom;

    if (uKN < 0.5 && (abs(uKLayout) + abs(uKSpin) + abs(twist) < 0.00001)) {
        return warpUv(piv + p / (zoom * asp));
    }

    let r = length(p);
    var a = atan2(p.y, p.x);

    if (uKN >= 0.5) {
        let seg = 6.28318530718 / uKN;
        var rem = (a + uKLayout) % seg;
        if (rem < 0.0) {
            rem += seg;
        }
        a = abs(rem - seg * 0.5);
    } else {
        a += uKLayout;
    }

    a += uKSpin + twist;
    return warpUv(piv + vec2<f32>(cos(a), sin(a)) * (r / zoom) / asp);
}

// Evaluates a single picture layer with radial RGB split chromatic aberration
fn layer(uv: vec2<f32>, zoom: f32, twist: f32) -> vec3<f32> {
    let s = sourceUv(uv, zoom, twist);
    let uRgb = fx.color_shift.y;
    if (uRgb < 0.0005) {
        return textureSample(u_src, u_sampler, s).rgb;
    }
    let d = (s - 0.5) * (uRgb * 2.0);
    let r = textureSample(u_src, u_sampler, s + d).r;
    let g = textureSample(u_src, u_sampler, s).g;
    let b = textureSample(u_src, u_sampler, s - d).b;
    return vec3<f32>(r, g, b);
}

// Infinite logarithmic zoom: two octave copies one depth apart cross-fade continuously
fn sceneZ(uv: vec2<f32>) -> vec3<f32> {
    let uZRatio = fx.zoom.x;
    let uZu = fx.zoom.y;
    let uZTwist = fx.zoom.z;
    let uZoomOn = fx.zoom.w;

    if (uZoomOn < 0.5) {
        return layer(uv, 1.0, 0.0);
    }

    let w = smoothstep(0.0, 1.0, uZu);
    let a = layer(uv, pow(uZRatio, uZu), uZTwist * uZu);
    if (w < 0.004) {
        return a;
    }
    let b = layer(uv, pow(uZRatio, uZu - 1.0), uZTwist * (uZu - 1.0));
    return mix(a, b, w);
}

// 3x3 Hue rotation matrix
fn hueRotate(c: vec3<f32>, a: f32) -> vec3<f32> {
    let cs = cos(a);
    let sn = sin(a);
    let row0 = vec3<f32>(0.213 + cs * 0.787 - sn * 0.213, 0.715 - cs * 0.715 - sn * 0.715, 0.072 - cs * 0.072 + sn * 0.928);
    let row1 = vec3<f32>(0.213 - cs * 0.213 + sn * 0.143, 0.715 + cs * 0.285 + sn * 0.140, 0.072 - cs * 0.072 - sn * 0.283);
    let row2 = vec3<f32>(0.213 - cs * 0.213 - sn * 0.787, 0.715 - cs * 0.715 + sn * 0.715, 0.072 + cs * 0.928 + sn * 0.072);
    return vec3<f32>(dot(row0, c), dot(row1, c), dot(row2, c));
}

// High-frequency spatial hash for anti-banding dithering
fn hash21(p_in: vec2<f32>) -> f32 {
    let p = fract(p_in * vec2<f32>(443.897, 441.423));
    let p2 = p + dot(p, p.yx + 19.19);
    return fract((p2.x + p2.y) * p2.x);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var c = sceneZ(in.uv);

    let uExposure = fx.grading.x;
    let uContrast = fx.grading.y;
    let uSat = fx.grading.z;
    let uVig = fx.grading.w;

    let uHue = fx.color_shift.x;
    let uBlack = fx.color_shift.z;
    let uAspect = fx.aspect_time.x;
    let uSeed = fx.aspect_time.w;

    // 1. Filmic exposure adjustment
    c *= uExposure;

    // 2. Filmic contrast expansion around midpoint
    c = (c - 0.5) * uContrast + 0.5;
    c = max(c, vec3<f32>(0.0));

    // 3. 3x3 Hue matrix rotation
    if (abs(uHue) > 0.0001) {
        c = hueRotate(c, uHue);
    }

    // 4. Rec.709 Luminance-weighted Saturation
    let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(l), c, uSat);

    // 5. Smoothstep Vignette
    let v = length((in.uv - 0.5) * vec2<f32>(uAspect, 1.0));
    c *= 1.0 - uVig * smoothstep(0.3, 0.9, v);

    // 6. Master Blackout
    c *= 1.0 - uBlack;

    // 7. Spatial hash dither to prevent 8-bit quantization banding
    let dither = (hash21(in.position.xy + vec2<f32>(uSeed, uSeed)) - 0.5) * (1.5 / 255.0);
    c += vec3<f32>(dither);

    return vec4<f32>(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
