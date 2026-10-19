import {
  TAU, clamp, fract, rng, uid, hsla, makeCanvas,
  PALETTES, PALETTE_OPTIONS, paletteHsl, paletteLut,
} from './util.js';
import { getImage } from './media.js';

/*
 * A layer owns its own canvas. The control window runs update() to simulate,
 * packs the result into `state` (a typed array) and ships it to the output
 * window, which only restores `state` and calls render().
 */

export const BLENDS = [
  ['source-over', 'Normal'], ['lighter', 'Add'], ['screen', 'Screen'], ['overlay', 'Overlay'],
  ['multiply', 'Multiply'], ['color-dodge', 'Dodge'], ['difference', 'Difference'],
];

class Layer {
  constructor(cfg, W, H) {
    this.id = cfg.id;
    this.type = cfg.type;
    this.setConfig(cfg);
    this.canvas = makeCanvas(W, H);
    this.ctx = this.canvas.getContext('2d');
    this.W = W;
    this.H = H;
    this.state = null;
    this.extra = 0; // seconds to extrapolate remote state (output window only)
    this.onInit();
  }
  /** Params as the layer should use them this frame (plain config plus any LFO values). */
  get p() { return this.pe || this.cfg.p; }
  get A() { return this.W / this.H; }
  setConfig(cfg) {
    // Configs saved by older versions may lack newer params.
    cfg.p = { ...LAYER_TYPES[cfg.type].defaults, ...cfg.p };
    cfg.mods ||= {};
    this.cfg = cfg;
    this.applyMod(null);
  }
  applyMod(m) {
    this.pe = m && m.p ? { ...this.cfg.p, ...m.p } : null;
    this.eo = m && m.opacity !== undefined ? m.opacity : this.cfg.opacity;
    this.eb = m && m.blur !== undefined ? m.blur : this.cfg.blur;
  }
  resize(W, H) {
    this.W = W; this.H = H;
    this.canvas.width = W; this.canvas.height = H;
  }
  onInit() {}
  update() {}
  render() {}
}

/* ------------------------------------------------------------------ balls */

class BallsLayer extends Layer {
  onInit() {
    this.balls = [];
    this.rand = rng(11);
  }
  ensure(n) {
    const r = this.rand;
    while (this.balls.length < n) {
      this.balls.push({
        x: r() * this.A, y: r(), vx: 0, vy: 0, a: r() * TAU, ph: r() * TAU,
        s: 0.6 + r() * 0.8, rv: r(), ci: Math.floor(r() * 16), sw: 1, r: 0.1,
      });
    }
    this.balls.length = n;
  }
  update(dt, t, input) {
    const p = this.p, A = this.A;
    this.ensure(p.count | 0);
    const R = p.mouseRadius, F = p.mouseForce, mode = p.mouse;
    const kv = 1 - Math.exp(-dt * 0.6), ks = 1 - Math.exp(-dt * 6);
    const out = new Float32Array(this.balls.length * 6);
    let o = 0;
    for (const b of this.balls) {
      b.a += Math.sin(t * 0.37 + b.ph) * p.drift * 0.9 * dt;
      const cruise = p.speed * 0.09 * b.s;
      b.vx += (Math.cos(b.a) * cruise - b.vx) * kv;
      b.vy += (Math.sin(b.a) * cruise - b.vy) * kv;

      let near = 0;
      if (input.active && mode !== 'off') {
        const dx = b.x - input.x, dy = b.y - input.y;
        const d = Math.hypot(dx, dy) + 1e-5;
        if (d < R) {
          const q = 1 - d / R;
          near = q * q * (3 - 2 * q);
          const nx = dx / d, ny = dy / d;
          const press = input.down ? 1.6 : 1;
          if (mode === 'repel') {
            b.vx += nx * near * F * 5 * press * dt;
            b.vy += ny * near * F * 5 * press * dt;
          } else if (mode === 'attract') {
            b.vx -= nx * near * F * 4 * press * dt;
            b.vy -= ny * near * F * 4 * press * dt;
            const damp = Math.max(0, 1 - near * dt * 2.5);
            b.vx *= damp; b.vy *= damp;
          } else if (mode === 'swirl') {
            b.vx += (-ny * 4 - nx * 0.8) * near * F * press * dt;
            b.vy += (nx * 4 - ny * 0.8) * near * F * press * dt;
          } else if (mode === 'stir') {
            const k = Math.min(1, near * F * dt * 4);
            b.vx += (input.vx * 0.8 - b.vx) * k;
            b.vy += (input.vy * 0.8 - b.vy) * k;
          }
        }
      }
      const vmax = 2.2, sp = Math.hypot(b.vx, b.vy);
      if (sp > vmax) { b.vx *= vmax / sp; b.vy *= vmax / sp; }
      b.x += b.vx * dt;
      b.y += b.vy * dt;
      b.sw += (1 + p.swell * near - b.sw) * ks;
      b.r = p.size * (1 + p.sizeVar * (b.rv * 2 - 1)) * b.sw * (1 + p.pulse * Math.sin(t * 0.8 + b.ph * 2));

      if (p.edges === 'wrap') {
        const m = b.r * 1.1;
        if (b.x < -m) b.x = A + m; else if (b.x > A + m) b.x = -m;
        if (b.y < -m) b.y = 1 + m; else if (b.y > 1 + m) b.y = -m;
      } else {
        if (b.x < 0) { b.x = 0; b.vx = Math.abs(b.vx); } else if (b.x > A) { b.x = A; b.vx = -Math.abs(b.vx); }
        if (b.y < 0) { b.y = 0; b.vy = Math.abs(b.vy); } else if (b.y > 1) { b.y = 1; b.vy = -Math.abs(b.vy); }
      }
      out[o++] = b.x; out[o++] = b.y; out[o++] = b.vx; out[o++] = b.vy; out[o++] = b.r; out[o++] = b.ci;
    }
    this.state = out;
  }
  render(t) {
    const { ctx, W, H, p } = this;
    ctx.globalCompositeOperation = 'source-over';
    ctx.clearRect(0, 0, W, H);
    const s = this.state;
    if (!s) return;
    ctx.globalCompositeOperation = p.additive ? 'lighter' : 'source-over';
    const pal = paletteHsl(p.palette);
    const shift = p.hueCycle * t;
    const rin = (1 - p.softness) * 0.98;
    const ex = this.extra;
    for (let o = 0; o < s.length; o += 6) {
      const x = (s[o] + s[o + 2] * ex) * H, y = (s[o + 1] + s[o + 3] * ex) * H, r = s[o + 4] * H;
      const [h, sat, l] = pal[s[o + 5] % pal.length];
      const g = ctx.createRadialGradient(x, y, 0, x, y, r);
      g.addColorStop(0, hsla(h + shift, sat, l, p.alpha));
      g.addColorStop(rin, hsla(h + shift, sat, l, p.alpha));
      g.addColorStop(rin + (1 - rin) * 0.25, hsla(h + shift, sat, l, p.alpha * 0.84));
      g.addColorStop(rin + (1 - rin) * 0.5, hsla(h + shift, sat, l, p.alpha * 0.5));
      g.addColorStop(rin + (1 - rin) * 0.75, hsla(h + shift, sat, l, p.alpha * 0.156));
      g.addColorStop(1, hsla(h + shift, sat, l, 0));
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, TAU);
      ctx.fill();
    }
  }
}

