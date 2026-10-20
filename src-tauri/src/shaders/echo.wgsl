// Lumex Native Post-Processing Pipeline - Feedback Echo Accumulation
// Ported from WebGL2 FRAG_ECHO with 16-bit float ping-pong accumulation

struct FxUniforms {
    aspect_time: vec4<f32>,   // x: aspect, y: time, z: delta_time, w: seed
    feedback: vec4<f32>,      // x: echo_amount, y: echo_zoom, z: echo_rot, w: feedback_decay
    blur_glow: vec4<f32>,     // x: blur_amount, y: glow_amount, z: glow_threshold, w: glow_intensity
    warp: vec4<f32>,          // x: warp_amount, y: warp_scale, z: warp_time, w: warp_speed
    kaleidoscope: vec4<f32>,  // x: segments, y: layout, z: spin, w: zoom
    zoom: vec4<f32>,          // x: zoom_ratio, y: zoom_u, z: zoom_twist, w: zoom_on
    grading: vec4<f32>,       // x: exposure, y: contrast, z: saturation, w: vignette
    color_shift: vec4<f32>,   // x: hue_rot, y: rgb_split, z: blackout, w: center_offset
};

@group(0) @binding(0) var<uniform> fx: FxUniforms;
@group(0) @binding(1) var u_sampler: sampler;
@group(0) @binding(2) var u_src: texture_2d<f32>;
@group(0) @binding(3) var u_prev: texture_2d<f32>;

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
    // Origin top-left, y down convention:
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uAspect = fx.aspect_time.x;
    let uAmt = fx.feedback.x;
    let uZoom = fx.feedback.y;
    let uRot = fx.feedback.z;

    let asp = vec2<f32>(uAspect, 1.0);
    var p = (in.uv - 0.5) * asp;
    let c = cos(uRot);
    let s = sin(uRot);
    p = vec2<f32>(c * p.x - s * p.y, s * p.x + c * p.y) / uZoom;
    let prev_uv = (p / asp) + 0.5;

    let prev = textureSample(u_prev, u_sampler, prev_uv).rgb;
    let cur = textureSample(u_src, u_sampler, in.uv).rgb;

    return vec4<f32>(mix(cur, prev, uAmt), 1.0);
}
