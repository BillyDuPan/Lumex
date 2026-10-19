/*
 * WebGL2 post-processing for the composed scene:
 *   source canvas -> echo (feedback, only when used) -> blur pyramid (only when used) -> blur/glow mix
 *     -> kaleidoscope / infinite zoom / warp + colour grading -> screen
 *
 * UV convention everywhere: origin top-left, y down (same as canvas 2D), so the
 * JS foldPoint() in scene.js can mirror sourceUv() below exactly.
 *
 * Every draw call costs real time on a laptop GPU, so passes are kept to a minimum:
 * the pyramid is one downsample pass per level and blur/glow are mixed into one texture
 * so the final pass reads a single texture.
 */

const VERT = `#version 300 es
uniform float uFlip;
out vec2 vUv;
void main() {
  vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2) * 2.0 - 1.0;
  gl_Position = vec4(p, 0.0, 1.0);
  vUv = p * 0.5 + 0.5;
  if (uFlip > 0.5) vUv.y = 1.0 - vUv.y;
}`;

const HEAD = `#version 300 es
precision highp float;
in vec2 vUv;
out vec4 outColor;
`;

const FRAG_ECHO = HEAD + `
uniform sampler2D uSrc, uPrev;
uniform float uAmt, uZoom, uRot, uAspect;
void main() {
  vec2 asp = vec2(uAspect, 1.0);
  vec2 p = (vUv - 0.5) * asp;
  float c = cos(uRot), s = sin(uRot);
  p = vec2(c * p.x - s * p.y, s * p.x + c * p.y) / uZoom;
  vec3 prev = texture(uPrev, p / asp + 0.5).rgb;
  vec3 cur = texture(uSrc, vUv).rgb;
  outColor = vec4(mix(cur, prev, uAmt), 1.0);
}`;

// 13-tap downsample: smooth, alias-free, and each level is also a blur.
const FRAG_DOWN = HEAD + `
uniform sampler2D uTex;
uniform vec2 uTexel;
void main() {
  vec2 o = uTexel;
  vec3 a = texture(uTex, vUv + o * vec2(-2.0, -2.0)).rgb, b = texture(uTex, vUv + o * vec2(0.0, -2.0)).rgb, c = texture(uTex, vUv + o * vec2(2.0, -2.0)).rgb;
  vec3 d = texture(uTex, vUv + o * vec2(-2.0,  0.0)).rgb, e = texture(uTex, vUv).rgb,                       f = texture(uTex, vUv + o * vec2(2.0,  0.0)).rgb;
  vec3 g = texture(uTex, vUv + o * vec2(-2.0,  2.0)).rgb, h = texture(uTex, vUv + o * vec2(0.0,  2.0)).rgb, i = texture(uTex, vUv + o * vec2(2.0,  2.0)).rgb;
  vec3 j = texture(uTex, vUv + o * vec2(-1.0, -1.0)).rgb, k = texture(uTex, vUv + o * vec2(1.0, -1.0)).rgb;
  vec3 l = texture(uTex, vUv + o * vec2(-1.0,  1.0)).rgb, m = texture(uTex, vUv + o * vec2(1.0,  1.0)).rgb;
  vec3 col = e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;
  outColor = vec4(col, 1.0);
}`;

// Blur + glow of the base picture in one texture. Coarse levels are read with a small tent
// filter so upsampling them stays smooth.
const FRAG_COMP = HEAD + `
uniform sampler2D uBase, uL1, uL2, uL3, uL4;
uniform vec2 uT2, uT3, uT4;
uniform float uBlur, uGlow;
vec3 soft(sampler2D t, vec2 texel, vec2 uv) {
  vec2 o = texel * 0.5;
  return (texture(t, uv + vec2(-o.x, -o.y)).rgb + texture(t, uv + vec2(o.x, -o.y)).rgb
        + texture(t, uv + vec2(-o.x,  o.y)).rgb + texture(t, uv + vec2(o.x,  o.y)).rgb) * 0.25;
}
void main() {
  vec2 uv = vUv;
  float f = clamp(uBlur, 0.0, 1.0) * 4.0;
  vec3 c;
  if (f <= 0.0) c = texture(uBase, uv).rgb;
  else if (f < 1.0) c = mix(texture(uBase, uv).rgb, texture(uL1, uv).rgb, f);
  else if (f < 2.0) c = mix(texture(uL1, uv).rgb, soft(uL2, uT2, uv), f - 1.0);
  else if (f < 3.0) c = mix(soft(uL2, uT2, uv), soft(uL3, uT3, uv), f - 2.0);
  else c = mix(soft(uL3, uT3, uv), soft(uL4, uT4, uv), f - 3.0);
  if (uGlow > 0.001) {
    vec3 g = soft(uL2, uT2, uv) * 0.4 + soft(uL3, uT3, uv) * 0.35 + soft(uL4, uT4, uv) * 0.25;
    c += max(g - 0.16, 0.0) * 1.6 * uGlow;
  }
  outColor = vec4(c, 1.0);
}`;

