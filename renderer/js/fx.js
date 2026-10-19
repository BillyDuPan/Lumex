import { rad, clamp } from './util.js';

const x2 = (v) => v.toFixed(2);
const pct = (v) => Math.round(v * 100) + '%';
const hund = (v) => (v * 100).toFixed(1);

/** Everything the FX tab exposes. `def` is the neutral value. Integer steps are not LFO targets. */
export const FX_GROUPS = [
  {
    title: 'Master',
    items: [
      { key: 'speed', label: 'Speed', min: 0, max: 3, step: 0.01, def: 1, fmt: (v) => (v === 0 ? 'Paused' : x2(v) + '×') },
    ],
  },
  {
    title: 'Infinite zoom',
    items: [
      { key: 'zoomSpeed', label: 'Flow', min: -0.6, max: 0.6, step: 0.005, def: 0, fmt: (v) => (v === 0 ? 'Off' : (v > 0 ? 'In ' : 'Out ') + Math.abs(v).toFixed(2)) },
      { key: 'zoomDepth', label: 'Depth', min: 0.4, max: 3, step: 0.01, def: 1, fmt: (v) => x2(v) + ' oct' },
      { key: 'zoomTwist', label: 'Spiral', min: -90, max: 90, step: 0.5, def: 0, unit: '°/oct' },
    ],
  },
  {
    title: 'Kaleidoscope',
    items: [
      { key: 'kN', label: 'Segments', min: 0, max: 24, step: 1, def: 0, fmt: (v) => (v === 0 ? 'Off' : v) },
      { key: 'kSpin', label: 'Segment spin', min: -90, max: 90, step: 0.5, def: 0, unit: '°/s', tip: 'Turns the picture inside each segment; the mirror lines stay put' },
      { key: 'kRot', label: 'Pattern angle', min: -180, max: 180, step: 1, def: 0, unit: '°', tip: 'Turns the mirror lines themselves' },
      { key: 'kZoom', label: 'Magnify', min: 0.4, max: 3, step: 0.01, def: 1, fmt: (v) => x2(v) + '×' },
      { key: 'kCx', label: 'Center X', min: -0.5, max: 0.5, step: 0.005, def: 0, fmt: x2 },
      { key: 'kCy', label: 'Center Y', min: -0.5, max: 0.5, step: 0.005, def: 0, fmt: x2 },
    ],
  },
  {
    title: 'Warp',
    items: [
      { key: 'warp', label: 'Liquid', min: 0, max: 0.08, step: 0.001, def: 0, fmt: (v) => (v * 1000).toFixed(0) },
      { key: 'warpScale', label: 'Ripple size', min: 0.5, max: 8, step: 0.05, def: 2.5, fmt: x2 },
      { key: 'warpSpeed', label: 'Ripple speed', min: 0, max: 2, step: 0.01, def: 0.5, fmt: x2 },
    ],
  },
  {
    title: 'Look',
    items: [
      { key: 'blur', label: 'Blur', min: 0, max: 1, step: 0.005, def: 0, fmt: pct },
      { key: 'glow', label: 'Glow', min: 0, max: 2, step: 0.01, def: 0, fmt: pct },
      { key: 'sat', label: 'Saturation', min: 0, max: 3, step: 0.01, def: 1, fmt: pct },
      { key: 'hue', label: 'Hue shift', min: -180, max: 180, step: 1, def: 0, unit: '°' },
      { key: 'hueSpin', label: 'Hue cycle', min: -90, max: 90, step: 0.5, def: 0, unit: '°/s' },
      { key: 'exposure', label: 'Brightness', min: 0.2, max: 2, step: 0.01, def: 1, fmt: pct },
      { key: 'contrast', label: 'Contrast', min: 0.5, max: 2, step: 0.01, def: 1, fmt: pct },
    ],
  },
  {
    title: 'Trails',
    items: [
      { key: 'echo', label: 'Echo trails', min: 0, max: 0.97, step: 0.005, def: 0, fmt: pct },
      { key: 'echoZoom', label: 'Echo zoom', min: -0.04, max: 0.04, step: 0.001, def: 0, fmt: hund },
      { key: 'echoTwist', label: 'Echo twist', min: -0.05, max: 0.05, step: 0.001, def: 0, fmt: hund },
    ],
  },
  {
    title: 'Post',
    items: [
      { key: 'rgb', label: 'RGB split', min: 0, max: 0.04, step: 0.0005, def: 0, fmt: (v) => (v * 1000).toFixed(0) },
      { key: 'vig', label: 'Vignette', min: 0, max: 1, step: 0.01, def: 0.25, fmt: pct },
    ],
  },
];