/* ----------------------------------------------------------------- plasma */

class PlasmaLayer extends Layer {
  onInit() {
    this.phase = 0;
    this.small = makeCanvas(2, 2);
    this.sctx = this.small.getContext('2d');
    this.img = null;
  }
  update(dt) {
    this.phase += dt * this.p.speed;
    this.state = new Float32Array([this.phase]);
  }
  render() {
    const { ctx, W, H, p } = this;
    ctx.clearRect(0, 0, W, H);
    if (!this.state) return;
    const ph = this.state[0] + this.extra * p.speed;
    const h = 90, w = Math.round(h * this.A);
    if (!this.img || this.img.width !== w) {
      this.small.width = w; this.small.height = h;
      this.img = this.sctx.createImageData(w, h);
    }
    const lut = paletteLut(p.palette), d = this.img.data;
    const sc = p.scale * 3.2, A = this.A;
    const cx0 = Math.sin(ph * 0.6) * 1.8, cy0 = Math.cos(ph * 0.5) * 1.8;
    const scroll = ph * p.cycle * 0.1;
    let i = 0;
    for (let y = 0; y < h; y++) {
      const ny = (y / h - 0.5) * sc;
      for (let x = 0; x < w; x++) {
        const nx = (x / w - 0.5) * A * sc;
        let v = Math.sin(nx * 1.3 + ph) + Math.sin(ny * 1.7 - ph * 1.2) + Math.sin((nx + ny) * 1.1 + ph * 0.8);
        const cx = nx + cx0, cy = ny + cy0;
        v += Math.sin(Math.sqrt(cx * cx + cy * cy) * 2.1 - ph * 1.1);
        v = v * 0.125 * p.contrast + 0.5;
        const k = ((fract(v * p.bands + scroll) * 256) | 0) * 3;
        const u = clamp((v - 0.15) / 0.7, 0, 1);
        const shade = 1 - p.dark + p.dark * u * u * (3 - 2 * u);
        d[i++] = lut[k] * shade; d[i++] = lut[k + 1] * shade; d[i++] = lut[k + 2] * shade; d[i++] = 255;
      }
    }
    this.sctx.putImageData(this.img, 0, 0);
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = 'low'; // 'high' is an order of magnitude slower and the field is smooth anyway
    ctx.drawImage(this.small, 0, 0, W, H);
  }
}

/* --------------------------------------------------------------- gradient */

class GradientLayer extends Layer {
  onInit() { this.ang = 0; this.hue = 0; }
  update(dt) {
    this.ang += this.p.angleSpeed * dt;
    this.hue += this.p.hueCycle * dt;
    this.state = new Float32Array([this.ang, this.hue]);
  }
  render() {
    const { ctx, W, H, p } = this;
    ctx.clearRect(0, 0, W, H);
    if (!this.state) return;
    const ang = ((this.state[0] + this.extra * p.angleSpeed) * Math.PI) / 180, hue = this.state[1];
    const pal = paletteHsl(p.palette);
    const cx = W / 2, cy = H / 2, len = Math.hypot(W, H) / 2;
    const dx = Math.cos(ang), dy = Math.sin(ang);
    let g;
    if (p.mode === 'radial') g = ctx.createRadialGradient(cx + dx * H * 0.18, cy + dy * H * 0.18, 0, cx, cy, len);
    else if (p.mode === 'conic' && ctx.createConicGradient) g = ctx.createConicGradient(ang, cx, cy);
    else g = ctx.createLinearGradient(cx - dx * len, cy - dy * len, cx + dx * len, cy + dy * len);
    const n = pal.length;
    for (let i = 0; i <= n; i++) {
      const [h, s, l] = pal[i % n];
      g.addColorStop(i / n, hsla(h + hue, s, l, 1));
    }
    ctx.fillStyle = g;
    ctx.fillRect(0, 0, W, H);
  }
}

/* ----------------------------------------------------------------- sparks */

const SP = 8; // floats per particle: x y vx vy age life hue size

class SparksLayer extends Layer {
  onInit() {
    this.parts = [];
    this.carry = 0;
    this.sprites = new Map();
    this.hueT = 0;
  }
  update(dt, t, input) {
    const p = this.p;
    this.hueT += dt * p.hueCycle;
    const moving = Math.hypot(input.vx, input.vy) > 0.05;
    const emitting = p.emit === 'always' || (input.active && ((p.emit === 'move' && moving) || (p.emit === 'press' && input.down)));
    if (emitting) {
      this.carry += p.rate * dt;
      const pal = paletteHsl(p.palette);
      while (this.carry >= 1 && this.parts.length < 700) {
        this.carry -= 1;
        const a = Math.random() * TAU, sp = p.speed * (0.25 + Math.random() * 0.75);
        const c = pal[Math.floor(Math.random() * pal.length)];
        this.parts.push({
          x: input.x + (Math.random() - 0.5) * 0.01, y: input.y + (Math.random() - 0.5) * 0.01,
          vx: Math.cos(a) * sp * p.spread + input.vx * 0.25, vy: Math.sin(a) * sp * p.spread + input.vy * 0.25,
          age: 0, life: p.life * (0.6 + Math.random() * 0.8), hue: c[0], size: p.size * (0.5 + Math.random()),
        });
      }
    } else this.carry = 0;

    const drag = Math.exp(-dt * 1.2);
    for (const q of this.parts) {
      q.age += dt;
      q.vx *= drag;
      q.vy = q.vy * drag + p.gravity * dt;
      q.x += q.vx * dt;
      q.y += q.vy * dt;
    }
    this.parts = this.parts.filter((q) => q.age < q.life);
    const out = new Float32Array(this.parts.length * SP + 4);
    out[0] = input.active ? 1 : 0; out[1] = input.x; out[2] = input.y; out[3] = this.hueT;
    let o = 4;
    for (const q of this.parts) {
      out[o++] = q.x; out[o++] = q.y; out[o++] = q.vx; out[o++] = q.vy;
      out[o++] = q.age; out[o++] = q.life; out[o++] = q.hue; out[o++] = q.size;
    }
    this.state = out;
  }
  sprite(hue) {
    const key = Math.round(hue / 6) % 60;
    let c = this.sprites.get(key);
    if (!c) {
      c = makeCanvas(64, 64);
      const g = c.getContext('2d'), gr = g.createRadialGradient(32, 32, 0, 32, 32, 32);
      const h = key * 6;
      gr.addColorStop(0, `hsla(${h},100%,92%,1)`);
      gr.addColorStop(0.25, `hsla(${h},100%,65%,0.75)`);
      gr.addColorStop(0.6, `hsla(${h},100%,55%,0.2)`);
      gr.addColorStop(1, `hsla(${h},100%,50%,0)`);
      g.fillStyle = gr;
      g.fillRect(0, 0, 64, 64);
      this.sprites.set(key, c);
    }
    return c;
  }
  render() {
    const { ctx, W, H, p } = this;
    ctx.clearRect(0, 0, W, H);
    const s = this.state;
    if (!s) return;
    ctx.globalCompositeOperation = 'lighter';
    const ex = this.extra, shift = s[3];
    if (p.orb && s[0]) {
      const r = p.orbSize * H, x = s[1] * H, y = s[2] * H;
      ctx.globalAlpha = 0.9;
      ctx.drawImage(this.sprite(200 + shift), x - r, y - r, r * 2, r * 2);
    }
    ctx.globalAlpha = 1;
    for (let o = 4; o < s.length; o += SP) {
      const life = s[o + 5], k = 1 - (s[o + 4] + ex) / life;
      if (k <= 0) continue;
      const r = s[o + 7] * H * (0.35 + 0.65 * k);
      const x = (s[o] + s[o + 2] * ex) * H, y = (s[o + 1] + s[o + 3] * ex) * H;
      ctx.globalAlpha = Math.min(1, k * 1.6);
      ctx.drawImage(this.sprite(s[o + 6] + shift), x - r, y - r, r * 2, r * 2);
    }
    ctx.globalAlpha = 1;
  }
}

