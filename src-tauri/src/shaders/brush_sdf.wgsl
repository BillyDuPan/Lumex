// Analytical Signed Distance Field (SDF) Vector Brush Shader
// Evaluates instanced line segment bounding quads with exact sub-pixel antialiasing,
// multi-pass neon exponential glow, round caps, round joins, and eraser carving.

struct StrokeSegment {
    p0: vec2<f32>,
    p1: vec2<f32>,
    size: f32,
    birth_time: f32,
    lifetime: f32,
    style: u32,              // 0: solid, 1: neon, 2: soft, 3: erase
    color: vec4<f32>,
};

struct DrawUniforms {
    viewport_size: vec2<f32>,
    time: f32,
    aspect: f32,
    is_eraser: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<uniform> u: DrawUniforms;
@group(0) @binding(1) var<storage, read> segments: array<StrokeSegment>;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) p0: vec2<f32>,
    @location(2) p1: vec2<f32>,
    @location(3) size: f32,
    @location(4) @interpolate(flat) style: u32,
    @location(5) color: vec4<f32>,
    @location(6) birth_time: f32,
    @location(7) lifetime: f32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vid: u32,
    @builtin(instance_index) iid: u32,
) -> VertexOutput {
    let seg = segments[iid];
    var out: VertexOutput;

    // 6-vertex quad corners: (0, 1, 2) and (2, 1, 3)
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0)
    );
    let c = corners[vid];

    let v = seg.p1 - seg.p0;
    let l = length(v);
    var dir = vec2<f32>(1.0, 0.0);
    if (l > 1e-5) {
        dir = v / l;
    }
    let norm = vec2<f32>(-dir.y, dir.x);

    let radius = seg.size * 0.5;
    var pad = radius + 3.0;
    if (seg.style == 1u) {
        // Neon glow extends wider
        pad = radius * 3.5 + 8.0;
    } else if (seg.style == 2u) {
        // Soft airbrush extends wider
        pad = radius * 2.5 + 6.0;
    }

    // Anchor quad to p0 or p1 depending on corner x sign
    var anchor = seg.p0;
    var along = -dir * pad;
    if (c.x > 0.0) {
        anchor = seg.p1;
        along = dir * pad;
    }
    let across = norm * (c.y * pad);
    let world_pos = anchor + along + across;

    // Convert pixel coordinates to normalized device coordinates [-1, 1]
    let ndc_x = (world_pos.x / u.viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (world_pos.y / u.viewport_size.y) * 2.0;

    out.clip_pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.world_pos = world_pos;
    out.p0 = seg.p0;
    out.p1 = seg.p1;
    out.size = seg.size;
    out.style = seg.style;
    out.color = seg.color;
    out.birth_time = seg.birth_time;
    out.lifetime = seg.lifetime;

    return out;
}

// Analytical distance from point p to segment [a, b]
fn distance_to_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ba = b - a;
    let pa = p - a;
    let l2 = dot(ba, ba);
    let t = clamp(dot(pa, ba) / max(l2, 1e-6), 0.0, 1.0);
    return length(pa - ba * t);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = distance_to_segment(in.world_pos, in.p0, in.p1);
    let radius = in.size * 0.5;

    // Evaluate lifetime decay for live vanishing strokes
    var life_factor = 1.0;
    if (in.lifetime > 0.0) {
        let age = u.time - in.birth_time;
        if (age >= in.lifetime) {
            discard;
        }
        life_factor = clamp(1.0 - age / in.lifetime, 0.0, 1.0);
    }

    if (in.style == 1u) {
        // Multi-pass neon exponential glow
        let core = 1.0 - smoothstep(radius * 0.3 - 0.75, radius * 0.3 + 0.75, d);
        let mid = exp(-d / max(1.0, radius * 0.8)) * 0.7;
        let wide = exp(-d / max(1.0, radius * 2.5)) * 0.3;
        let glow = (mid + wide) * min(1.0, life_factor * 3.0);
        let rgb = mix(in.color.rgb, vec3<f32>(1.0, 1.0, 1.0), core);
        return vec4<f32>(rgb * glow, glow);
    } else if (in.style == 2u) {
        // Soft airbrush radial exponential falloff
        let sigma = max(1.0, radius * 0.5);
        let alpha = exp(-(d * d) / (2.0 * sigma * sigma)) * 0.55 * life_factor;
        return vec4<f32>(in.color.rgb * alpha, alpha);
    } else if (in.style == 3u || u.is_eraser == 1u) {
        // Eraser mask for destination-out blending
        let aa = 1.0 - smoothstep(radius - 0.75, radius + 0.75, d);
        let alpha = aa * min(1.0, life_factor * 2.5);
        return vec4<f32>(0.0, 0.0, 0.0, alpha);
    } else {
        // Solid vector stroke with sub-pixel analytical antialiasing
        let aa = 1.0 - smoothstep(radius - 0.75, radius + 0.75, d);
        let alpha = aa * in.color.a * min(1.0, life_factor * 2.5);
        return vec4<f32>(in.color.rgb * alpha, alpha);
    }
}