const FRAG_FINAL = HEAD + `
uniform sampler2D uSrc;
uniform float uAspect, uKN, uKLayout, uKSpin, uKZoom, uZoomOn, uZu, uZRatio, uZTwist, uWarp, uWarpScale, uWarpT;
uniform float uSat, uHue, uExposure, uContrast, uRgb, uVig, uBlack, uSeed;
uniform vec2 uKC;

// Liquid ripple on the source picture. Applied after the fold, so mirror symmetry survives.
vec2 warpUv(vec2 s) {
  if (uWarp < 0.0001) return s;
  vec2 q = s * vec2(uAspect, 1.0) * uWarpScale;
  s.x += uWarp * (sin(q.y * 2.1 + uWarpT) + 0.5 * sin(q.y * 4.3 - uWarpT * 1.3 + q.x));
  s.y += uWarp * (sin(q.x * 1.9 - uWarpT * 0.9) + 0.5 * sin(q.x * 3.7 + uWarpT * 1.1 + q.y));
  return s;
}

// Which spot of the source picture is shown at screen position uv.
// Mirror lines are fixed on screen; spin and twist turn the picture *inside* each segment,
// after the fold, so the seams stay continuous. Mirrors foldPoint() in scene.js.
vec2 sourceUv(vec2 uv, float zoom, float twist) {
  vec2 asp = vec2(uAspect, 1.0);
  vec2 piv = vec2(0.5) + uKC;
  vec2 p = (uv - piv) * asp / uKZoom;
  if (uKN < 0.5 && abs(uKLayout) + abs(uKSpin) + abs(twist) < 0.00001) return warpUv(piv + p / zoom / asp);
  float r = length(p);
  float a = atan(p.y, p.x);
  if (uKN >= 0.5) {
    float seg = 6.28318530718 / uKN;
    a = mod(a + uKLayout, seg);
    a = abs(a - seg * 0.5);
  } else {
    a += uKLayout;
  }
  a += uKSpin + twist;
  return warpUv(piv + vec2(cos(a), sin(a)) * (r / zoom) / asp);
}

// One copy of the picture, with the RGB split pushed outwards from the centre in source space.
vec3 layer(vec2 uv, float zoom, float twist) {
  vec2 s = sourceUv(uv, zoom, twist);
  if (uRgb < 0.0005) return texture(uSrc, s).rgb;
  vec2 d = (s - 0.5) * uRgb * 2.0;
  return vec3(texture(uSrc, s + d).r, texture(uSrc, s).g, texture(uSrc, s - d).b);
}

// Infinite zoom: two copies of the picture one "depth" apart cross-fade as the phase runs 0 -> 1.
// At phase 1 the second copy sits exactly where the first one started, so the loop has no seam.
vec3 sceneZ(vec2 uv) {
  if (uZoomOn < 0.5) return layer(uv, 1.0, 0.0);
  float w = smoothstep(0.0, 1.0, uZu);
  vec3 a = layer(uv, pow(uZRatio, uZu), uZTwist * uZu);
  if (w < 0.004) return a;
  vec3 b = layer(uv, pow(uZRatio, uZu - 1.0), uZTwist * (uZu - 1.0));
  return mix(a, b, w);
}

vec3 hueRotate(vec3 c, float a) {
  float cs = cos(a), sn = sin(a);
  return vec3(
    dot(vec3(0.213 + cs * 0.787 - sn * 0.213, 0.715 - cs * 0.715 - sn * 0.715, 0.072 - cs * 0.072 + sn * 0.928), c),
    dot(vec3(0.213 - cs * 0.213 + sn * 0.143, 0.715 + cs * 0.285 + sn * 0.140, 0.072 - cs * 0.072 - sn * 0.283), c),
    dot(vec3(0.213 - cs * 0.213 - sn * 0.787, 0.715 - cs * 0.715 + sn * 0.715, 0.072 + cs * 0.928 + sn * 0.072), c));
}

float hash(vec2 p) {
  p = fract(p * vec2(443.897, 441.423));
  p += dot(p, p.yx + 19.19);
  return fract((p.x + p.y) * p.x);
}

void main() {
  vec3 c = sceneZ(vUv);
  c *= uExposure;
  c = (c - 0.5) * uContrast + 0.5;
  c = max(c, 0.0);
  if (abs(uHue) > 0.0001) c = hueRotate(c, uHue);
  float l = dot(c, vec3(0.2126, 0.7152, 0.0722));
  c = mix(vec3(l), c, uSat);
  float v = length((vUv - 0.5) * vec2(uAspect, 1.0));
  c *= 1.0 - uVig * smoothstep(0.3, 0.9, v);
  c *= 1.0 - uBlack;
  c += (hash(gl_FragCoord.xy + uSeed) - 0.5) * (1.5 / 255.0);
  outColor = vec4(clamp(c, 0.0, 1.0), 1.0);
}`;