/* ------------------------------------------------------------------- draw */

const STRIDE = 4; // x y t hue

class DrawLayer extends Layer {
  onInit() {
    this.baked = makeCanvas(this.W, this.H);
    this.bctx = this.baked.getContext('2d');
    this.live = new Map();
    this.sprites = new Map();
  }
  resize(W, H) {
    const old = this.baked;
    super.resize(W, H);
    this.baked = makeCanvas(W, H);
    this.bctx = this.baked.getContext('2d');
    this.bctx.drawImage(old, 0, 0, W, H);
  }
  clear() {
    this.bctx.clearRect(0, 0, this.W, this.H);
    this.live.clear();
  }
  beginStroke(meta) {
    this.live.set(meta.id, { ...meta, pts: [], i0: 0, done: false });
  }
  addPoints(id, pts) {
    const s = this.live.get(id);
    if (!s) return;
    const from = Math.max(0, s.pts.length / STRIDE - 1);
    for (let i = 0; i < pts.length; i++) s.pts.push(pts[i]);
    if (s.erase) {
      const c = this.bctx, H = this.H;
      c.save();
      c.globalCompositeOperation = 'destination-out';
      c.lineCap = c.lineJoin = 'round';
      c.lineWidth = s.size * H;
      c.beginPath();
      const n = s.pts.length / STRIDE;
      c.moveTo(s.pts[from * STRIDE] * H, s.pts[from * STRIDE + 1] * H);
      for (let i = from; i < n; i++) c.lineTo(s.pts[i * STRIDE] * H, s.pts[i * STRIDE + 1] * H);
      if (n - from < 2) c.lineTo(s.pts[from * STRIDE] * H + 0.01, s.pts[from * STRIDE + 1] * H);
      c.stroke();
      c.restore();
    }
  }
  endStroke(id) {
    const s = this.live.get(id);
    if (!s) return;
    s.done = true;
    if (s.erase) this.live.delete(id);
    else if (s.life <= 0) {
      this.paint(this.bctx, s, 0);
      this.live.delete(id);
    }
  }
  sprite(color) {
    let c = this.sprites.get(color);
    if (!c) {
      c = makeCanvas(64, 64);
      const g = c.getContext('2d'), gr = g.createRadialGradient(32, 32, 0, 32, 32, 32);
      gr.addColorStop(0, color);
      gr.addColorStop(1, 'rgba(0,0,0,0)');
      g.fillStyle = gr;
      g.globalAlpha = 1;
      g.fillRect(0, 0, 64, 64);
      // Gradient to transparent black dims the colour, so recolour via source-in.
      g.globalCompositeOperation = 'source-in';
      g.fillStyle = color;
      g.fillRect(0, 0, 64, 64);
      this.sprites.set(color, c);
      if (this.sprites.size > 200) this.sprites.delete(this.sprites.keys().next().value);
    }
    return c;
  }
  render(t) {
    const { ctx, W, H } = this;
    ctx.globalCompositeOperation = 'source-over';
    ctx.clearRect(0, 0, W, H);
    ctx.drawImage(this.baked, 0, 0);
    for (const [id, s] of this.live) {
      if (s.erase) continue;
      if (s.life > 0) {
        const n = s.pts.length / STRIDE;
        while (s.i0 < n && t - s.pts[s.i0 * STRIDE + 2] >= s.life) s.i0++;
        if (s.i0 >= n && s.done) { this.live.delete(id); continue; }
        if (s.i0 > 256) { s.pts.splice(0, s.i0 * STRIDE); s.i0 = 0; }
      }
      this.paint(ctx, s, t);
    }
  }
  /** Draws one stroke. `t` only matters for vanishing strokes (life > 0). */
  paint(c, s, t) {
    const H = this.H, P = s.pts, n = P.length / STRIDE, i0 = s.i0 || 0;
    if (n - i0 < 1) return;
    const size = s.size * H, life = s.life;
    c.save();
    c.lineCap = c.lineJoin = 'round';
    c.globalCompositeOperation = 'source-over';
    const col = (i) => (s.rainbow ? `hsl(${P[i * STRIDE + 3]},100%,60%)` : s.color);
    const age = (i) => (life > 0 ? clamp(1 - (t - P[i * STRIDE + 2]) / life, 0, 1) : 1);

    if (s.style === 'soft') {
      const step = Math.max(1.5, size * 0.2);
      for (let i = i0; i < n; i++) {
        const k = age(i), x1 = P[i * STRIDE] * H, y1 = P[i * STRIDE + 1] * H;
        const prev = i > i0 ? i - 1 : i;
        const x0 = P[prev * STRIDE] * H, y0 = P[prev * STRIDE + 1] * H;
        const dist = Math.hypot(x1 - x0, y1 - y0), steps = Math.max(1, Math.ceil(dist / step));
        const r = size * (life > 0 ? Math.pow(k, 0.6) : 1);
        const sp = this.sprite(s.rainbow ? `hsl(${Math.round(P[i * STRIDE + 3] / 6) * 6},100%,60%)` : s.color);
        c.globalAlpha = (life > 0 ? Math.min(1, k * 2.5) : 1) * 0.55;
        for (let j = 1; j <= steps; j++) {
          const f = j / steps;
          c.drawImage(sp, x0 + (x1 - x0) * f - r, y0 + (y1 - y0) * f - r, r * 2, r * 2);
        }
      }
      c.restore();
      return;
    }

    // Split into runs that share one colour/width so alpha doesn't pile up at every joint.
    const hueGap = (a, b) => { const d = Math.abs(P[a * STRIDE + 3] - P[b * STRIDE + 3]); return Math.min(d, 360 - d); };
    const runs = [];
    let a = i0;
    for (let i = i0 + 1; i < n; i++) {
      if ((s.rainbow && hueGap(a, i) > 10) || (life > 0 && i - a >= 3)) { runs.push([a, i]); a = i; }
    }
    runs.push([a, n - 1]);

    const passes = s.style === 'neon'
      ? [[2.6, 0.28, null], [1, 1, null], [0.32, 1, '#fff']]
      : [[1, 1, null]];
    for (const [wMul, aMul, fixed] of passes) {
      for (const [r0, r1] of runs) {
        const k = age((r0 + r1) >> 1);
        if (k <= 0) continue;
        c.strokeStyle = fixed || col(r0);
        c.globalAlpha = aMul * (life > 0 ? Math.min(1, k * 3) : 1);
        c.lineWidth = Math.max(0.5, size * wMul * (life > 0 ? Math.pow(k, 0.6) : 1));
        c.beginPath();
        c.moveTo(P[r0 * STRIDE] * H, P[r0 * STRIDE + 1] * H);
        for (let i = r0 + 1; i <= r1; i++) c.lineTo(P[i * STRIDE] * H, P[i * STRIDE + 1] * H);
        if (r1 === r0) c.lineTo(P[r0 * STRIDE] * H + 0.01, P[r0 * STRIDE + 1] * H);
        c.stroke();
      }
    }
    c.restore();
  }
  async exportState() {
    const blob = await new Promise((res) => this.baked.toBlob(res, 'image/png'));
    const strokes = [...this.live.values()].map((s) => ({ ...s, pts: s.pts.slice() }));
    return { blob, strokes };
  }
  async importState({ blob, strokes }) {
    this.clear();
    if (blob) {
      const bmp = await createImageBitmap(blob);
      this.bctx.drawImage(bmp, 0, 0, this.W, this.H);
    }
    for (const s of strokes || []) this.live.set(s.id, s);
  }
}

