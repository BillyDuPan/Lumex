import { TAU, clamp, fract } from './util.js';

/*
 * Modulation. A "store" is a plain object { key: entry } living on the fx state or on a layer config:
 *   entry = { on, src, shape, rate (Hz), depth (0..1 of the slider range), s (phase seed), smooth (0..1), inv }
 * `src` picks the source: an LFO that swings around the slider value, or one of the audio levels
 * (bass / mid / high / beat) that pushes the value up (or down with `inv`). The slider keeps its own value.
 * `ph` is runtime state only: the LFO phase, or the smoothed audio level.
 */

export const SHAPES = [['sine', 'Sine'], ['tri', 'Tri'], ['saw', 'Saw'], ['pulse', 'Pulse'], ['drift', 'Drift']];
export const SOURCES = [['lfo', 'LFO'], ['bass', 'Bass'], ['mid', 'Mid'], ['high', 'High'], ['beat', 'Beat']];

/** Live audio levels (0..1), written once per frame by audio.js. A typed array, so updating it never allocates. */
export const LEVEL = { bass: 0, mid: 1, high: 2, beat: 3, input: 4 };
export const levels = new Float64Array(5);

export const isAudio = (e) => !!e.src && e.src !== 'lfo';

/** Wave in [-1, 1]; `ph` counts cycles. */
export function wave(shape, ph, seed = 0) {
  switch (shape) {
    case 'tri': return 4 * Math.abs(fract(ph) - 0.5) - 1;
    case 'saw': return 2 * fract(ph) - 1;
    case 'pulse': return Math.tanh(5 * Math.sin(TAU * ph));
    case 'drift': {
      const a = TAU * ph;
      return (Math.sin(a + seed * 7) + 0.7 * Math.sin(a * 0.43 + seed * 13) + 0.5 * Math.sin(a * 0.17 + seed * 29)) / 2.2;
    }
    default: return Math.sin(TAU * ph);
  }
}

export function newEntry(over = {}) {
  return { on: true, src: 'lfo', shape: 'sine', rate: 0.15, depth: 0.3, s: Math.random(), smooth: 0.35, inv: false, ...over };
}

/** Global knobs: scale every LFO's speed and intensity, and every audio source's intensity, at once. */
export const master = { on: true, rate: 1, depth: 1, audio: 1 };

/** Attack / release time constants (s) for one smoothing amount; the release is the one you feel. */
export const releaseTau = (smooth) => 0.05 * Math.pow(30, smooth);
const ATTACK = 0.15;

/**
 * Advances all running modulators in `store` and returns this frame's offset per key (in value units).
 * `ranges` maps key -> [min, max]; keys without a range are ignored.
 */
export function stepStore(store, ranges, dt) {
  const out = {};
  for (const key in store) {
    const e = store[key], r = ranges[key];
    if (!e.on || !r) continue;
    if (isAudio(e)) {
      const target = levels[LEVEL[e.src]] || 0, cur = e.ph || 0;
      const tau = releaseTau(e.smooth ?? 0.35) * (target > cur ? ATTACK : 1);
      e.ph = cur + (target - cur) * (1 - Math.exp(-dt / tau));
      out[key] = (e.inv ? -e.ph : e.ph) * e.depth * master.audio * (r[1] - r[0]);
    } else {
      e.ph = (e.ph || 0) + dt * e.rate * master.rate;
      if (master.on) out[key] = wave(e.shape, e.ph + e.s, e.s) * e.depth * master.depth * (r[1] - r[0]) * 0.5;
    }
  }
  return out;
}

export const modulated = (value, delta, range) => clamp(value + (delta || 0), range[0], range[1]);

/** Offsets of the modulation's reach below and above the slider value, in value units (for the slider's range band). */
export function reach(e, range, into) {
  const span = range[1] - range[0];
  if (isAudio(e)) {
    const a = e.depth * master.audio * span;
    into[0] = e.inv ? -a : 0;
    into[1] = e.inv ? 0 : a;
  } else {
    const a = e.depth * master.depth * span * 0.5;
    into[0] = -a;
    into[1] = a;
  }
  return into;
}
