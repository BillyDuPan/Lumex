import { levels, LEVEL } from './mod.js';
import { clamp } from './util.js';

/*
 * Audio input for the control window: Web Audio capture -> analyser -> three band levels and a beat pulse,
 * written into `levels` (mod.js) once per frame. All buffers are allocated up front; update() does no allocation.
 */

const FFT = 2048;
const BANDS = [[30, 150], [150, 2000], [2000, 12000]]; // Hz: bass, mid, high
const GATE = 0.003; // band amplitude below which we treat the band as silent instead of boosting it
const FIXED_PEAK = 0.12; // reference amplitude when auto level is off
const BEAT_GAP = 0.2; // s
const MAC_SETTINGS_HINT = 'System Settings → Privacy & Security → Microphone';

export const audioCfg = { on: false, device: 'default', gain: 1, auto: true };
export const audioState = {
  status: 'off', // off | starting | on | denied | error
  message: '', label: '', devices: [], suspended: false,
  clip: false, // post-gain input peaked at full scale recently (the level itself is levels[LEVEL.input])
};

import { bridge } from './bridge.js';
const spec = new Float32Array(FFT / 2);
const wav = new Float32Array(FFT);
const lo = new Int32Array(3);
const hi = new Int32Array(3);
const peak = new Float32Array(3).fill(GATE);

// Running numbers live in a typed array: assigning a double to a module-level `let` would allocate a heap number every frame.
const AVG = 0, PREV = 1, SINCE = 2, CLIP = 3;
const run64 = new Float64Array(4);

let ctx = null, gainNode = null, analyser = null, source = null, stream = null;
let run = 0;
let notify = () => {};

export const onAudioChange = (fn) => { notify = fn; };

function setStatus(status, message = '') {
  audioState.status = status;
  audioState.message = message;
  notify();
}

function build() {
  ctx = new AudioContext({ latencyHint: 'interactive' });
  gainNode = ctx.createGain();
  analyser = ctx.createAnalyser();
  analyser.fftSize = FFT;
  analyser.smoothingTimeConstant = 0.2;
  gainNode.connect(analyser); // not connected to the speakers on purpose: no feedback loop
  const binHz = ctx.sampleRate / FFT;
  BANDS.forEach(([a, b], i) => {
    lo[i] = Math.max(1, Math.round(a / binHz));
    hi[i] = Math.min(FFT / 2 - 1, Math.max(lo[i], Math.round(b / binHz)));
  });
  ctx.addEventListener('statechange', () => { audioState.suspended = ctx.state !== 'running'; notify(); });
  const wake = () => { if (ctx.state === 'suspended' && audioState.status === 'on') ctx.resume(); };
  addEventListener('pointerdown', wake);
  addEventListener('keydown', wake);
}

/** A looping two-second beat (kick, hats, chord stabs) that goes straight into the analyser, silently. */
function demoSource() {
  const sr = ctx.sampleRate, n = Math.round(sr * 2);
  const buf = ctx.createBuffer(1, n, sr);
  const d = buf.getChannelData(0);
  for (let beat = 0; beat < 4; beat++) {
    const t0 = Math.round(sr * beat * 0.5);
    for (let i = 0, ph = 0; i < sr * 0.4; i++) {
      const t = i / sr;
      ph += (2 * Math.PI * (48 + 110 * Math.exp(-t * 28))) / sr;
      d[t0 + i] += 0.85 * Math.sin(ph) * Math.exp(-t * 8);
    }
    let prev = 0;
    for (let i = 0, t1 = t0 + Math.round(sr * 0.25); i < sr * 0.08; i++) {
      const w = Math.random() * 2 - 1;
      d[t1 + i] += 0.28 * (w - prev) * Math.exp(-(i / sr) * 55);
      prev = w;
    }
  }
  for (const s of [0.5, 1.5]) {
    const t0 = Math.round(sr * s);
    for (let i = 0; i < sr * 0.35; i++) {
      const t = i / sr;
      d[t0 + i] += 0.1 * Math.exp(-t * 6) * (Math.sin(2 * Math.PI * 440 * t) + Math.sin(2 * Math.PI * 554.4 * t) + Math.sin(2 * Math.PI * 659.3 * t));
    }
  }
  const node = ctx.createBufferSource();
  node.buffer = buf;
  node.loop = true;
  node.start();
  return node;
}

async function micAllowed() {
  if (!bridge?.micStatus) return true;
  let st = await bridge.micStatus();
  if (st === 'not-determined' && (await bridge.askMic())) st = 'granted';
  return st === 'granted';
}

const DENIED = `The microphone is blocked. Allow it for ${bridge ? 'Electron' : 'this page'} in ${MAC_SETTINGS_HINT}, then switch Audio off and on again.`;

export async function refreshDevices() {
  try {
    const list = await navigator.mediaDevices.enumerateDevices();
    audioState.devices = list.filter((d) => d.kind === 'audioinput' && d.deviceId)
      .map((d, i) => ({ id: d.deviceId, label: d.label || `Input ${i + 1}` }));
  } catch { audioState.devices = []; }
  notify();
}

