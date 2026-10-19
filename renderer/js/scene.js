import { createLayer } from './layers.js';
import { makeCanvas } from './util.js';

/** Composites all layers into one source canvas which the WebGL pipeline then processes. */
export class Scene {
  constructor(H, aspect) {
    this.layers = [];
    this.bg = '#05060a';
    this.W = 0; this.H = 0;
    this.canvas = makeCanvas(2, 2);
    this.ctx = this.canvas.getContext('2d', { alpha: false });
    this.ghost = makeCanvas(2, 2);
    this.ghostA = 0;
    this.ghostDur = 1;
    this.resize(H, aspect);
  }

  resize(H, aspect) {
    const W = Math.round(H * aspect);
    if (W === this.W && H === this.H) return;
    this.W = W; this.H = H;
    this.canvas.width = W; this.canvas.height = H;
    this.ghost.width = W; this.ghost.height = H;
    this.ghostA = 0;
    for (const l of this.layers) l.resize(W, H);
  }

  /** Reconcile layer instances with a config list (same ids keep their state). */
  setConfig(config) {
    this.bg = config.bg;
    const old = new Map(this.layers.map((l) => [l.id, l]));
    this.layers = config.layers.map((cfg) => {
      const l = old.get(cfg.id);
      if (l && l.type === cfg.type) { l.setConfig(cfg); return l; }
      return createLayer(cfg, this.W, this.H);
    });
  }

  get(id) { return this.layers.find((l) => l.id === id); }

  update(dt, t, input) {
    for (const l of this.layers) if (l.cfg.visible) l.update(dt, t, input);
  }

  /** LFO results per layer: { id: { p: {key: value}, opacity, blur } }. Missing layers use their plain config. */
  applyMods(map) {
    for (const l of this.layers) l.applyMod(map[l.id]);
  }

  snapshot() {
    const out = {};
    for (const l of this.layers) if (l.cfg.visible && l.state) out[l.id] = l.state;
    return out;
  }

  restore(snaps, extra) {
    for (const l of this.layers) {
      if (snaps[l.id]) l.state = snaps[l.id];
      l.extra = extra;
    }
  }

  /** Dissolve from the picture as it is right now into whatever gets composed next. */
  crossfade(seconds) {
    const g = this.ghost.getContext('2d');
    g.globalCompositeOperation = 'source-over';
    g.globalAlpha = 1;
    g.drawImage(this.canvas, 0, 0);
    this.ghostA = 1;
    this.ghostDur = Math.max(0.2, seconds);
  }

  compose(t, dt = 0) {
    const c = this.ctx, { W, H } = this;
    c.globalAlpha = 1;
    c.globalCompositeOperation = 'source-over';
    c.filter = 'none';
    c.fillStyle = this.bg;
    c.fillRect(0, 0, W, H);
    for (const l of this.layers) {
      const cfg = l.cfg;
      if (!cfg.visible || l.eo <= 0.002) continue;
      l.render(t);
      c.globalAlpha = l.eo;
      c.globalCompositeOperation = cfg.blend;
      c.filter = l.eb > 0.001 ? `blur(${(l.eb * 0.05 * H).toFixed(1)}px)` : 'none';
      c.drawImage(l.canvas, 0, 0);
    }
    c.globalCompositeOperation = 'source-over';
    c.filter = 'none';
    if (this.ghostA > 0.003) {
      const k = this.ghostA;
      c.globalAlpha = k * k * (3 - 2 * k);
      c.drawImage(this.ghost, 0, 0);
      this.ghostA = Math.max(0, this.ghostA - dt / this.ghostDur);
    }
    c.globalAlpha = 1;
  }
}

/**
 * Maps a point on the final picture back to the source canvas position that is shown there.
 * Mirrors sourceUv() in pipeline.js (warp is ignored: it only nudges things slightly).
 * Points are in world units: x in [0, aspect], y in [0, 1].
 * k = { n, layout, spin, zoom, cx, cy, zmul, twist } with angles in radians.
 */
export function foldPoint(x, y, aspect, k) {
  const px0 = (0.5 + k.cx) * aspect, py0 = 0.5 + k.cy;
  const px = (x - px0) / k.zoom, py = (y - py0) / k.zoom;
  const r = Math.hypot(px, py);
  let a = Math.atan2(py, px);
  if (k.n >= 0.5) {
    const seg = (Math.PI * 2) / k.n;
    a = (((a + k.layout) % seg) + seg) % seg;
    a = Math.abs(a - seg * 0.5);
  } else a += k.layout;
  a += k.spin + k.twist;
  const rr = r / k.zmul;
  return [px0 + Math.cos(a) * rr, py0 + Math.sin(a) * rr];
}
