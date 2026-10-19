import { makeLayerConfig } from './layers.js';
import { newEntry } from './mod.js';
import { PALETTES, uid } from './util.js';

const L = (type, p = {}, over = {}) => ({ type, ...over, p });
const draw = () => L('draw');
/** LFO shorthand: M(speed in Hz, intensity 0..1, shape) */
const M = (rate, depth, shape = 'sine') => ({ rate, depth, shape });
/** Music shorthand: A(source, intensity 0..1, smoothing 0..1) with source bass / mid / high / beat. */
const A = (src, depth, smooth = 0.35, inv = false) => ({ src, depth, smooth, inv });

/** Starting scenes. Effect values not listed fall back to the neutral defaults. */
export const BUILTIN_PRESETS = [
  {
    name: 'Lava lamp', palette: 'sunset',
    fx: { zoomSpeed: 0.04, warp: 0.012, warpScale: 2, blur: 0.08, glow: 0.5, hueSpin: 3, vig: 0.45 },
    mods: { glow: M(0.07, 0.3), warp: M(0.05, 0.5, 'drift'), exposure: A('bass', 0.1, 0.5), zoomSpeed: A('bass', 0.1, 0.55) },
    bg: '#12050a',
    layers: [
      L('gradient', { palette: 'sunset', mode: 'linear', angleSpeed: 4 }, { opacity: 0.32 }),
      L('balls', { palette: 'sunset', count: 11, size: 0.2, softness: 0.85, alpha: 0.72, speed: 0.45, mouse: 'repel' },
        { mods: { 'p.size': M(0.05, 0.25, 'drift'), 'p.speed': A('bass', 0.18, 0.5) } }),
      draw(),
    ],
  },
  {
    name: 'Neon mandala', palette: 'neon',
    fx: { kN: 8, kSpin: 4, kZoom: 0.9, zoomSpeed: 0.12, zoomDepth: 1.1, warp: 0.008, glow: 0.6, sat: 1.2, rgb: 0.006, vig: 0.5 },
    mods: { hue: M(0.04, 0.5, 'drift'), glow: M(0.1, 0.25), kZoom: A('bass', 0.08, 0.3), rgb: A('beat', 0.5, 0.3), exposure: A('beat', 0.08, 0.3), kSpin: A('mid', 0.06, 0.6) },
    bg: '#04030a',
    layers: [
      L('plasma', { palette: 'neon', scale: 0.8, speed: 0.35, dark: 0.85 }, { opacity: 0.7 }),
      L('balls', { palette: 'neon', count: 14, size: 0.085, softness: 0.55, alpha: 0.7, speed: 0.7, edges: 'bounce', mouse: 'swirl' }),
      draw(),
    ],
  },
  {
    name: 'Hypno', palette: 'rainbow',
    fx: { kN: 12, kSpin: -5, kZoom: 1, zoomSpeed: 0.2, zoomTwist: 28, warp: 0.03, warpScale: 3, hueSpin: 15, glow: 0.7, rgb: 0.01, sat: 1.35, vig: 0.5 },
    mods: {
      warp: M(0.06, 0.5, 'drift'), zoomTwist: M(0.04, 0.6, 'drift'), rgb: M(0.3, 0.4, 'pulse'),
      kZoom: A('bass', 0.07, 0.3), glow: A('bass', 0.25, 0.4), sat: A('high', 0.12, 0.45), exposure: A('beat', 0.1, 0.3),
    },
    bg: '#030208',
    layers: [
      L('plasma', { palette: 'rainbow', scale: 1.1, speed: 0.5, contrast: 1.5, dark: 0.9 }, { opacity: 0.55 }),
      L('balls', { palette: 'rainbow', count: 12, size: 0.07, softness: 0.6, alpha: 0.8, speed: 0.9, edges: 'bounce', mouse: 'stir' }),
      draw(),
    ],
  },
  {
    name: 'Tunnel', palette: 'aurora',
    fx: { kN: 4, kSpin: 6, zoomSpeed: 0.32, zoomTwist: 20, echo: 0.82, echoZoom: 0.01, glow: 0.6, hueSpin: 22, sat: 1.3 },
    mods: {
      zoomSpeed: M(0.04, 0.3, 'drift'), rgb: M(0.2, 0.5, 'pulse'),
      exposure: A('beat', 0.12, 0.3), glow: A('bass', 0.25, 0.4), hue: A('high', 0.1, 0.5), kZoom: A('bass', 0.06, 0.35),
    },
    bg: '#02040a',
    layers: [
      L('plasma', { palette: 'aurora', scale: 1.4, speed: 0.6, contrast: 1.6, dark: 0.9 }, { opacity: 0.45 }),
      L('balls', { palette: 'rainbow', count: 9, size: 0.05, softness: 0.5, alpha: 0.9, speed: 1.4, drift: 1.2, edges: 'bounce', mouse: 'swirl' }),
      L('sparks', { palette: 'rainbow', emit: 'move', rate: 140, life: 2.4 }),
      draw(),
    ],
  },
  {
    name: 'Deep ocean', palette: 'ocean',
    fx: { zoomSpeed: 0.07, warp: 0.025, warpScale: 1.6, blur: 0.18, glow: 0.8, echo: 0.5, echoZoom: 0.004, vig: 0.55 },
    mods: { blur: M(0.05, 0.3, 'drift'), warp: M(0.08, 0.4), glow: A('bass', 0.3, 0.6), zoomSpeed: A('bass', 0.08, 0.6), sat: A('mid', 0.15, 0.5) },
    bg: '#01050d',
    layers: [
      L('gradient', { palette: 'ocean', mode: 'radial', angleSpeed: 9 }, { opacity: 0.45 }),
      L('balls', { palette: 'ocean', count: 18, size: 0.14, softness: 0.9, alpha: 0.6, speed: 0.3, mouse: 'attract', mouseRadius: 0.35 }),
      L('sparks', { palette: 'ocean', emit: 'move', gravity: -0.12 }),
      draw(),
    ],
  },
  {
    name: 'Candy flower', palette: 'candy',
    fx: { kN: 6, kSpin: -3, kZoom: 0.9, zoomSpeed: 0.1, zoomTwist: 12, sat: 1.2, glow: 0.4, echo: 0.35, vig: 0.4 },
    mods: { sat: M(0.08, 0.2), zoomSpeed: M(0.03, 0.5), kZoom: A('bass', 0.08, 0.35), exposure: A('beat', 0.1, 0.3), hue: A('mid', 0.06, 0.55) },
    bg: '#0e0710',
    layers: [
      L('gradient', { palette: 'candy', mode: 'conic', angleSpeed: 12 }, { opacity: 0.3 }),
      L('balls', { palette: 'candy', count: 16, size: 0.09, softness: 0.4, alpha: 0.9, additive: false, speed: 0.8, edges: 'bounce', mouse: 'stir' }, { blend: 'source-over' }),
      draw(),
    ],
  },
  {
    name: 'Blank canvas', palette: 'mono',
    fx: { zoomSpeed: 0.04, warp: 0.01, glow: 0.8, echo: 0.8, echoZoom: 0.004, vig: 0.5 },
    mods: { glow: A('bass', 0.3, 0.45), exposure: A('beat', 0.08, 0.3), zoomSpeed: A('bass', 0.08, 0.55) },
    bg: '#030305',
    layers: [draw()],
  },
  {
    name: 'Bioluminescence', palette: 'aurora',
    fx: { zoomSpeed: 0.05, warp: 0.015, warpScale: 1.8, blur: 0.05, glow: 0.75, sat: 1.25, echo: 0.6, vig: 0.5 },
    mods: { glow: M(0.06, 0.35), warp: M(0.04, 0.4, 'drift'), exposure: A('beat', 0.12, 0.3), zoomSpeed: A('bass', 0.06, 0.5) },
    bg: '#02070c',
    layers: [
      L('gradient', { palette: 'ocean', mode: 'radial', angleSpeed: 5 }, { opacity: 0.4 }),
      L('life', { count: 70, lifeType: 'swimmer', palette: 'aurora', speed: 0.75, flock: 1.0, separation: 1.1, sense: 0.3, spores: 6, biolum: 1.2, mouse: 'hunt' }, { opacity: 0.95, blend: 'screen' }),
      L('sparks', { palette: 'aurora', emit: 'move', rate: 80, life: 1.8, gravity: -0.04 }),
      draw(),
    ],
  },
  {
    name: 'Sacred Vision', palette: 'neon',
    fx: { kN: 6, kSpin: 3, kZoom: 0.95, zoomSpeed: 0.08, zoomTwist: 16, glow: 0.65, sat: 1.3, echo: 0.45, vig: 0.45 },
    mods: { kZoom: A('bass', 0.08, 0.3), exposure: A('beat', 0.1, 0.3), glow: M(0.08, 0.25) },
    bg: '#05030c',
    layers: [
      L('plasma', { palette: 'neon', scale: 0.9, speed: 0.3, dark: 0.85 }, { opacity: 0.5 }),
      L('image', { src: 'builtin:mandala_flower', fit: 'fit', scale: 0.95, spin: 3, sat: 1.2, contrast: 1.1 }, { opacity: 0.85, blend: 'screen' }),
      draw(),
    ],
  },
];