export async function startAudio() {
  stopAudio(true);
  const token = ++run;
  setStatus('starting');
  try {
    if (!ctx) build();
    ctx.resume().catch(() => {});
    if (audioCfg.device === 'demo') {
      source = demoSource();
      audioState.label = 'Demo beat';
    } else {
      if (!(await micAllowed())) { if (token === run) setStatus('denied', DENIED); return; }
      const audio = { echoCancellation: false, noiseSuppression: false, autoGainControl: false };
      if (audioCfg.device !== 'default') audio.deviceId = { exact: audioCfg.device };
      try {
        stream = await navigator.mediaDevices.getUserMedia({ audio });
      } catch (err) {
        if (err.name !== 'OverconstrainedError') throw err;
        audioCfg.device = 'default'; // the saved input is gone (unplugged or renamed)
        delete audio.deviceId;
        stream = await navigator.mediaDevices.getUserMedia({ audio });
      }
      if (token !== run) { for (const t of stream.getTracks()) t.stop(); stream = null; return; }
      const track = stream.getAudioTracks()[0];
      audioState.label = track.label || 'Input';
      track.addEventListener('ended', () => { if (token === run) { stopAudio(true); setStatus('error', 'The input was disconnected. Plug it back in and it will resume.'); } });
      source = ctx.createMediaStreamSource(stream);
    }
    if (token !== run) return;
    source.connect(gainNode);
    setGain(audioCfg.gain);
    audioState.suspended = ctx.state !== 'running';
    setStatus('on');
    refreshDevices();
  } catch (err) {
    if (token !== run) return;
    stopAudio(true);
    if (err.name === 'NotAllowedError' || err.name === 'SecurityError') setStatus('denied', DENIED);
    else if (err.name === 'NotFoundError') setStatus('error', 'No audio input found.');
    else if (err.name === 'NotReadableError') setStatus('error', 'The input is busy or unavailable. Another app may be using it.');
    else setStatus('error', err.message || String(err));
  }
}

/** Releases the input. With `keepStatus` the caller sets the status itself. */
export function stopAudio(keepStatus = false) {
  run++;
  if (stream) for (const t of stream.getTracks()) t.stop();
  stream = null;
  if (source) {
    try { source.stop?.(); } catch { /* already stopped */ }
    try { source.disconnect(); } catch { /* not connected */ }
  }
  source = null;
  levels.fill(0);
  peak.fill(GATE);
  run64.fill(0);
  run64[SINCE] = 9;
  audioState.clip = false;
  audioState.label = '';
  if (ctx && ctx.state === 'running') ctx.suspend().catch(() => {});
  if (!keepStatus) setStatus('off');
}

export function setGain(v) {
  audioCfg.gain = v;
  if (gainNode) gainNode.gain.setTargetAtTime(v, ctx.currentTime, 0.02);
}

export const micHelp = () => bridge?.openMicSettings?.();

/** Where harnesses and tests can feed their own signal in. */
export const audioInputNode = () => gainNode;
export const audioContext = () => ctx;

/** Reads the analyser and refreshes `levels`. Call once per frame. */
export function updateAudio(dt) {
  if (audioState.status !== 'on' || ctx.state !== 'running') return;
  analyser.getFloatTimeDomainData(wav);
  let sum = 0, top = 0;
  for (let i = 0; i < FFT; i++) {
    const v = wav[i], a = v < 0 ? -v : v;
    sum += v * v;
    if (a > top) top = a;
  }
  levels[LEVEL.input] = clamp((20 * Math.log10(Math.sqrt(sum / FFT) + 1e-5) + 60) / 60, 0, 1);
  run64[CLIP] = top >= 0.98 ? 0.6 : Math.max(0, run64[CLIP] - dt);
  audioState.clip = run64[CLIP] > 0;

  analyser.getFloatFrequencyData(spec);
  const decay = Math.exp(-dt / 5), rise = 1 - Math.exp(-dt / 0.08);
  let bass = 0;
  for (let b = 0; b < 3; b++) {
    let e = 0;
    for (let i = lo[b]; i <= hi[b]; i++) {
      const a = Math.exp(spec[i] * 0.11513); // dB -> amplitude
      e += a * a;
    }
    e = Math.sqrt(e);
    const p = peak[b];
    peak[b] = e > p ? p + (e - p) * rise : Math.max(GATE, p * decay);
    const n = e / (audioCfg.auto ? Math.max(GATE, peak[b]) : FIXED_PEAK);
    const v = n > 1 ? 1 : n;
    if (b === 0) bass = v;
    levels[b] = v;
  }

  const avg = run64[AVG] + (bass - run64[AVG]) * (1 - Math.exp(-dt / 0.5));
  run64[AVG] = avg;
  run64[SINCE] += dt;
  levels[LEVEL.beat] *= Math.exp(-dt / 0.12);
  if (run64[SINCE] > BEAT_GAP && bass > 0.3 && bass > run64[PREV] && bass > avg * 1.2 + 0.05) {
    levels[LEVEL.beat] = 1;
    run64[SINCE] = 0;
  }
  run64[PREV] = bass;
}
