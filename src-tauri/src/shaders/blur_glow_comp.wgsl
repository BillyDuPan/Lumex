// Lumex Native Post-Processing Pipeline - Tent Filter Upsampling & Bloom Glow Composite
// Combines multi-scale blur pyramid levels with 4-tap tent smoothing and HDR thresholded bloom

struct CompUniforms {
    blur_glow: vec4<f32>, // x: blur, y: glow, z: threshold, w: intensity
    t2: vec2<f32>,        // Texel size of mip level 2 (1/W2, 1/H2)
    t3: vec2<f32>,        // Texel size of mip level 3 (1/W3, 1/H3)
    t4: vec2<f32>,        // Texel size of mip level 4 (1/W4, 1/H4)
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: CompUniforms;
@group(0) @binding(1) var u_sampler: sampler;
@group(0) @binding(2) var u_base: texture_2d<f32>;
@group(0) @binding(3) var u_l1: texture_2d<f32>;
@group(0) @binding(4) var u_l2: texture_2d<f32>;
@group(0) @binding(5) var u_l3: texture_2d<f32>;
@group(0) @binding(6) var u_l4: texture_2d<f32>;

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
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// 4-tap tent upsampling filter at half-texel offsets to eliminate coarse-mip blocking
fn soft(t: texture_2d<f32>, texel: vec2<f32>, uv: vec2<f32>) -> vec3<f32> {
    let o = texel * 0.5;
    let s0 = textureSample(t, u_sampler, uv + vec2<f32>(-o.x, -o.y)).rgb;
    let s1 = textureSample(t, u_sampler, uv + vec2<f32>( o.x, -o.y)).rgb;
    let s2 = textureSample(t, u_sampler, uv + vec2<f32>(-o.x,  o.y)).rgb;
    let s3 = textureSample(t, u_sampler, uv + vec2<f32>( o.x,  o.y)).rgb;
    return (s0 + s1 + s2 + s3) * 0.25;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let uBlur = u.blur_glow.x;
    let uGlow = u.blur_glow.y;

    // Piecewise linear interpolation across the 4 blur pyramid levels
    let f = clamp(uBlur, 0.0, 1.0) * 4.0;
    var c: vec3<f32>;

    if (f <= 0.0) {
        c = textureSample(u_base, u_sampler, uv).rgb;
    } else if (f < 1.0) {
        c = mix(textureSample(u_base, u_sampler, uv).rgb, textureSample(u_l1, u_sampler, uv).rgb, f);
    } else if (f < 2.0) {
        c = mix(textureSample(u_l1, u_sampler, uv).rgb, soft(u_l2, u.t2, uv), f - 1.0);
    } else if (f < 3.0) {
        c = mix(soft(u_l2, u.t2, uv), soft(u_l3, u.t3, uv), f - 2.0);
    } else {
        c = mix(soft(u_l3, u.t3, uv), soft(u_l4, u.t4, uv), f - 3.0);
    }

    // Bloom glow extraction with luminance thresholding and high-dynamic-range scaling
    if (uGlow > 0.001) {
        let g = soft(u_l2, u.t2, uv) * 0.40 + soft(u_l3, u.t3, uv) * 0.35 + soft(u_l4, u.t4, uv) * 0.25;
        let threshold = select(0.16, u.blur_glow.z, u.blur_glow.z > 0.0);
        let intensity = select(1.6, u.blur_glow.w, u.blur_glow.w > 0.0);
        c += max(g - vec3<f32>(threshold), vec3<f32>(0.0)) * (intensity * uGlow);
    }

    return vec4<f32>(c, 1.0);
}
