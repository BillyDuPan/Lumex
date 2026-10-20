// Lumex Native Post-Processing Pipeline - 13-Tap Downsample Blur Pyramid Filter
// Exact Jimenez 13-tap filter with anti-aliasing and box/tent integration

struct DownsampleUniforms {
    texel_size: vec2<f32>, // (1.0 / W_in, 1.0 / H_in)
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: DownsampleUniforms;
@group(0) @binding(1) var u_sampler: sampler;
@group(0) @binding(2) var u_tex: texture_2d<f32>;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let o = u.texel_size;
    let uv = in.uv;

    // 4 outer corners at (+-2, +-2)
    let a = textureSample(u_tex, u_sampler, uv + o * vec2<f32>(-2.0, -2.0)).rgb;
    let c = textureSample(u_tex, u_sampler, uv + o * vec2<f32>( 2.0, -2.0)).rgb;
    let g = textureSample(u_tex, u_sampler, uv + o * vec2<f32>(-2.0,  2.0)).rgb;
    let i = textureSample(u_tex, u_sampler, uv + o * vec2<f32>( 2.0,  2.0)).rgb;

    // 4 outer cardinals at (+-2, 0) and (0, +-2)
    let b = textureSample(u_tex, u_sampler, uv + o * vec2<f32>( 0.0, -2.0)).rgb;
    let d = textureSample(u_tex, u_sampler, uv + o * vec2<f32>(-2.0,  0.0)).rgb;
    let f = textureSample(u_tex, u_sampler, uv + o * vec2<f32>( 2.0,  0.0)).rgb;
    let h = textureSample(u_tex, u_sampler, uv + o * vec2<f32>( 0.0,  2.0)).rgb;

    // 1 center tap at (0, 0)
    let e = textureSample(u_tex, u_sampler, uv).rgb;

    // 4 inner diagonals at (+-1, +-1)
    let j = textureSample(u_tex, u_sampler, uv + o * vec2<f32>(-1.0, -1.0)).rgb;
    let k = textureSample(u_tex, u_sampler, uv + o * vec2<f32>( 1.0, -1.0)).rgb;
    let l = textureSample(u_tex, u_sampler, uv + o * vec2<f32>(-1.0,  1.0)).rgb;
    let m = textureSample(u_tex, u_sampler, uv + o * vec2<f32>( 1.0,  1.0)).rgb;

    // Jimenez 13-tap weighted sum: 0.125 center + 4x0.03125 corners + 4x0.0625 cardinals + 4x0.125 inner diagonals
    let col = e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
    return vec4<f32>(col, 1.0);
}