export const FX_ITEMS = FX_GROUPS.flatMap((g) => g.items);
export const FX_DEFAULTS = Object.fromEntries(FX_ITEMS.map((i) => [i.key, i.def]));
export const FX_RANGES = Object.fromEntries(FX_ITEMS.filter((i) => i.step < 1).map((i) => [i.key, [i.min, i.max]]));
const LIMITS = Object.fromEntries(FX_ITEMS.map((i) => [i.key, [i.min, i.max]]));

/**
 * Slider targets plus eased "current" values, so knob moves and scene changes glide
 * instead of jumping. Spin / hue-cycle / zoom accumulate with scene time (so Speed scales them).
 * `delta` holds the LFO offsets for this frame; every read goes through eff().
 */
export class FxState {
  constructor() {
    this.target = { ...FX_DEFAULTS };
    this.cur = { ...FX_DEFAULTS };
    this.mods = {};
    this.delta = {};
    this.spinAcc = 0;
    this.hueAcc = 0;
    this.zoomAcc = 0;
    this.warpT = 0;
    this.black = 0;
    this.blackTarget = 0;
    this.tau = 0.08;
  }

  set(values, glide = 0) {
    Object.assign(this.target, values);
    if (glide) this.tau = glide;
  }

  /** Eased value plus this frame's LFO offset, clamped to the slider range. */
  eff(key) {
    const d = this.delta[key];
    if (!d) return this.cur[key];
    const [lo, hi] = LIMITS[key];
    return clamp(this.cur[key] + d, lo, hi);
  }

  step(dt, simDt) {
    const k = 1 - Math.exp(-dt / this.tau);
    for (const key in this.target) {
      this.cur[key] = key === 'kN' ? this.target[key] : this.cur[key] + (this.target[key] - this.cur[key]) * k;
    }
    this.tau += (0.08 - this.tau) * (1 - Math.exp(-dt / 1.5));
    this.spinAcc += this.eff('kSpin') * simDt;
    this.hueAcc += this.eff('hueSpin') * simDt;
    this.warpT += this.eff('warpSpeed') * simDt;

    // Infinite zoom phase. When the flow stops, settle on a whole cycle so no ghost copy is left behind.
    const zs = this.eff('zoomSpeed');
    this.zoomAcc += zs * simDt;
    const idle = 1 - clamp(Math.abs(zs) / 0.01, 0, 1);
    if (idle > 0) this.zoomAcc += (Math.round(this.zoomAcc) - this.zoomAcc) * (1 - Math.exp(-simDt * 2 * idle));

    this.black += (this.blackTarget - this.black) * (1 - Math.exp(-dt / 0.12));
  }

  /** The payload both windows feed to the pipeline (angles in radians). */
  frame() {
    const e = (k) => this.eff(k);
    const zu = this.zoomAcc - Math.floor(this.zoomAcc);
    const depth = e('zoomDepth');
    const zoomOn = zu > 0.002 && zu < 0.998 ? 1 : 0;
    return {
      speed: e('speed'),
      kN: this.cur.kN, kLayout: rad(e('kRot')), kSpin: rad(this.spinAcc), kZoom: e('kZoom'), kCx: e('kCx'), kCy: e('kCy'),
      zoomOn, zu: zoomOn ? zu : 0, zRatio: Math.pow(2, depth), zTwist: rad(e('zoomTwist')) * depth,
      warp: e('warp'), warpScale: e('warpScale'), warpT: this.warpT,
      blur: e('blur'), glow: e('glow'), sat: e('sat'), hue: rad(e('hue') + this.hueAcc),
      exposure: e('exposure'), contrast: e('contrast'),
      echo: e('echo'), echoZoom: e('echoZoom'), echoTwist: e('echoTwist'),
      rgb: e('rgb'), vig: e('vig'), black: this.black,
    };
  }
}