/* ------------------------------------------------------------------ image */

const FRAMING_OPTIONS = [
  ['fit', 'Fit'], ['cover', 'Cover'], ['stretch', 'Stretch'], ['tile', 'Tile'], ['center', 'Center'],
];
const MIRROR_OPTIONS = [
  ['none', 'None'], ['mirrorH', 'Mirror X'], ['mirrorV', 'Mirror Y'], ['quad', 'Quad 4x'],
];

class ImageLayer extends Layer {
  onInit() {
    this.angle = 0;
    this.panX = 0;
    this.panY = 0;
    this.small = makeCanvas(2, 2);
    this.sctx = this.small.getContext('2d');
  }
  update(dt) {
    const p = this.p;
    this.angle += (p.spin || 0) * dt;
    this.panX += (p.driftX || 0) * dt * 0.1;
    this.panY += (p.driftY || 0) * dt * 0.1;
    this.state = new Float32Array([this.angle, this.panX, this.panY]);
  }
  render(t) {
    const { ctx, W, H, p } = this;
    ctx.clearRect(0, 0, W, H);
    const src = p.src || 'builtin:mandala_flower';
    const { img, ready } = getImage(src);
    if (!ready || !img.width || !img.height) return;

    const iw = img.width, ih = img.height;
    const ang = ((this.state ? this.state[0] : this.angle) + this.extra * (p.spin || 0) + (p.rot || 0)) * (Math.PI / 180);
    const px = (this.state ? this.state[1] : this.panX) + this.extra * (p.driftX || 0) * 0.1 + (p.panX || 0);
    const py = (this.state ? this.state[2] : this.panY) + this.extra * (p.driftY || 0) * 0.1 + (p.panY || 0);

    const filters = [];
    if (p.invert > 0.001) filters.push(`invert(${(p.invert * 100).toFixed(1)}%)`);
    if (p.sat !== 1) filters.push(`saturate(${(p.sat * 100).toFixed(1)}%)`);
    if (p.contrast !== 1) filters.push(`contrast(${(p.contrast * 100).toFixed(1)}%)`);
    if (p.brightness !== 1) filters.push(`brightness(${(p.brightness * 100).toFixed(1)}%)`);
    if (p.tint !== 0) filters.push(`hue-rotate(${p.tint.toFixed(1)}deg)`);
    if (p.threshold > 0.01) {
      const threshBoost = Math.max(10, (p.threshold * 50).toFixed(0));
      filters.push(`contrast(${threshBoost * 100}%)`);
    }

    const usePixelate = p.pixelate > 1.5;
    const targetCtx = usePixelate ? this.sctx : ctx;
    const tw = usePixelate ? Math.max(8, Math.round(W / p.pixelate)) : W;
    const th = usePixelate ? Math.max(8, Math.round(H / p.pixelate)) : H;

    if (usePixelate && (this.small.width !== tw || this.small.height !== th)) {
      this.small.width = tw;
      this.small.height = th;
    }
    if (usePixelate) targetCtx.clearRect(0, 0, tw, th);

    targetCtx.save();
    targetCtx.filter = filters.length ? filters.join(' ') : 'none';

    const mode = p.fit || 'fit';
    let dw, dh;
    if (mode === 'cover') {
      const s = Math.max(tw / iw, th / ih) * (p.scale || 1);
      dw = iw * s; dh = ih * s;
    } else if (mode === 'stretch') {
      dw = tw * (p.scale || 1); dh = th * (p.scale || 1);
    } else if (mode === 'center') {
      const s = p.scale || 1;
      dw = iw * s; dh = ih * s;
    } else if (mode === 'tile') {
      targetCtx.translate(tw / 2 + px * tw, th / 2 + py * th);
      targetCtx.rotate(ang);
      const pattern = targetCtx.createPattern(img, 'repeat');
      if (pattern) {
        targetCtx.fillStyle = pattern;
        targetCtx.fillRect(-tw * 2, -th * 2, tw * 4, th * 4);
      }
      targetCtx.restore();
      if (usePixelate) {
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(this.small, 0, 0, W, H);
      }
      return;
    } else {
      const s = Math.min(tw / iw, th / ih) * (p.scale || 1);
      dw = iw * s; dh = ih * s;
    }

    const cx = tw / 2 + px * tw;
    const cy = th / 2 + py * th;

    const drawTransformed = (ox, oy, sx = 1, sy = 1) => {
      targetCtx.save();
      targetCtx.translate(ox, oy);
      targetCtx.scale(sx, sy);
      targetCtx.rotate(ang);
      targetCtx.drawImage(img, -dw / 2, -dh / 2, dw, dh);
      targetCtx.restore();
    };

    const mirror = p.mirror || 'none';
    if (mirror === 'mirrorH') {
      drawTransformed(cx, cy, 1, 1);
      drawTransformed(tw - cx, cy, -1, 1);
    } else if (mirror === 'mirrorV') {
      drawTransformed(cx, cy, 1, 1);
      drawTransformed(cx, th - cy, 1, -1);
    } else if (mirror === 'quad') {
      drawTransformed(cx, cy, 1, 1);
      drawTransformed(tw - cx, cy, -1, 1);
      drawTransformed(cx, th - cy, 1, -1);
      drawTransformed(tw - cx, th - cy, -1, -1);
    } else {
      drawTransformed(cx, cy, 1, 1);
    }

    targetCtx.restore();

    if (usePixelate) {
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(this.small, 0, 0, W, H);
      ctx.imageSmoothingEnabled = true;
    }

    if (p.glitch > 0.02) {
      const g = p.glitch;
      const numSlices = Math.floor(2 + g * 8);
      for (let i = 0; i < numSlices; i++) {
        const sy = Math.floor(Math.sin(t * 13 + i * 27) * 0.5 + 0.5) * H;
        const sh = Math.floor((0.02 + 0.08 * Math.sin(t * 19 + i * 43) * 0.5 + 0.5) * H);
        const offset = Math.sin(t * 31 + i * 17) * g * W * 0.12;
        ctx.drawImage(this.canvas, 0, sy, W, sh, offset, sy, W, sh);
      }
    }
  }
}

