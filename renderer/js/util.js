export const TAU = Math.PI * 2;
export const clamp = (v, a, b) => (v < a ? a : v > b ? b : v);
export const lerp = (a, b, t) => a + (b - a) * t;
export const fract = (v) => v - Math.floor(v);
export const rad = (deg) => (deg * Math.PI) / 180;

/** Small seeded RNG so layers start in the same arrangement every launch. */
export function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export const uid = () => Math.random().toString(36).slice(2, 8);

export function hexToRgb(hex) {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function rgbToHsl(r, g, b) {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const l = (max + min) / 2;
  let h = 0, s = 0;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h *= 60;
  }
  return [h, s * 100, l * 100];
}

export function hslToRgb(h, s, l) {
  h = ((h % 360) + 360) % 360; s /= 100; l /= 100;
  const k = (n) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [f(0) * 255, f(8) * 255, f(4) * 255];
}

export const hsla = (h, s, l, a) =>
  `hsla(${(((h % 360) + 360) % 360).toFixed(1)},${s.toFixed(1)}%,${l.toFixed(1)}%,${a.toFixed(3)})`;

export const PALETTES = {
  neon: ['#ff2d95', '#7a5cff', '#00e5ff', '#39ff88', '#ffe14a'],
  sunset: ['#ff4e50', '#fc913a', '#f9d423', '#e8458b', '#7b2cbf'],
  ocean: ['#0077b6', '#00b4d8', '#90e0ef', '#48cae4', '#3a0ca3'],
  candy: ['#ff99c8', '#fcf6bd', '#d0f4de', '#a9def9', '#e4c1f9'],
  fire: ['#ff3d00', '#ff9100', '#ffc400', '#d50000', '#ff6e40'],
  aurora: ['#00f5a0', '#00d9f5', '#7b2ff7', '#f107a3', '#2af598'],
  rainbow: ['#ff0040', '#ff8a00', '#ffe600', '#00e676', '#00b0ff', '#7c4dff', '#ff00c8'],
  mono: ['#ffffff', '#cdd6ff', '#8f9bd1'],
};
export const PALETTE_OPTIONS = Object.keys(PALETTES).map((k) => [k, k[0].toUpperCase() + k.slice(1)]);

const hslCache = {};
export function paletteHsl(name) {
  return (hslCache[name] ||= (PALETTES[name] || PALETTES.neon).map((c) => rgbToHsl(...hexToRgb(c))));
}

const lutCache = {};
/** 256-entry RGB lookup that loops smoothly through the palette. */
export function paletteLut(name) {
  if (lutCache[name]) return lutCache[name];
  const stops = (PALETTES[name] || PALETTES.neon).map(hexToRgb);
  const lut = new Uint8ClampedArray(256 * 3);
  for (let i = 0; i < 256; i++) {
    const f = (i / 256) * stops.length;
    const a = stops[Math.floor(f) % stops.length];
    const b = stops[(Math.floor(f) + 1) % stops.length];
    const t = f - Math.floor(f);
    const e = t * t * (3 - 2 * t);
    for (let c = 0; c < 3; c++) lut[i * 3 + c] = a[c] + (b[c] - a[c]) * e;
  }
  return (lutCache[name] = lut);
}

export function makeCanvas(w, h) {
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  return c;
}