function compile(gl, type, src) {
  const s = gl.createShader(type);
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
  return s;
}

function program(gl, frag) {
  const p = gl.createProgram();
  gl.attachShader(p, compile(gl, gl.VERTEX_SHADER, VERT));
  gl.attachShader(p, compile(gl, gl.FRAGMENT_SHADER, frag));
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const u = {};
  const n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) {
    const name = gl.getActiveUniform(p, i).name;
    u[name] = gl.getUniformLocation(p, name);
  }
  return { p, u };
}

const LEVELS = 4;

export class Pipeline {
  constructor(canvas) {
    const gl = canvas.getContext('webgl2', { alpha: false, antialias: false, powerPreference: 'high-performance' });
    if (!gl) throw new Error('WebGL2 is not available');
    this.gl = gl;
    this.canvas = canvas;
    this.floatOk = !!(gl.getExtension('EXT_color_buffer_float') || gl.getExtension('EXT_color_buffer_half_float'));
    this.progs = {
      echo: program(gl, FRAG_ECHO), down: program(gl, FRAG_DOWN),
      comp: program(gl, FRAG_COMP), final: program(gl, FRAG_FINAL),
    };
    gl.bindVertexArray(gl.createVertexArray());
    this.srcTex = this.makeTexture();
    this.size = [0, 0];
    this.seed = 0;
    this.echoWarm = false;
  }