/* ------------------------------------------------------------------- life */

const LIFE_MODES = [
  ['hunt', 'Curious swarm'], ['flee', 'Scatter'], ['orbit', 'Orbit'], ['off', 'Autonomous'],
];
const LIFE_TYPES = [
  ['swimmer', 'Bioluminescent fish'], ['microbe', 'Amoeba cells'], ['spore', 'Wispy spores'],
];

class LifeLayer extends Layer {
  onInit() {
    this.organisms = [];
    this.spores = [];
    this.rand = rng(42);
    this.initSpores(6);
  }

  initSpores(n) {
    const r = this.rand;
    while (this.spores.length < n) {
      this.spores.push({
        x: r() * this.A,
        y: r(),
        vx: (r() - 0.5) * 0.05,
        vy: (r() - 0.5) * 0.05,
        pulse: r() * TAU,
      });
    }
    this.spores.length = n;
  }

  ensure(n) {
    const r = this.rand;
    while (this.organisms.length < n) {
      const a = r() * TAU;
      this.organisms.push({
        x: r() * this.A,
        y: r(),
        vx: Math.cos(a) * 0.1,
        vy: Math.sin(a) * 0.1,
        a,
        ph: r() * TAU,
        light: 0.2 + r() * 0.4,
        size: 0.75 + r() * 0.5,
        ci: Math.floor(r() * 8),
      });
    }
    this.organisms.length = n;
  }