const expandMods = (m) => Object.fromEntries(Object.entries(m || {}).map(([k, v]) => [k, newEntry(v)]));

/** Turn a preset (built-in or saved) into fresh layer configs. */
export function presetLayers(preset) {
  return preset.layers.map((l) => makeLayerConfig(l.type, { ...l, p: l.p, mods: expandMods(l.mods) }));
}

/** LFO setup of the effects, ready to assign to fx.mods. */
export const presetFxMods = (preset) => expandMods(preset.mods);

export function swatchOf(preset) {
  const key = preset.palette && PALETTES[preset.palette] ? preset.palette : 'neon';
  return PALETTES[key];
}

const plainMods = (store) => Object.fromEntries(Object.entries(store || {}).map(([k, e]) => {
  const { ph, ...rest } = e; // eslint-disable-line no-unused-vars
  return [k, rest];
}));

export function snapshotPreset(name, fx, fxMods, config, thumb) {
  return {
    id: uid(), name, user: true, thumb,
    fx: { ...fx }, mods: plainMods(fxMods), bg: config.bg,
    layers: config.layers.map((l) => ({
      type: l.type, name: l.name, visible: l.visible, opacity: l.opacity, blend: l.blend, blur: l.blur,
      p: { ...l.p }, mods: plainMods(l.mods),
    })),
    palette: (config.layers.find((l) => l.p.palette) || { p: {} }).p.palette,
  };
}