  makeTexture() {
    const gl = this.gl, t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.MIRRORED_REPEAT);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.MIRRORED_REPEAT);
    return t;
  }

  makeTarget(w, h, float = this.floatOk) {
    const gl = this.gl, tex = this.makeTexture();
    if (float) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA16F, w, h, 0, gl.RGBA, gl.HALF_FLOAT, null);
    else gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    const fbo = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    gl.clearColor(0, 0, 0, 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    return { tex, fbo, w, h };
  }

  allocate(W, H) {
    const gl = this.gl;
    for (const t of [...(this.echo || []), this.comp, ...(this.levels || [])]) {
      if (t) { gl.deleteTexture(t.tex); gl.deleteFramebuffer(t.fbo); }
    }
    // Feedback needs 16-bit precision or faint trails never fully decay; the blur mix does not.
    this.echo = [this.makeTarget(W, H), this.makeTarget(W, H)];
    this.echoIdx = 0;
    this.echoWarm = false;
    this.comp = this.makeTarget(W, H, false);
    this.levels = [];
    for (let i = 1; i <= LEVELS; i++) {
      this.levels.push(this.makeTarget(Math.max(2, Math.ceil(W / 2 ** i)), Math.max(2, Math.ceil(H / 2 ** i))));
    }
    this.size = [W, H];
  }

  use(prog, target, w, h) {
    const gl = this.gl;
    gl.useProgram(prog.p);
    gl.bindFramebuffer(gl.FRAMEBUFFER, target ? target.fbo : null);
    gl.viewport(0, 0, w, h);
    gl.uniform1f(prog.u.uFlip, target ? 0 : 1);
  }

  bind(unit, tex, loc) {
    const gl = this.gl;
    gl.activeTexture(gl.TEXTURE0 + unit);
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.uniform1i(loc, unit);
  }

  draw() { this.gl.drawArrays(this.gl.TRIANGLES, 0, 3); }

  /**
   * @param src   composed scene canvas
   * @param fx    effect values (see FxState.frame())
   * @param dt    real seconds since last frame (for frame-rate independent echo)
   * @param black 0..1 blackout amount
   */
  render(src, fx, dt, black = 0) {
    const gl = this.gl, W = src.width, H = src.height, A = W / H;
    if (W !== this.size[0] || H !== this.size[1]) this.allocate(W, H);
    const ow = this.canvas.width, oh = this.canvas.height;

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.srcTex);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, src);

    // Echo / feedback. Skipped entirely while unused; on the frame it starts, it seeds itself from the source.
    let base = { tex: this.srcTex, w: W, h: H };
    if (fx.echo > 0.001) {
      const frames = Math.min(dt, 0.1) * 60;
      const prev = this.echo[this.echoIdx], next = this.echo[1 - this.echoIdx];
      const e = this.progs.echo;
      this.use(e, next, W, H);
      this.bind(0, this.srcTex, e.u.uSrc);
      this.bind(1, prev.tex, e.u.uPrev);
      gl.uniform1f(e.u.uAmt, this.echoWarm ? Math.pow(fx.echo, frames) : 0);
      gl.uniform1f(e.u.uZoom, 1 + fx.echoZoom * frames);
      gl.uniform1f(e.u.uRot, fx.echoTwist * frames);
      gl.uniform1f(e.u.uAspect, A);
      this.draw();
      this.echoIdx = 1 - this.echoIdx;
      this.echoWarm = true;
      base = next;
    } else this.echoWarm = false;

    // Blur pyramid and blur/glow mix, only when something uses them.
    const blurred = fx.blur > 0.001 || fx.glow > 0.001;
    let final = base;
    if (blurred) {
      let from = base;
      for (const lv of this.levels) {
        const d = this.progs.down;
        this.use(d, lv, lv.w, lv.h);
        this.bind(0, from.tex, d.u.uTex);
        gl.uniform2f(d.u.uTexel, 1 / from.w, 1 / from.h);
        this.draw();
        from = lv;
      }
      const c = this.progs.comp;
      this.use(c, this.comp, W, H);
      this.bind(0, base.tex, c.u.uBase);
      for (let i = 0; i < LEVELS; i++) this.bind(i + 1, this.levels[i].tex, c.u['uL' + (i + 1)]);
      for (let i = 1; i < LEVELS; i++) gl.uniform2f(c.u['uT' + (i + 1)], 1 / this.levels[i].w, 1 / this.levels[i].h);
      gl.uniform1f(c.u.uBlur, fx.blur);
      gl.uniform1f(c.u.uGlow, fx.glow);
      this.draw();
      final = this.comp;
    }

    const f = this.progs.final;
    this.use(f, null, ow, oh);
    this.bind(0, final.tex, f.u.uSrc);
    const u = f.u;
    gl.uniform1f(u.uAspect, A);
    gl.uniform1f(u.uKN, fx.kN);
    gl.uniform1f(u.uKLayout, fx.kLayout);
    gl.uniform1f(u.uKSpin, fx.kSpin);
    gl.uniform1f(u.uKZoom, fx.kZoom);
    gl.uniform2f(u.uKC, fx.kCx, fx.kCy);
    gl.uniform1f(u.uZoomOn, fx.zoomOn);
    gl.uniform1f(u.uZu, fx.zu);
    gl.uniform1f(u.uZRatio, fx.zRatio);
    gl.uniform1f(u.uZTwist, fx.zTwist);
    gl.uniform1f(u.uWarp, fx.warp);
    gl.uniform1f(u.uWarpScale, fx.warpScale);
    gl.uniform1f(u.uWarpT, fx.warpT);
    gl.uniform1f(u.uSat, fx.sat);
    gl.uniform1f(u.uHue, fx.hue);
    gl.uniform1f(u.uExposure, fx.exposure);
    gl.uniform1f(u.uContrast, fx.contrast);
    gl.uniform1f(u.uRgb, fx.rgb);
    gl.uniform1f(u.uVig, fx.vig);
    gl.uniform1f(u.uBlack, black);
    gl.uniform1f(u.uSeed, (this.seed = (this.seed + 7.13) % 1000));
    this.draw();
  }
}