  update(dt, t, input) {
    const p = this.p, A = this.A;
    const n = Math.max(5, p.count | 0);
    this.ensure(n);
    const numSpores = Math.max(0, p.spores | 0);
    this.initSpores(numSpores);

    for (const s of this.spores) {
      s.x += s.vx * dt;
      s.y += s.vy * dt;
      s.pulse += dt * 3;
      if (s.x < 0) s.x += A; else if (s.x > A) s.x -= A;
      if (s.y < 0) s.y += 1; else if (s.y > 1) s.y -= 1;
    }

    const orgs = this.organisms;
    const sense = p.sense, sense2 = sense * sense;
    const sepDist = sense * 0.38, sepDist2 = sepDist * sepDist;
    const maxSpeed = p.speed * 0.28;
    const steerForce = 4.0;
    const mouseR = p.mouseRadius, mouseF = p.mouseForce, mouseM = p.mouse;

    for (let i = 0; i < n; i++) {
      const o = orgs[i];
      let sepX = 0, sepY = 0, sepN = 0;
      let aliX = 0, aliY = 0, aliN = 0;
      let cohX = 0, cohY = 0, cohN = 0;

      for (let j = 0; j < n; j++) {
        if (i === j) continue;
        const other = orgs[j];
        let dx = other.x - o.x;
        let dy = other.y - o.y;
        if (p.edges === 'wrap') {
          if (dx > A * 0.5) dx -= A; else if (dx < -A * 0.5) dx += A;
          if (dy > 0.5) dy -= 1; else if (dy < -0.5) dy += 1;
        }
        const d2 = dx * dx + dy * dy;
        if (d2 < sense2 && d2 > 1e-6) {
          const d = Math.sqrt(d2);
          if (d2 < sepDist2) {
            const f = (1 - d / sepDist) / d;
            sepX -= dx * f;
            sepY -= dy * f;
            sepN++;
          }
          aliX += other.vx;
          aliY += other.vy;
          aliN++;
          cohX += other.x;
          cohY += other.y;
          cohN++;
        }
      }

      let ax = 0, ay = 0;
      if (sepN > 0) {
        ax += (sepX / sepN) * p.separation * 3.5;
        ay += (sepY / sepN) * p.separation * 3.5;
      }
      if (aliN > 0) {
        const avgVx = aliX / aliN, avgVy = aliY / aliN;
        ax += (avgVx - o.vx) * p.flock * 1.5;
        ay += (avgVy - o.vy) * p.flock * 1.5;
      }
      if (cohN > 0) {
        const centerDistX = (cohX / cohN) - o.x;
        const centerDistY = (cohY / cohN) - o.y;
        ax += centerDistX * p.flock * 1.8;
        ay += centerDistY * p.flock * 1.8;
      }

      let closestSpore = null, closestDist2 = sense2 * 2;
      for (const s of this.spores) {
        let sdx = s.x - o.x, sdy = s.y - o.y;
        if (p.edges === 'wrap') {
          if (sdx > A * 0.5) sdx -= A; else if (sdx < -A * 0.5) sdx += A;
          if (sdy > 0.5) sdy -= 1; else if (sdy < -0.5) sdy += 1;
        }
        const sd2 = sdx * sdx + sdy * sdy;
        if (sd2 < closestDist2) {
          closestDist2 = sd2;
          closestSpore = { s, sdx, sdy, sd2 };
        }
      }

      if (closestSpore) {
        const dist = Math.sqrt(closestSpore.sd2);
        if (dist < 0.025) {
          o.light = 1.0;
          closestSpore.s.x = this.rand() * A;
          closestSpore.s.y = this.rand();
        } else {
          const foodPull = Math.min(2.5, 0.08 / (dist + 0.01));
          ax += (closestSpore.sdx / dist) * foodPull;
          ay += (closestSpore.sdy / dist) * foodPull;
        }
      }

      if (input.active && mouseM !== 'off') {
        let mdx = input.x - o.x, mdy = input.y - o.y;
        const md2 = mdx * mdx + mdy * mdy;
        if (md2 < mouseR * mouseR && md2 > 1e-6) {
          const md = Math.sqrt(md2);
          const nx = mdx / md, ny = mdy / md;
          const str = (1 - md / mouseR) * mouseF * 4;
          if (mouseM === 'hunt') {
            ax += nx * str * 2.5;
            ay += ny * str * 2.5;
            o.light = Math.max(o.light, str * 0.7);
          } else if (mouseM === 'flee') {
            ax -= nx * str * 3.5;
            ay -= ny * str * 3.5;
          } else if (mouseM === 'orbit') {
            ax += (-ny * 1.5 + nx * 0.4) * str;
            ay += (nx * 1.5 + ny * 0.4) * str;
          }
        }
      }

      const wander = Math.sin(t * 2.5 + o.ph) * 0.6;
      ax += -Math.sin(o.a) * wander;
      ay += Math.cos(o.a) * wander;

      o.vx += ax * dt * steerForce;
      o.vy += ay * dt * steerForce;

      const spd = Math.hypot(o.vx, o.vy);
      const targetSpd = maxSpeed * o.size;
      if (spd > 1e-5) {
        const smoothSpd = spd + (targetSpd - spd) * (1 - Math.exp(-dt * 3));
        o.vx = (o.vx / spd) * smoothSpd;
        o.vy = (o.vy / spd) * smoothSpd;
      }

      o.x += o.vx * dt;
      o.y += o.vy * dt;

      if (p.edges === 'bounce') {
        if (o.x < 0) { o.x = 0; o.vx = Math.abs(o.vx); }
        else if (o.x > A) { o.x = A; o.vx = -Math.abs(o.vx); }
        if (o.y < 0) { o.y = 0; o.vy = Math.abs(o.vy); }
        else if (o.y > 1) { o.y = 1; o.vy = -Math.abs(o.vy); }
      } else {
        if (o.x < 0) o.x += A; else if (o.x > A) o.x -= A;
        if (o.y < 0) o.y += 1; else if (o.y > 1) o.y -= 1;
      }

      o.a = Math.atan2(o.vy, o.vx);
      o.ph += dt * (3 + p.speed * 5);
      o.light = Math.max(0.15, o.light - dt * 0.8);
    }

    const out = new Float32Array(2 + n * 6 + numSpores * 3);
    out[0] = n;
    out[1] = numSpores;
    let idx = 2;
    for (let i = 0; i < n; i++) {
      const o = orgs[i];
      out[idx++] = o.x;
      out[idx++] = o.y;
      out[idx++] = o.a;
      out[idx++] = o.ph;
      out[idx++] = o.light;
      out[idx++] = o.size;
    }
    for (let i = 0; i < numSpores; i++) {
      const s = this.spores[i];
      out[idx++] = s.x;
      out[idx++] = s.y;
      out[idx++] = s.pulse;
    }
    this.state = out;
  }

  render() {
    const { ctx, W, H, p } = this;
    ctx.clearRect(0, 0, W, H);
    if (!this.state) return;

    const data = this.state;
    const n = data[0] | 0;
    const numSpores = data[1] | 0;
    const pal = paletteHsl(p.palette);
    const palLen = pal.length;
    const style = p.lifeType || 'swimmer';
    const biolum = p.biolum || 1.0;

    let spIdx = 2 + n * 6;
    for (let i = 0; i < numSpores; i++) {
      const sx = data[spIdx++] * H, sy = data[spIdx++] * H, pulse = data[spIdx++];
      const glowR = (0.012 + 0.005 * Math.sin(pulse)) * H;
      const g = ctx.createRadialGradient(sx, sy, 0, sx, sy, glowR * 3);
      g.addColorStop(0, 'rgba(255, 255, 255, 0.95)');
      g.addColorStop(0.35, 'rgba(0, 245, 160, 0.6)');
      g.addColorStop(1, 'rgba(0, 245, 160, 0)');
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(sx, sy, glowR * 3, 0, TAU);
      ctx.fill();

      ctx.fillStyle = '#ffffff';
      ctx.beginPath();
      ctx.arc(sx, sy, glowR * 0.7, 0, TAU);
      ctx.fill();
    }

    let orgIdx = 2;
    for (let i = 0; i < n; i++) {
      const ox = data[orgIdx++] * H;
      const oy = data[orgIdx++] * H;
      const a = data[orgIdx++];
      const ph = data[orgIdx++];
      const light = data[orgIdx++];
      const size = data[orgIdx++];

      const [hue, sat, baseL] = pal[i % palLen];
      const brightness = Math.min(100, baseL + light * 40 * biolum);
      const color = hsla(hue, sat, brightness, 0.9);
      const glowColor = hsla(hue, sat, brightness, 0.35 * biolum * light);

      const r = (0.016 * size) * H;

      ctx.save();
      ctx.translate(ox, oy);
      ctx.rotate(a);

      if (style === 'microbe') {
        const membraneG = ctx.createRadialGradient(0, 0, r * 0.2, 0, 0, r * 2.2);
        membraneG.addColorStop(0, color);
        membraneG.addColorStop(0.6, glowColor);
        membraneG.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.fillStyle = membraneG;

        ctx.beginPath();
        const pts = 8;
        for (let j = 0; j <= pts; j++) {
          const ang = (j / pts) * TAU;
          const wobble = Math.sin(ang * 4 + ph) * 0.22;
          const rad = r * (1.6 + wobble);
          const px = Math.cos(ang) * rad, py = Math.sin(ang) * rad;
          if (j === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
        }
        ctx.closePath();
        ctx.fill();

        ctx.fillStyle = '#ffffff';
        ctx.beginPath();
        ctx.arc(r * 0.2, 0, r * 0.45, 0, TAU);
        ctx.fill();

      } else if (style === 'spore') {
        const g = ctx.createRadialGradient(0, 0, 0, 0, 0, r * 2.5);
        g.addColorStop(0, '#ffffff');
        g.addColorStop(0.4, color);
        g.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.fillStyle = g;
        ctx.beginPath();
        ctx.arc(0, 0, r * 2.5, 0, TAU);
        ctx.fill();

        ctx.strokeStyle = color;
        ctx.lineWidth = Math.max(1, r * 0.2);
        for (let t = -1; t <= 1; t++) {
          ctx.beginPath();
          ctx.moveTo(0, t * r * 0.6);
          const wave = Math.sin(ph + t * 2) * r * 0.8;
          ctx.quadraticCurveTo(-r * 1.5, t * r * 0.8 + wave, -r * 3.5, t * r + wave * 1.5);
          ctx.stroke();
        }

      } else {
        const aura = ctx.createRadialGradient(0, 0, 0, 0, 0, r * 2.8);
        aura.addColorStop(0, glowColor);
        aura.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.fillStyle = aura;
        ctx.beginPath();
        ctx.arc(0, 0, r * 2.8, 0, TAU);
        ctx.fill();

        ctx.strokeStyle = color;
        ctx.lineWidth = Math.max(1.5, r * 0.45);
        ctx.lineCap = 'round';
        ctx.beginPath();
        ctx.moveTo(0, 0);
        const tailSegments = 4;
        let lastTx = 0, lastTy = 0;
        for (let s = 1; s <= tailSegments; s++) {
          const segDist = -s * r * 0.75;
          const segFlex = Math.sin(ph - s * 0.85) * (s * r * 0.35);
          ctx.lineTo(segDist, segFlex);
          lastTx = segDist; lastTy = segFlex;
        }
        ctx.stroke();

        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.moveTo(lastTx, lastTy);
        ctx.lineTo(lastTx - r * 0.6, lastTy - r * 0.5);
        ctx.lineTo(lastTx - r * 0.4, lastTy);
        ctx.lineTo(lastTx - r * 0.6, lastTy + r * 0.5);
        ctx.closePath();
        ctx.fill();

        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.ellipse(r * 0.4, 0, r * 0.95, r * 0.6, 0, 0, TAU);
        ctx.fill();

        ctx.fillStyle = '#ffffff';
        ctx.beginPath();
        ctx.arc(r * 0.6, -r * 0.15, r * 0.28, 0, TAU);
        ctx.fill();
      }

      ctx.restore();
    }
  }
}

/* --------------------------------------------------------------- registry */

const MOUSE_MODES = [['off', 'Off'], ['repel', 'Repel'], ['attract', 'Attract'], ['swirl', 'Swirl'], ['stir', 'Stir']];

export const LAYER_TYPES = {
  balls: {
    label: 'Floating balls', Class: BallsLayer, blend: 'lighter',
    defaults: {
      count: 14, size: 0.17, sizeVar: 0.5, softness: 0.75, alpha: 0.9, speed: 0.5, drift: 0.5, pulse: 0.15,
      palette: 'neon', hueCycle: 6, additive: true, edges: 'wrap',
      mouse: 'repel', mouseRadius: 0.28, mouseForce: 0.7, swell: 0.35,
    },
    schema: [
      { key: 'count', label: 'Count', min: 1, max: 80, step: 1 },
      { key: 'size', label: 'Size', min: 0.02, max: 0.5, step: 0.005, fmt: (v) => (v * 100).toFixed(0) + '%' },
      { key: 'sizeVar', label: 'Size variation', min: 0, max: 0.9, step: 0.01 },
      { key: 'softness', label: 'Softness', min: 0, max: 1, step: 0.01 },
      { key: 'alpha', label: 'Brightness', min: 0.05, max: 1, step: 0.01 },
      { key: 'speed', label: 'Float speed', min: 0, max: 3, step: 0.01 },
      { key: 'drift', label: 'Wander', min: 0, max: 3, step: 0.01 },
      { key: 'pulse', label: 'Pulse', min: 0, max: 0.6, step: 0.01 },
      { key: 'palette', label: 'Colors', type: 'select', options: PALETTE_OPTIONS },
      { key: 'hueCycle', label: 'Color drift', min: -60, max: 60, step: 0.5, unit: '°/s' },
      { key: 'additive', label: 'Glow where they overlap', type: 'toggle' },
      { key: 'edges', label: 'Edges', type: 'segmented', options: [['wrap', 'Wrap'], ['bounce', 'Bounce']] },
      { group: 'Mouse' },
      { key: 'mouse', label: 'Reaction', type: 'segmented', options: MOUSE_MODES },
      { key: 'mouseRadius', label: 'Reach', min: 0.05, max: 0.8, step: 0.01 },
      { key: 'mouseForce', label: 'Strength', min: 0, max: 2, step: 0.01 },
      { key: 'swell', label: 'Swell near mouse', min: 0, max: 1.5, step: 0.01 },
    ],
  },
  plasma: {
    label: 'Plasma', Class: PlasmaLayer, blend: 'source-over',
    defaults: { palette: 'aurora', scale: 1, speed: 0.4, bands: 1.4, contrast: 1.2, dark: 0.6, cycle: 1 },
    schema: [
      { key: 'palette', label: 'Colors', type: 'select', options: PALETTE_OPTIONS },
      { key: 'scale', label: 'Scale', min: 0.2, max: 3, step: 0.01 },
      { key: 'speed', label: 'Speed', min: 0, max: 2, step: 0.01 },
      { key: 'bands', label: 'Color bands', min: 0.5, max: 4, step: 0.01 },
      { key: 'contrast', label: 'Contrast', min: 0.4, max: 3, step: 0.01 },
      { key: 'dark', label: 'Dark areas', min: 0, max: 1, step: 0.01 },
      { key: 'cycle', label: 'Color scroll', min: -5, max: 5, step: 0.05 },
    ],
  },
  gradient: {
    label: 'Gradient', Class: GradientLayer, blend: 'source-over',
    defaults: { mode: 'linear', palette: 'sunset', angleSpeed: 6, hueCycle: 0 },
    schema: [
      { key: 'mode', label: 'Shape', type: 'segmented', options: [['linear', 'Linear'], ['radial', 'Radial'], ['conic', 'Conic']] },
      { key: 'palette', label: 'Colors', type: 'select', options: PALETTE_OPTIONS },
      { key: 'angleSpeed', label: 'Rotation', min: -120, max: 120, step: 0.5, unit: '°/s' },
      { key: 'hueCycle', label: 'Color drift', min: -60, max: 60, step: 0.5, unit: '°/s' },
    ],
  },
  sparks: {
    label: 'Mouse sparks', Class: SparksLayer, blend: 'lighter',
    defaults: {
      emit: 'move', rate: 110, life: 1.6, size: 0.016, speed: 0.22, spread: 1, gravity: -0.06,
      palette: 'rainbow', hueCycle: 40, orb: true, orbSize: 0.05,
    },
    schema: [
      { key: 'emit', label: 'Emit', type: 'segmented', options: [['move', 'On move'], ['press', 'On press'], ['always', 'Always']] },
      { key: 'rate', label: 'Amount', min: 5, max: 400, step: 1 },
      { key: 'life', label: 'Lifetime', min: 0.2, max: 6, step: 0.05, unit: 's' },
      { key: 'size', label: 'Size', min: 0.004, max: 0.06, step: 0.001, fmt: (v) => (v * 100).toFixed(1) + '%' },
      { key: 'speed', label: 'Burst speed', min: 0, max: 1, step: 0.01 },
      { key: 'gravity', label: 'Gravity', min: -0.4, max: 0.4, step: 0.005 },
      { key: 'palette', label: 'Colors', type: 'select', options: PALETTE_OPTIONS },
      { key: 'hueCycle', label: 'Color drift', min: -240, max: 240, step: 1, unit: '°/s' },
      { key: 'orb', label: 'Glowing pointer', type: 'toggle' },
      { key: 'orbSize', label: 'Pointer size', min: 0.01, max: 0.2, step: 0.005 },
    ],
  },
  draw: {
    label: 'Drawing', Class: DrawLayer, blend: 'source-over', defaults: {}, schema: [],
  },
  image: {
    label: 'Image', Class: ImageLayer, blend: 'source-over',
    defaults: {
      src: 'builtin:mandala_flower', fit: 'fit', scale: 1.0, rot: 0, spin: 0,
      panX: 0, panY: 0, driftX: 0, driftY: 0,
      invert: 0, tint: 0, sat: 1.0, contrast: 1.0, brightness: 1.0,
      pixelate: 1, threshold: 0, glitch: 0, mirror: 'none',
    },
    schema: [
      { key: 'fit', label: 'Framing', type: 'segmented', options: FRAMING_OPTIONS },
      { key: 'scale', label: 'Size / Zoom', min: 0.1, max: 3.5, step: 0.01, fmt: (v) => (v * 100).toFixed(0) + '%' },
      { key: 'spin', label: 'Spin speed', min: -120, max: 120, step: 0.5, unit: '°/s' },
      { key: 'rot', label: 'Angle', min: -180, max: 180, step: 1, unit: '°' },
      { key: 'panX', label: 'Pan X', min: -1, max: 1, step: 0.01 },
      { key: 'panY', label: 'Pan Y', min: -1, max: 1, step: 0.01 },
      { key: 'driftX', label: 'Drift X', min: -2, max: 2, step: 0.01 },
      { key: 'driftY', label: 'Drift Y', min: -2, max: 2, step: 0.01 },
      { group: 'Image Effects' },
      { key: 'mirror', label: 'Mirror', type: 'segmented', options: MIRROR_OPTIONS },
      { key: 'invert', label: 'Invert', min: 0, max: 1, step: 0.01, fmt: (v) => (v * 100).toFixed(0) + '%' },
      { key: 'tint', label: 'Color tint', min: -180, max: 180, step: 1, unit: '°' },
      { key: 'sat', label: 'Saturation', min: 0, max: 3, step: 0.01 },
      { key: 'contrast', label: 'Contrast', min: 0.2, max: 3, step: 0.01 },
      { key: 'brightness', label: 'Brightness', min: 0.2, max: 2.5, step: 0.01 },
      { key: 'pixelate', label: 'Pixelate / Mosaic', min: 1, max: 48, step: 1, unit: 'px' },
      { key: 'threshold', label: 'Graphic stencil', min: 0, max: 1, step: 0.01 },
      { key: 'glitch', label: 'Glitch slice', min: 0, max: 1, step: 0.01 },
    ],
  },
  life: {
    label: 'Intelligent Life', Class: LifeLayer, blend: 'screen',
    defaults: {
      count: 65, lifeType: 'swimmer', speed: 0.7, flock: 0.9, separation: 1.1, sense: 0.28,
      spores: 6, biolum: 1.0, palette: 'aurora', edges: 'wrap',
      mouse: 'hunt', mouseRadius: 0.35, mouseForce: 1.0,
    },
    schema: [
      { key: 'lifeType', label: 'Organism', type: 'segmented', options: LIFE_TYPES },
      { key: 'palette', label: 'Colors', type: 'select', options: PALETTE_OPTIONS },
      { key: 'count', label: 'Population', min: 15, max: 140, step: 1 },
      { key: 'speed', label: 'Swim speed', min: 0.1, max: 3, step: 0.01 },
      { key: 'flock', label: 'Schooling / Cohesion', min: 0, max: 2, step: 0.01 },
      { key: 'separation', label: 'Personal space', min: 0, max: 2, step: 0.01 },
      { key: 'sense', label: 'Sensory reach', min: 0.05, max: 0.6, step: 0.01 },
      { key: 'spores', label: 'Energy spores', min: 0, max: 12, step: 1 },
      { key: 'biolum', label: 'Bioluminescent glow', min: 0.1, max: 2, step: 0.01 },
      { key: 'edges', label: 'Boundary', type: 'segmented', options: [['wrap', 'Wrap'], ['bounce', 'Bounce']] },
      { group: 'Mouse Reaction' },
      { key: 'mouse', label: 'Behavior', type: 'segmented', options: LIFE_MODES },
      { key: 'mouseRadius', label: 'Reach', min: 0.05, max: 0.8, step: 0.01 },
      { key: 'mouseForce', label: 'Strength', min: 0, max: 2, step: 0.01 },
    ],
  },
};

export function makeLayerConfig(type, over = {}) {
  const def = LAYER_TYPES[type];
  return {
    id: uid(), type, name: def.label, visible: true, opacity: 1, blend: def.blend, blur: 0,
    ...over,
    p: { ...def.defaults, ...(over.p || {}) },
    mods: { ...(over.mods || {}) },
  };
}

/** Value range of every LFO-able control of a layer: 'p.<key>' for its own params, plus opacity and blur. */
export function layerRanges(type) {
  const out = { opacity: [0, 1], blur: [0, 1] };
  for (const it of LAYER_TYPES[type].schema) {
    if (it.key && !it.type && it.step < 1) out['p.' + it.key] = [it.min, it.max];
  }
  return out;
}

export function createLayer(cfg, W, H) {
  return new LAYER_TYPES[cfg.type].Class(cfg, W, H);
}
