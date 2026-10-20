import { Scene, foldPoint } from './scene.js';
import { Pipeline } from './pipeline.js';
import { openLink } from './link.js';
import { LAYER_TYPES, BLENDS, makeLayerConfig, layerRanges } from './layers.js';
import { FX_GROUPS, FX_DEFAULTS, FX_RANGES, FxState } from './fx.js';
import { BUILTIN_PRESETS, presetLayers, presetFxMods, swatchOf, snapshotPreset } from './presets.js';
import { h, slider, knob, toggle, select, segmented, button, section, updateMarker, ICONS, showToast } from './ui.js';
import { SHAPES, SOURCES, LEVEL, master, levels, newEntry, stepStore, reach, isAudio, releaseTau } from './mod.js';
import { audioCfg, audioState, onAudioChange, startAudio, stopAudio, setGain, updateAudio, refreshDevices, micHelp, audioInputNode, audioContext } from './audio.js';
import { clamp, uid } from './util.js';
import { getAllMedia, getMediaAsset, saveUserMedia, deleteUserMedia, loadUserMedia, getImage } from './media.js';

const $ = (id) => document.getElementById(id);
const bridge = window.lumen || null; // set by preload.js inside Electron
const link = openLink();

loadUserMedia();

const CTRL_H = 540; // internal height of the preview; the output window uses its own quality
const SAVE_KEY = 'lumen.state.v1';
const SWATCHES = ['#ff2d95', '#ff3b30', '#ff9500', '#ffe14a', '#a8ff3e', '#39ff88', '#00e5ff', '#2f7bff', '#7a5cff', '#e040fb', '#ffffff', '#9aa3c7'];
const DOT = { balls: '#ff2d95', plasma: '#39ff88', gradient: '#ffb347', sparks: '#00e5ff', draw: '#ffffff', image: '#e040fb', life: '#00ffcc' };

/* ------------------------------------------------------------------ state */

function load() {
  try { return JSON.parse(localStorage.getItem(SAVE_KEY)) || {}; } catch { return {}; }
}
const saved = load();
if (saved.ver !== 2) { // v2 changed the defaults for output resolution and dissolve time
  if (saved.settings) { delete saved.settings.outH; delete saved.settings.glide; }
}

const fx = new FxState();
const config = { bg: BUILTIN_PRESETS[0].bg, layers: [] };
const brush = { tool: 'draw', style: 'neon', color: '#ff2d95', rainbow: false, size: 14, vanishPos: 0, layerId: null, ...saved.brush };
const settings = {
  aspectMode: 'auto', aspect: 16 / 9, outH: 720, testPattern: false, display: null, glide: 2, view: 'final',
  auto: false, autoEvery: 45, autoMode: 'sequence', autoBeats: 32, autoPaused: false,
  sceneViewMode: 'grid', sceneFilter: 'all', ...saved.settings,
};
let userPresets = saved.presets || [];
let selectedId = null;
let activeSceneId = BUILTIN_PRESETS[0].id;
let sceneIdx = -1;
let autoT = 0;
let beatCount = 0;
let lastBeatDetectTime = 0;
let sceneSearchQuery = '';
// Central Beat Clock & Sequencer State
const clock = {
  bpm: saved.settings?.bpm || 124.0,
  taps: [],
  phase: 0,
  beatInBar: 0,
  loopLength: saved.settings?.loopLength || 32,
  loopBeat: 0,
  source: saved.settings?.clockSource || 'tap', // 'tap' | 'audio'
};

let aspect = settings.aspectMode === 'auto' ? 16 / 9 : settings.aspect;
Object.assign(master, saved.lfo);
Object.assign(audioCfg, saved.audio);

if (saved.config?.layers?.length) {
  config.bg = saved.config.bg;
  config.layers = saved.config.layers;
  if (saved.fx) { fx.set(saved.fx); Object.assign(fx.cur, fx.target); }
  fx.mods = saved.fxMods || {};
} else {
  const p = BUILTIN_PRESETS[0];
  config.layers = presetLayers(p);
  fx.set({ ...FX_DEFAULTS, ...p.fx });
  Object.assign(fx.cur, fx.target);
  fx.mods = presetFxMods(p);
}

const out = { connected: false, last: 0, w: 0, h: 0, fps: 0 };
const mouse = { x: 0.5 * aspect, y: 0.5, vx: 0, vy: 0, down: false, active: false };

const scene = new Scene(CTRL_H, aspect);
scene.setConfig(config);

const frameEl = $('frame');
const glCanvas = $('gl');
const cursorEl = $('cursor');
scene.canvas.classList.add('src');
frameEl.insertBefore(scene.canvas, cursorEl);
let pipe;
try {
  pipe = new Pipeline(glCanvas);
} catch (err) {
  document.body.innerHTML = `<p style="padding:40px;color:#ff8da0">Could not start WebGL2: ${err.message}</p>`;
  throw err;
}

let dirty = false;
let saveTimer = 0;
const touch = () => { dirty = true; scheduleSave(); };

function scheduleSave() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    try {
      const state = { ver: 2, fx: fx.target, fxMods: fx.mods, lfo: master, audio: audioCfg, config, brush, settings, presets: userPresets };
      localStorage.setItem(SAVE_KEY, JSON.stringify(state, (k, v) => (k === 'ph' ? undefined : v)));
    } catch { /* storage may be unavailable */ }
  }, 400);
}

/* ----------------------------------------------------------------- layers */

const drawLayers = () => config.layers.filter((l) => l.type === 'draw');

function ensureBrushLayer() {
  const draws = drawLayers();
  if (!draws.length) {
    const cfg = makeLayerConfig('draw', { name: 'Drawing' });
    config.layers.push(cfg);
    scene.setConfig(config);
    brush.layerId = cfg.id;
    sendConfig();
    renderLayers(); renderDock();
  } else if (!draws.some((l) => l.id === brush.layerId)) {
    brush.layerId = draws[draws.length - 1].id;
  }
}

function commitStructure() {
  scene.setConfig(config);
  ensureBrushLayer();
  sendConfig();
  dirty = false;
  renderLayers();
  renderDock();
  scheduleSave();
}

function addLayer(type) {
  const n = config.layers.filter((l) => l.type === type).length;
  const cfg = makeLayerConfig(type, n ? { name: `${LAYER_TYPES[type].label} ${n + 1}` } : {});
  config.layers.push(cfg);
  selectedId = cfg.id;
  if (type === 'draw') brush.layerId = cfg.id;
  commitStructure();
}

function removeLayer(id) {
  config.layers = config.layers.filter((l) => l.id !== id);
  if (selectedId === id) selectedId = null;
  commitStructure();
}

function moveLayer(id, dir) {
  const i = config.layers.findIndex((l) => l.id === id), j = i + dir;
  if (j < 0 || j >= config.layers.length) return;
  [config.layers[i], config.layers[j]] = [config.layers[j], config.layers[i]];
  commitStructure();
}

const allPresets = () => {
  const userMap = new Map(userPresets.map((p) => [p.name, p]));
  const builtins = BUILTIN_PRESETS.filter((bp) => !userMap.has(bp.name));
  return [...builtins, ...userPresets];
};

function applyPreset(p) {
  const glide = (p.glide != null && p.glide >= 0) ? p.glide : settings.glide;
  if (glide > 0.15) {
    link.send({ type: 'xfade', dur: glide });
    scene.crossfade(glide);
  }
  const draws = drawLayers();
  const layers = presetLayers(p).map((cfg) => {
    if (cfg.type !== 'draw' || !draws.length) return cfg;
    const keep = draws.shift();
    Object.assign(keep, { visible: cfg.visible, opacity: cfg.opacity, blend: cfg.blend, blur: cfg.blur, mods: cfg.mods });
    return keep;
  });
  config.layers = [...layers, ...draws];
  config.bg = p.bg || '#05060a';
  const vals = { ...FX_DEFAULTS, ...p.fx };
  if (!('speed' in p.fx)) delete vals.speed;
  fx.set(vals, Math.max(0.05, glide * 0.6));
  fx.mods = presetFxMods(p);
  activeSceneId = p.id;
  sceneIdx = allPresets().findIndex((x) => x.id === p.id || x.name === p.name);
  if (sceneIdx === -1) sceneIdx = 0;
  autoT = 0;
  selectedId = null;
  commitStructure();
  renderFx();
  renderScenes();
  updateTopSceneBadge();
}

/* ------------------------------------------------------------- output link */

function sendConfig() {
  link.send({ type: 'config', config, aspect, outH: settings.outH });
}

async function sendInit() {
  link.send({ type: 'init', config, aspect, outH: settings.outH });
  link.send({ type: 'testpattern', on: settings.testPattern });
  for (const l of scene.layers) {
    if (l.type === 'draw') {
      link.send({ type: 'layerState', layer: l.id, state: await l.exportState() });
    } else if (l.type === 'image' && l.p.src && l.p.src.startsWith('user:')) {
      const asset = getMediaAsset(l.p.src);
      if (asset) link.send({ type: 'imageAsset', id: asset.id, dataUrl: asset.dataUrl, name: asset.name });
    }
  }
}

function noteAlive(m) {
  out.last = performance.now();
  out.w = m.w; out.h = m.h; out.fps = m.fps || out.fps;
  if (!out.connected) {
    out.connected = true;
    renderTopActions(); renderOutput();
    if (m.type === 'alive') sendInit(); // output was already running (e.g. this window was reloaded)
  }
  if (settings.aspectMode === 'auto' && m.w && m.h) setAspect(m.w / m.h);
}

link.on((m) => {
  if (m.type === 'hello') { noteAlive(m); sendInit(); }
  else if (m.type === 'alive') noteAlive(m);
  else if (m.type === 'bye') { out.connected = false; renderTopActions(); renderOutput(); }
});

function setAspect(a) {
  if (Math.abs(a - aspect) < 0.002) return;
  aspect = a;
  scene.resize(CTRL_H, aspect);
  fitStage();
  sendConfig();
}

function aspectFromMode() {
  const m = settings.aspectMode;
  if (m === 'auto') return out.w && out.h ? out.w / out.h : aspect;
  const [a, b] = m.split(':').map(Number);
  return a / b;
}

async function openOutput() {
  if (bridge) {
    const sel = settings.display;
    await bridge.openOutput({ displayId: sel && sel !== 'window' ? Number(sel) : null });
  } else {
    const w = window.open('output.html', 'lumen-output', 'popup=yes,width=1280,height=720');
    if (!w) alert('The browser blocked the output window. Allow pop-ups for this page and try again.');
  }
}

function closeOutput() {
  if (bridge) bridge.closeOutput();
  link.send({ type: 'close' });
  out.connected = false;
  renderTopActions(); renderOutput();
}

const toggleOutput = () => (out.connected ? closeOutput() : openOutput());

/* ------------------------------------------------------------ preview size */

function fitStage() {
  const stage = $('stage');
  const aw = stage.clientWidth - 36, ah = stage.clientHeight - 36;
  let w = aw, hgt = aw / aspect;
  if (hgt > ah) { hgt = ah; w = ah * aspect; }
  w = Math.max(160, Math.floor(w)); hgt = Math.max(90, Math.floor(hgt));
  frameEl.style.width = w + 'px';
  frameEl.style.height = hgt + 'px';
  // The preview doesn't need Retina sharpness; keeping it near 540p leaves the GPU to the output window.
  const k = Math.min(window.devicePixelRatio || 1, 2, 960 / w);
  glCanvas.width = Math.round(w * k);
  glCanvas.height = Math.round(hgt * k);
  updateCursor();
}
new ResizeObserver(fitStage).observe($('stage'));

/* --------------------------------------------------------------- pointer */

function kaleido() {
  const f = fx.frame();
  const u = f.zu >= 0.5 ? f.zu - 1 : f.zu; // follow whichever zoom copy is more visible
  return {
    n: f.kN, layout: f.kLayout, spin: f.kSpin, zoom: f.kZoom, cx: f.kCx, cy: f.kCy,
    zmul: Math.pow(f.zRatio, u), twist: f.zTwist * u,
  };
}

function toSource(e) {
  const r = frameEl.getBoundingClientRect();
  const x = ((e.clientX - r.left) / r.width) * aspect, y = (e.clientY - r.top) / r.height;
  return settings.view === 'final' ? foldPoint(x, y, aspect, kaleido()) : [x, y];
}

let lastMove = null;
function trackMouse(e) {
  const [x, y] = toSource(e);
  if (lastMove) {
    const dt = Math.max(1 / 240, (e.timeStamp - lastMove) / 1000);
    mouse.vx += ((x - mouse.x) / dt - mouse.vx) * 0.4;
    mouse.vy += ((y - mouse.y) / dt - mouse.vy) * 0.4;
  }
  lastMove = e.timeStamp;
  mouse.x = x; mouse.y = y; mouse.active = true;
  return [x, y];
}

let stroke = null;
let strokeCount = 0;

function brushLife() {
  const v = brush.vanishPos;
  return v <= 0 ? 0 : 0.5 * Math.pow(60, (v - 1) / 99);
}

function beginStroke() {
  ensureBrushLayer();
  const layer = scene.get(brush.layerId);
  if (!layer) return;
  const erase = brush.tool === 'erase';
  const meta = {
    id: `${uid()}${strokeCount++}`, size: (brush.size * (erase ? 1.8 : 1)) / 1080,
    style: brush.style, color: brush.color, rainbow: brush.rainbow && !erase, life: erase ? 0 : brushLife(), erase,
  };
  layer.beginStroke(meta);
  stroke = { layerId: layer.id, layer, meta, dist: 0, last: null, pending: [], sent: false };
}

function addStrokePoint(x, y) {
  if (!stroke) return;
  if (stroke.last) {
    const d = Math.hypot(x - stroke.last[0], y - stroke.last[1]);
    if (d < 0.0012) return;
    stroke.dist += d;
  }
  const pt = [x, y, simT, (stroke.dist * 260 + simT * 40) % 360];
  stroke.last = [x, y];
  stroke.pending.push(...pt);
  stroke.layer.addPoints(stroke.meta.id, pt);
}

function flushStroke() {
  if (!stroke || !stroke.pending.length) return;
  link.send({
    type: 'stroke', layer: stroke.layerId, id: stroke.meta.id,
    meta: stroke.sent ? undefined : stroke.meta, pts: stroke.pending,
  });
  stroke.sent = true;
  stroke.pending = [];
}

function endStroke() {
  if (!stroke) return;
  flushStroke();
  stroke.layer.endStroke(stroke.meta.id);
  link.send({ type: 'end', layer: stroke.layerId, id: stroke.meta.id });
  stroke = null;
}

function updateCursor(e) {
  if (e) cursorEl._e = { x: e.clientX, y: e.clientY };
  const p = cursorEl._e;
  if (!p) return;
  const r = frameEl.getBoundingClientRect();
  const px = (brush.size * (brush.tool === 'erase' ? 1.8 : 1) / 1080) * r.height;
  const dia = brush.tool === 'interact' ? 10 : Math.max(8, px);
  cursorEl.style.width = cursorEl.style.height = dia + 'px';
  cursorEl.style.transform = `translate(${p.x - r.left - dia / 2}px, ${p.y - r.top - dia / 2}px)`;
  cursorEl.classList.toggle('erase', brush.tool === 'erase');
  cursorEl.classList.toggle('dot', brush.tool === 'interact');
}

frameEl.addEventListener('pointerenter', (e) => { cursorEl.classList.add('show'); trackMouse(e); updateCursor(e); });
frameEl.addEventListener('pointerleave', () => {
  if (stroke) return;
  cursorEl.classList.remove('show');
  mouse.active = false; lastMove = null;
});
frameEl.addEventListener('pointerdown', (e) => {
  if (e.button !== 0) return;
  try { frameEl.setPointerCapture(e.pointerId); } catch { /* synthetic events have no active pointer */ }
  mouse.down = true;
  const [x, y] = trackMouse(e);
  if (brush.tool !== 'interact') { beginStroke(); addStrokePoint(x, y); }
});
frameEl.addEventListener('pointermove', (e) => {
  updateCursor(e);
  const events = stroke && e.getCoalescedEvents ? e.getCoalescedEvents() : [e];
  for (const ev of events.length ? events : [e]) {
    const [x, y] = trackMouse(ev);
    addStrokePoint(x, y);
  }
});
const release = (e) => {
  mouse.down = false;
  endStroke();
  if (e.type === 'pointercancel' || !frameEl.matches(':hover')) { mouse.active = false; cursorEl.classList.remove('show'); }
};
frameEl.addEventListener('pointerup', release);
frameEl.addEventListener('pointercancel', release);
frameEl.addEventListener('contextmenu', (e) => e.preventDefault());

/* ------------------------------------------------------------- frame loop */

const RANGES = {};
const rangesOf = (type) => (RANGES[type] ||= layerRanges(type));

/** Runs every layer's LFOs; returns the effective values of the controls being modulated. */
function stepLayerMods(dt) {
  const out = {};
  for (const cfg of config.layers) {
    const ranges = rangesOf(cfg.type);
    const d = stepStore(cfg.mods, ranges, dt);
    let m = null;
    for (const key in d) {
      const [lo, hi] = ranges[key];
      m ||= { p: {} };
      if (key === 'opacity' || key === 'blur') m[key] = clamp(cfg[key] + d[key], lo, hi);
      else m.p[key.slice(2)] = clamp(cfg.p[key.slice(2)] + d[key], lo, hi);
    }
    if (m) out[cfg.id] = m;
  }
  return out;
}

const markers = { fx: [], layers: [] };
const meters = { fx: [], audio: [] };

let simT = 0;
let last = performance.now();
let fpsAvg = 60;
let thumbReq = null;

function frame(now) {
  requestAnimationFrame(frame);
  const dt = Math.min(0.05, (now - last) / 1000);
  last = now;
  if (dt <= 0) return;
  fpsAvg += (1 / dt - fpsAvg) * 0.05;

  updateAudio(dt);
  fx.delta = stepStore(fx.mods, FX_RANGES, dt);
  const lm = stepLayerMods(dt);
  scene.applyMods(lm);

  const sdt = dt * fx.eff('speed');
  simT += sdt;
  fx.step(dt, sdt);
  mouse.vx *= Math.exp(-dt * 5);
  mouse.vy *= Math.exp(-dt * 5);

  scene.update(sdt, simT, mouse);
  scene.compose(simT, dt);
  const f = fx.frame();
  pipe.render(scene.canvas, f, dt, 0);

  if (thumbReq) { thumbReq(makeThumb()); thumbReq = null; }

  flushStroke();
  if (dirty) { dirty = false; sendConfig(); }
  link.send({ type: 'frame', t: simT, fx: f, layers: scene.snapshot(), lm });

  for (const m of markers[activeTab] || []) updateMarker(m, reach);
  for (const m of meters[activeTab] || []) m.tick();
  tickAudioDot();
  tickAutoCycle(dt);
  updateMonitorHud();

  blackBadge.classList.toggle('show', fx.blackTarget > 0.5);
  if (out.connected && now - out.last > 3000) { out.connected = false; renderTopActions(); renderOutput(); }
}

const blackBadge = $('blackBadge');

function makeThumb() {
  const c = document.createElement('canvas');
  c.width = 224; c.height = Math.round(224 / aspect);
  c.getContext('2d').drawImage(glCanvas, 0, 0, c.width, c.height);
  return c.toDataURL('image/jpeg', 0.72);
}

/* --------------------------------------------------------------- top bar */

let viewSeg;
function updateStatus() {
  const status = $('status');
  status.classList.toggle('on', out.connected);
  status.lastElementChild.textContent = out.connected ? `Output live · ${out.w}×${out.h}` : 'Output closed';
}

function renderTopActions() {
  const root = $('topActions');
  root.replaceChildren();
  updateStatus();

  viewSeg = segmented({
    options: [['final', 'Final', 'What the beamer shows'], ['source', 'Source', 'Layers before the kaleidoscope']],
    value: settings.view,
    onChange: (v) => setView(v),
  });
  root.append(
    viewSeg.el,
    button(['Clear', h('kbd', {}, 'C')], clearCurrent, '', 'Clear the current drawing layer'),
    button(['Blackout', h('kbd', {}, 'B')], toggleBlackout, 'danger' + (fx.blackTarget > 0.5 ? ' on' : ''), 'Fade the output to black'),
    button([out.connected ? 'Close output' : 'Open output', h('kbd', {}, 'O')], toggleOutput, out.connected ? '' : 'primary'),
    button(h('span', { html: ICONS.help }), () => ($('help').hidden = false), 'icon ghost', 'Shortcuts'),
  );
}

function setView(v) {
  settings.view = v;
  frameEl.classList.toggle('view-source', v === 'source');
  viewSeg?.set(v);
  touch();
}

function toggleBlackout() {
  fx.blackTarget = fx.blackTarget > 0.5 ? 0 : 1;
  renderTopActions();
}

function clearCurrent(all) {
  const ids = all === true ? drawLayers().map((l) => l.id) : [brush.layerId];
  for (const id of ids) {
    scene.get(id)?.clear();
    link.send({ type: 'clear', layer: id });
  }
}

/* ------------------------------------------------------------------ dock */

let toolSeg, styleSeg, sizeSlider, vanishSlider, swatchEls;

function makeScrubControl(label, val, min, max, step, unit, onCommit, fmt) {
  const lbl = h('span', { class: 'tool-scrub-lbl' }, label);
  const out = h('div', { class: 'tool-scrub-val', title: 'Click to type · Drag up/down' }, fmt ? fmt(val) : `${val}${unit ? ' ' + unit : ''}`);
  let cur = val;

  let drag = null;
  out.addEventListener('pointerdown', (e) => {
    out.setPointerCapture(e.pointerId);
    drag = { y: e.clientY, v: cur };
    e.preventDefault();
  });
  out.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const dy = drag.y - e.clientY;
    const speed = e.shiftKey ? 0.2 : 1;
    let nv = drag.v + Math.round(dy * speed * step);
    nv = Math.max(min, Math.min(max, nv));
    cur = nv;
    out.textContent = fmt ? fmt(cur) : `${cur}${unit ? ' ' + unit : ''}`;
    onCommit(cur);
  });
  out.addEventListener('pointerup', () => { drag = null; });

  out.addEventListener('click', () => {
    if (out.querySelector('input')) return;
    const input = h('input', { type: 'number', min, max, step, value: cur, class: 'val-input', style: { width: '40px' } });
    out.textContent = '';
    out.append(input);
    input.focus(); input.select();
    const commit = () => {
      let nv = parseFloat(input.value);
      if (isNaN(nv)) nv = cur;
      cur = Math.max(min, Math.min(max, nv));
      out.textContent = fmt ? fmt(cur) : `${cur}${unit ? ' ' + unit : ''}`;
      onCommit(cur);
    };
    input.addEventListener('keydown', (ke) => { ke.stopPropagation(); if (ke.key === 'Enter') commit(); else if (ke.key === 'Escape') out.textContent = fmt ? fmt(cur) : `${cur}${unit ? ' ' + unit : ''}`; });
    input.addEventListener('blur', commit);
  });

  return h('div', { class: 'tool-scrub' }, lbl, out);
}

function renderDock() {
  const bar = $('leftToolbar') || $('dock');
  if (!bar) return;
  bar.replaceChildren();

  // Tool buttons
  const mkTool = (t, label, icon, kbd) => h('button', {
    class: `tool-btn${brush.tool === t ? ' on' : ''}`,
    title: `${label} (${kbd})`,
    onclick: () => setTool(t)
  }, h('span', { html: icon }), h('span', {}, label), h('kbd', {}, kbd));

  const toolGroup = h('div', { class: 'tool-group' },
    mkTool('draw', 'Draw', ICONS.pencil, 'D'),
    mkTool('erase', 'Erase', ICONS.trash, 'E'),
    mkTool('interact', 'Move', ICONS.screen, 'P')
  );

  // Brush style buttons
  const styleBtns = ['pen', 'neon', 'soft'].map(s => h('button', {
    class: `style-btn${brush.style === s ? ' on' : ''}`,
    title: s.charAt(0).toUpperCase() + s.slice(1),
    onclick: () => { brush.style = s; for (const b of styleGroup.children) b.classList.toggle('on', b.textContent.toLowerCase() === s); touch(); }
  }, s.charAt(0).toUpperCase() + s.slice(1)));
  const styleGroup = h('div', { class: 'tool-group' }, ...styleBtns);

  // Swatches & Custom Picker
  swatchEls = SWATCHES.map((c) => h('button', {
    class: 'sw', style: { background: c }, title: c,
    onclick: () => { brush.color = c; brush.rainbow = false; if (brush.tool === 'erase') setTool('draw'); paintSwatches(); touch(); },
  }));
  const rainbow = h('button', { class: 'sw rainbow', title: 'Rainbow', onclick: () => { brush.rainbow = !brush.rainbow; if (brush.tool === 'erase') setTool('draw'); paintSwatches(); touch(); } });
  const picker = h('input', { type: 'color', value: brush.color });
  const custom = h('label', { class: 'sw custom', title: 'Custom colour' }, picker, h('span'));
  picker.addEventListener('input', () => { brush.color = picker.value; brush.rainbow = false; paintSwatches(); touch(); });
  swatchEls.push(rainbow, custom);
  bar._rainbow = rainbow; bar._custom = custom;
  const swatchGroup = h('div', { class: 'tool-swatches' }, ...swatchEls);

  // Scrubbers
  const sizeScrub = makeScrubControl('Size', brush.size, 2, 140, 1, 'px', (v) => {
    brush.size = v; updateCursor(); touch();
  });
  const vanishScrub = makeScrubControl('Vanish', brush.vanishPos, 0, 100, 1, '', (v) => {
    brush.vanishPos = v; touch();
  }, (v) => (v === 0 ? 'Never' : (0.5 * Math.pow(60, (v - 1) / 99)).toFixed(1) + 's'));

  // Layer Selector
  const drawSel = select({
    options: drawLayers().map((l) => [l.id, l.name]), value: brush.layerId,
    onChange: (v) => { brush.layerId = v; touch(); renderLayers(); },
  });
  drawSel.sel.title = 'Target layer';

  const clearBtn = h('button', {
    class: 'tool-btn', style: { width: '44px', height: '26px', fontSize: '9px', marginTop: 'auto' },
    title: 'Clear layer (C)',
    onclick: () => clearCurrent(false)
  }, 'Clear', h('kbd', {}, 'C'));

  bar.append(
    toolGroup,
    h('div', { class: 'tool-divider' }),
    styleGroup,
    h('div', { class: 'tool-divider' }),
    swatchGroup,
    h('div', { class: 'tool-divider' }),
    sizeScrub,
    vanishScrub,
    h('div', { class: 'tool-divider' }),
    h('div', { style: { width: '100%' } }, drawSel.el),
    clearBtn
  );
  paintSwatches();
}

function paintSwatches() {
  const dock = $('leftToolbar') || $('dock');
  SWATCHES.forEach((c, i) => swatchEls[i].classList.toggle('on', !brush.rainbow && brush.color.toLowerCase() === c));
  dock._rainbow?.classList.toggle('on', brush.rainbow);
  const custom = dock._custom;
  if (custom) {
    const isPreset = SWATCHES.includes(brush.color.toLowerCase());
    custom.classList.toggle('on', !brush.rainbow && !isPreset);
    custom.style.setProperty('--c', brush.color);
    custom.querySelector('input').value = brush.color;
  }
}

function setTool(t) {
  brush.tool = t;
  toolSeg?.set(t);
  updateCursor();
  touch();
}

/* ------------------------------------------------------------------ tabs */

const TABS = [['fx', 'FX'], ['layers', 'Layers'], ['audio', 'Audio'], ['output', 'Output']];
const panels = {};
let activeTab = 'fx';

function buildTabs() {
  const nav = $('tabs');
  nav.replaceChildren(...TABS.map(([id, label]) =>
    h('button', { 'data-t': id, class: id === activeTab ? 'on' : '', onclick: () => showTab(id) }, label)));
  for (const [id] of TABS) panels[id] = h('div', { hidden: id !== activeTab });
  $('panel').replaceChildren(...Object.values(panels));
}

function showTab(id) {
  activeTab = id;
  for (const b of $('tabs').children) b.classList.toggle('on', b.dataset.t === id);
  for (const [k, el] of Object.entries(panels)) el.hidden = k !== id;
}

/* -------------------------------------------------------------------- FX */

let fxSliders = {};

const MOD_RATES = [0.02, 2];
const pickOne = (a) => a[Math.floor(Math.random() * a.length)];

/** Wiring that gives a slider its modulation button. `live` reads the value being produced right now. */
function modFor(store, key, bucket, live) {
  return {
    store, key, shapes: SHAPES, sources: SOURCES, isAudio, smoothFmt: (v) => releaseTau(v).toFixed(2) + ' s',
    rateRange: MOD_RATES, newEntry, onChange: touch, live,
    register: (m) => markers[bucket].push(m),
  };
}

function renderFx() {
  const root = panels.fx;
  root.replaceChildren();
  fxSliders = {};
  markers.fx.length = 0;
  meters.fx.length = 0;
  root.append(lfoCard());
  root.append(h('div', { class: 'btnrow' },
    button('Surprise me', surprise, 'primary'),
    button('Reset effects', () => { fx.set({ ...FX_DEFAULTS, speed: fx.target.speed }, 0.5); syncFxSliders(); touch(); }),
  ));
  for (const g of FX_GROUPS) {
    const sec = section(g.title);
    for (const it of g.items) {
      const s = slider({
        label: it.label, min: it.min, max: it.max, step: it.step, value: fx.target[it.key], def: it.def, fmt: it.fmt, unit: it.unit, tip: it.tip,
        onInput: (v) => { fx.set({ [it.key]: v }); touch(); },
        mod: it.step < 1 ? modFor(fx.mods, it.key, 'fx', () => fx.eff(it.key)) : undefined,
      });
      fxSliders[it.key] = s;
      sec.append(s.el);
    }
    root.append(sec);
  }
}

function syncFxSliders() {
  for (const k in fxSliders) fxSliders[k].set(fx.target[k]);
}

/** Master LFO controls: one speed and one intensity knob that scale every LFO at once. */
function lfoCard() {
  const card = h('div', { class: 'lfo-card' + (master.on ? ' on' : '') });
  const box = h('input', { type: 'checkbox' });
  box.checked = master.on;
  box.addEventListener('change', () => { master.on = box.checked; card.classList.toggle('on', master.on); touch(); });
  const sw = h('label', { class: 'switch-only', title: 'All LFOs on / off (L)' }, box, h('span', { class: 'switch' }));
  const audioKnob = knob({
    label: 'Audio', min: 0, max: 2, value: master.audio, def: 1, fmt: (v) => Math.round(v * 100) + '%',
    onInput: (v) => { master.audio = v; touch(); },
  }).el;
  audioKnob.classList.add('aud');
  audioKnob.title = 'How hard the music pushes everything that follows it · double-click to reset';
  const miniMeter = meter(false);
  meters.fx.push(miniMeter);
  card.append(
    h('div', { class: 'lfo-head' }, h('h3', {}, 'LFO'),
      button('Randomize', () => randomizeMods(), 'small', 'Give a handful of effects their own LFO (and music reactions while audio is on)'),
      button('Clear', () => { fx.mods = {}; renderFx(); touch(); }, 'small ghost', 'Switch every effect LFO and music reaction off'),
      sw),
    h('div', { class: 'lfo-body' },
      knob({
        label: 'Speed', min: 0.1, max: 4, log: true, value: master.rate, def: 1, fmt: (v) => v.toFixed(2) + '×',
        onInput: (v) => { master.rate = v; touch(); },
      }).el,
      knob({
        label: 'Intensity', min: 0, max: 2, value: master.depth, def: 1, fmt: (v) => Math.round(v * 100) + '%',
        onInput: (v) => { master.depth = v; touch(); },
      }).el,
      audioKnob,
      miniMeter.el,
      h('p', { class: 'lfo-note' }, 'Speed and Intensity scale every LFO; Audio scales everything that follows the music. Hit the wave button next to any slider to modulate it.')),
  );
  return card;
}

/** How far each effect may swing when randomised, and a starting value if it sits at its neutral position. */
const LFO_POOL = {
  kZoom: { depth: 0.3 }, kCx: { depth: 0.12 }, kCy: { depth: 0.12 }, blur: { depth: 0.35, base: 0.15 }, glow: { depth: 0.5, base: 0.6 },
  sat: { depth: 0.3 }, hue: { depth: 0.5 }, exposure: { depth: 0.1 }, contrast: { depth: 0.12 }, warp: { depth: 0.6, base: 0.03 },
  warpScale: { depth: 0.35 }, zoomSpeed: { depth: 0.45, base: 0.12 }, zoomTwist: { depth: 0.5, base: 18 }, rgb: { depth: 0.6, base: 0.012 },
  echo: { depth: 0.15 }, vig: { depth: 0.3 },
};

/** Same idea for the music: which sources suit an effect, how hard it may be pushed, and a value to start from. */
const AUDIO_POOL = {
  kZoom: { src: ['bass', 'beat'], depth: 0.09 }, exposure: { src: ['beat', 'bass'], depth: 0.1 }, glow: { src: ['bass', 'mid'], depth: 0.3, base: 0.5 },
  warp: { src: ['bass', 'mid'], depth: 0.35, base: 0.02 }, zoomSpeed: { src: ['bass'], depth: 0.1, base: 0.1 }, hue: { src: ['mid', 'high', 'beat'], depth: 0.1 },
  sat: { src: ['mid', 'high'], depth: 0.15 }, rgb: { src: ['beat', 'high'], depth: 0.5 }, kSpin: { src: ['mid', 'bass'], depth: 0.08, base: 4 },
  zoomTwist: { src: ['mid', 'beat'], depth: 0.09, base: 10 }, echoTwist: { src: ['high'], depth: 0.25 }, contrast: { src: ['beat'], depth: 0.15 },
  vig: { src: ['bass'], depth: 0.3, inv: true },
};

const audioMod = (spec) => newEntry({ src: pickOne(spec.src), depth: spec.depth * (0.6 + Math.random() * 0.4), smooth: 0.2 + Math.random() * 0.4, inv: !!spec.inv });

function lfoMod(spec) {
  return newEntry({
    shape: pickOne(['sine', 'sine', 'tri', 'drift', 'drift', 'pulse']), rate: 0.03 * Math.pow(12, Math.random()),
    depth: spec.depth * (0.5 + Math.random() * 0.5),
  });
}

/** Hands a handful of effects a modulator: LFOs, plus music reactions when audio is listening. */
function randomizeMods(withAudio = audioState.status === 'on') {
  fx.mods = {};
  const pool = withAudio ? [...new Set([...Object.keys(LFO_POOL), ...Object.keys(AUDIO_POOL)])] : Object.keys(LFO_POOL);
  const keys = pool.sort(() => Math.random() - 0.5).slice(0, 5 + Math.floor(Math.random() * 3));
  const base = {};
  for (const k of keys) {
    const a = AUDIO_POOL[k], l = LFO_POOL[k];
    const spec = withAudio && a && (!l || Math.random() < 0.6) ? a : l;
    fx.mods[k] = spec === a ? audioMod(a) : lfoMod(l);
    if (spec.base !== undefined && fx.target[k] === FX_DEFAULTS[k]) base[k] = spec.base;
  }
  if (withAudio) { // make sure the music gets a say
    const have = () => Object.values(fx.mods).filter(isAudio).length;
    for (const k of Object.keys(AUDIO_POOL).sort(() => Math.random() - 0.5)) {
      if (have() >= 2) break;
      if (fx.mods[k] && isAudio(fx.mods[k])) continue;
      fx.mods[k] = audioMod(AUDIO_POOL[k]);
      if (AUDIO_POOL[k].base !== undefined && fx.target[k] === FX_DEFAULTS[k]) base[k] = AUDIO_POOL[k].base;
    }
  }
  fx.set(base, 0.6);
  master.on = true;
  renderFx();

  touch();
}

/** Music reactions only: leaves the effects that already have an LFO alone where it can. */
function randomizeReactions() {
  for (const k in fx.mods) if (isAudio(fx.mods[k])) delete fx.mods[k];
  const keys = Object.keys(AUDIO_POOL).sort(() => Math.random() - 0.5);
  const free = keys.filter((k) => !fx.mods[k]);
  const chosen = (free.length >= 5 ? free : keys).slice(0, 5 + Math.floor(Math.random() * 2));
  const base = {};
  for (const k of chosen) {
    fx.mods[k] = audioMod(AUDIO_POOL[k]);
    if (AUDIO_POOL[k].base !== undefined && fx.target[k] === FX_DEFAULTS[k]) base[k] = AUDIO_POOL[k].base;
  }
  fx.set(base, 0.6);
  renderFx();

  touch();
}

function surprise() {
  const pick = pickOne;
  const r = (a, b) => a + Math.random() * (b - a);
  fx.set({
    kN: pick([0, 0, 3, 4, 5, 6, 6, 8, 10, 12]), kSpin: pick([0, 0, r(-12, 12)]), kZoom: r(0.8, 1.4), kRot: r(-180, 180),
    zoomSpeed: pick([0, 0.08, 0.15, 0.25, -0.1]), zoomTwist: pick([0, 0, r(-25, 25)]), warp: pick([0, 0.01, 0.025, 0.04]),
    glow: r(0, 1.1), sat: r(0.9, 1.6), hueSpin: pick([0, r(-30, 30)]), blur: pick([0, 0, r(0, 0.3)]),
    echo: pick([0, 0.3, 0.6, 0.85]), echoZoom: pick([0, r(-0.015, 0.02)]), echoTwist: pick([0, r(-0.02, 0.02)]), rgb: pick([0, 0, r(0, 0.012)]),
  }, 0.7);
  syncFxSliders();
  touch();
}

/* ---------------------------------------------------------------- layers */

function controlFor(cfg, it) {
  const set = (v) => { cfg.p[it.key] = v; touch(); };
  if (it.group) return h('h4', {}, it.group);
  if (it.type === 'select') return select({ label: it.label, options: it.options, value: cfg.p[it.key], onChange: set }).el;
  if (it.type === 'toggle') return toggle({ label: it.label, value: cfg.p[it.key], onChange: set }).el;
  if (it.type === 'segmented') return segmented({ label: it.label, options: it.options, value: cfg.p[it.key], onChange: set }).el;
  return slider({
    label: it.label, min: it.min, max: it.max, step: it.step, value: cfg.p[it.key], def: LAYER_TYPES[cfg.type].defaults[it.key],
    fmt: it.fmt, unit: it.unit, onInput: set,
    mod: it.step < 1 ? modFor(cfg.mods, 'p.' + it.key, 'layers', () => (scene.get(cfg.id)?.p ?? cfg.p)[it.key]) : undefined,
  }).el;
}

function layerRow(cfg) {
  const isBrush = cfg.type === 'draw' && cfg.id === brush.layerId;
  const i = config.layers.indexOf(cfg);
  const mini = (icon, title, fn, extra = '', disabled = false) =>
    h('button', { class: 'mini ' + extra, title, disabled, html: icon, onclick: (e) => { e.stopPropagation(); fn(); } });
  return h('div', {
    class: `layer${cfg.id === selectedId ? ' sel' : ''}${cfg.visible ? '' : ' off'}`,
    onclick: () => { selectedId = cfg.id === selectedId ? null : cfg.id; if (cfg.type === 'draw') { brush.layerId = cfg.id; renderDock(); } renderLayers(); },
  },
  mini(cfg.visible ? ICONS.eye : ICONS.eyeOff, cfg.visible ? 'Hide' : 'Show', () => { cfg.visible = !cfg.visible; touch(); renderLayers(); }, cfg.visible ? '' : 'off'),
  h('span', { class: 'dot', style: { background: DOT[cfg.type] } }),
  h('span', { class: 'nm' }, cfg.name, h('small', {}, isBrush ? 'brush' : '')),
  h('span', { class: 'acts' },
    mini(ICONS.up, 'Move up', () => moveLayer(cfg.id, 1), '', i === config.layers.length - 1),
    mini(ICONS.down, 'Move down', () => moveLayer(cfg.id, -1), '', i === 0),
    mini(ICONS.trash, 'Delete layer', () => removeLayer(cfg.id)),
  ));
}

function renderLayers() {
  const root = panels.layers;
  if (!root) return;
  root.replaceChildren();
  markers.layers.length = 0;
  root.append(section('Add layer', h('div', { class: 'chips' },
    Object.entries(LAYER_TYPES).map(([type, def]) => button('+ ' + def.label, () => addLayer(type), 'chip')))));
  const list = h('div', { class: 'layer-list' });
  for (const cfg of [...config.layers].reverse()) list.append(layerRow(cfg));
  root.append(section('Stack · top first', list));

  const cfg = config.layers.find((l) => l.id === selectedId);
  if (!cfg) {
    root.append(h('p', { class: 'hint' }, 'Select a layer to tune it. Every layer reacts to the mouse in its own way — try the balls with Repel, Swirl or Stir.'));
    return;
  }
  const name = h('input', { class: 'name-input', value: cfg.name, spellcheck: 'false' });
  name.addEventListener('input', () => { cfg.name = name.value; touch(); });
  name.addEventListener('change', () => { renderLayers(); renderDock(); });
  const sec = section(LAYER_TYPES[cfg.type].label, name,
    slider({
      label: 'Opacity', min: 0, max: 1, step: 0.01, value: cfg.opacity, def: 1, fmt: (v) => Math.round(v * 100) + '%',
      onInput: (v) => { cfg.opacity = v; touch(); }, mod: modFor(cfg.mods, 'opacity', 'layers', () => scene.get(cfg.id)?.eo ?? cfg.opacity),
    }).el,
    select({ label: 'Blend', options: BLENDS, value: cfg.blend, onChange: (v) => { cfg.blend = v; touch(); } }).el,
    slider({
      label: 'Blur', min: 0, max: 1, step: 0.01, value: cfg.blur, def: 0, fmt: (v) => Math.round(v * 100) + '%',
      onInput: (v) => { cfg.blur = v; touch(); }, mod: modFor(cfg.mods, 'blur', 'layers', () => scene.get(cfg.id)?.eb ?? cfg.blur),
    }).el,
  );
  if (cfg.type === 'draw') {
    sec.append(h('div', { class: 'btnrow' },
      button('Clear this layer', () => { scene.get(cfg.id)?.clear(); link.send({ type: 'clear', layer: cfg.id }); }),
      button('Draw here', () => { brush.layerId = cfg.id; renderDock(); renderLayers(); touch(); }, 'primary'),
    ));
  } else if (cfg.type === 'image') {
    const asset = getMediaAsset(cfg.p.src);
    const thumbUrl = asset ? asset.thumb : '';
    const imgName = asset ? asset.name : 'Choose an image';
    const card = h('div', { class: 'image-card-preview' },
      h('div', { class: 'image-card-thumb' },
        thumbUrl ? h('img', { src: thumbUrl, alt: imgName }) : null
      ),
      h('div', { class: 'image-card-meta' },
        h('b', {}, imgName),
        h('span', {}, asset ? (asset.category === 'user' ? 'Custom import' : 'Built-in library') : '')
      ),
      h('div', { class: 'image-card-actions' },
        button('Change…', () => openMediaPicker((id) => {
          cfg.p.src = id;
          const a = getMediaAsset(id);
          if (a && a.category === 'user') {
            link.send({ type: 'imageAsset', id: a.id, dataUrl: a.dataUrl, name: a.name });
          }
          touch();
          renderLayers();
        }, cfg.p.src), 'primary'),
      )
    );
    sec.append(card);
  }
  for (const it of LAYER_TYPES[cfg.type].schema) sec.append(controlFor(cfg, it));
  root.append(sec);
}

/* ------------------------------------------------------------- media picker */

let mediaCat = 'all';
let mediaPickerCb = null;

function openMediaPicker(onSelect, activeId) {
  mediaPickerCb = onSelect;
  const modal = $('mediaModal');
  modal.hidden = false;
  renderMediaGrid(activeId);
}

function closeMediaPicker() {
  const modal = $('mediaModal');
  modal.hidden = true;
  mediaPickerCb = null;
}

$('mediaClose').addEventListener('click', closeMediaPicker);
$('mediaModal').addEventListener('click', (e) => {
  if (e.target === $('mediaModal')) closeMediaPicker();
});

const mediaTabs = $('mediaTabs');
for (const b of mediaTabs.children) {
  b.addEventListener('click', () => {
    for (const it of mediaTabs.children) it.classList.toggle('on', it === b);
    mediaCat = b.dataset.cat;
    renderMediaGrid();
  });
}

$('mediaFileInput').addEventListener('change', async (e) => {
  const file = e.target.files?.[0];
  if (!file) return;
  e.target.value = '';
  const asset = await saveUserMedia(file);
  link.send({ type: 'imageAsset', id: asset.id, dataUrl: asset.dataUrl, name: asset.name });
  if (mediaPickerCb) {
    mediaPickerCb(asset.id);
    closeMediaPicker();
  } else {
    renderMediaGrid(asset.id);
  }
});

function renderMediaGrid(activeId) {
  const grid = $('mediaGrid');
  grid.replaceChildren();
  const { builtin, user } = getAllMedia();
  let list = [];
  if (mediaCat === 'all') list = [...builtin, ...user];
  else if (mediaCat === 'user') list = user;
  else list = builtin.filter((b) => b.category === mediaCat);

  if (!list.length) {
    grid.append(h('p', { class: 'hint', style: { gridColumn: '1 / -1', padding: '32px 0', textAlign: 'center' } },
      mediaCat === 'user' ? 'No custom images imported yet. Click "+ Import Image…" or drop images anywhere!' : 'No items in this category.'));
    return;
  }

  for (const item of list) {
    const isUser = item.category === 'user';
    const isSel = item.id === activeId;
    const el = h('div', {
      class: 'media-item' + (isSel ? ' selected' : ''),
      onclick: () => {
        if (mediaPickerCb) {
          mediaPickerCb(item.id);
          closeMediaPicker();
        }
      },
    },
      h('div', { class: 'thumb-box' }, h('img', { src: item.thumb, alt: item.name })),
      h('span', { class: 'item-name', title: item.name }, item.name),
      h('span', { class: 'item-cat' }, isUser ? 'Custom' : item.category),
      isUser ? h('button', {
        class: 'del-btn', title: 'Delete image',
        onclick: async (e) => {
          e.stopPropagation();
          await deleteUserMedia(item.id);
          renderMediaGrid(activeId);
        },
      }, '×') : null,
    );
    grid.append(el);
  }
}

/* ----------------------------------------------------------- drag & drop */

const dropOverlay = $('dropOverlay');
let dragCounter = 0;

window.addEventListener('dragenter', (e) => {
  if (e.dataTransfer && e.dataTransfer.types.includes('Files')) {
    dragCounter++;
    dropOverlay.hidden = false;
  }
});

window.addEventListener('dragleave', () => {
  dragCounter--;
  if (dragCounter <= 0) {
    dragCounter = 0;
    dropOverlay.hidden = true;
  }
});

window.addEventListener('dragover', (e) => {
  if (e.dataTransfer && e.dataTransfer.types.includes('Files')) {
    e.preventDefault();
  }
});

window.addEventListener('drop', async (e) => {
  dragCounter = 0;
  dropOverlay.hidden = true;
  e.preventDefault();
  const file = e.dataTransfer?.files?.[0];
  if (!file || !file.type.startsWith('image/')) return;

  const asset = await saveUserMedia(file);
  link.send({ type: 'imageAsset', id: asset.id, dataUrl: asset.dataUrl, name: asset.name });

  const selLayer = config.layers.find((l) => l.id === selectedId);
  if (selLayer && selLayer.type === 'image') {
    selLayer.p.src = asset.id;
    touch();
    renderLayers();
  } else {
    const n = config.layers.filter((l) => l.type === 'image').length;
    const cfg = makeLayerConfig('image', {
      name: asset.name || (n ? `Image ${n + 1}` : 'Image'),
      p: { src: asset.id },
    });
    config.layers.push(cfg);
    selectedId = cfg.id;
    commitStructure();
    showTab('layers');
  }
});

/* ---------------------------------------------------------------- scenes & automation */

function updateMonitorHud() {
  const hudRes = $('hudRes');
  const hudFps = $('hudFps');
  if (hudRes) hudRes.textContent = `${Math.round(CTRL_H * aspect)}×${CTRL_H}`;
  if (hudFps) hudFps.textContent = `${Math.round(fpsAvg)} FPS`;
}

function updateTopSceneBadge() {
  const pill = $('activeScenePill');
  if (!pill) return;
  const cur = allPresets().find((p) => p.id === activeSceneId) || allPresets()[sceneIdx] || allPresets()[0];
  if (cur) {
    const lbl = pill.querySelector('.scene-lbl');
    if (lbl) lbl.textContent = cur.name;
    pill.classList.toggle('on', true);
  }
}

function getPlaylist() {
  const all = allPresets();
  const enabled = all.filter((p) => p.enabled !== false);
  return enabled.length > 0 ? enabled : all;
}

function nextAutoPreset() {
  const list = getPlaylist();
  if (!list.length) return;
  if (settings.autoMode === 'shuffle' && list.length > 1) {
    let nextIdx;
    const curP = allPresets().find((p) => p.id === activeSceneId) || allPresets()[sceneIdx];
    do {
      nextIdx = Math.floor(Math.random() * list.length);
    } while (list[nextIdx] === curP && list.length > 1);
    applyPreset(list[nextIdx]);
  } else {
    const curP = allPresets().find((p) => p.id === activeSceneId) || allPresets()[sceneIdx];
    const curListIdx = list.indexOf(curP);
    const nextListIdx = (curListIdx + 1) % list.length;
    applyPreset(list[nextListIdx]);
  }
}

function prevAutoPreset() {
  const list = getPlaylist();
  if (!list.length) return;
  const curP = allPresets().find((p) => p.id === activeSceneId) || allPresets()[sceneIdx];
  const curListIdx = list.indexOf(curP);
  const prevListIdx = (curListIdx - 1 + list.length) % list.length;
  applyPreset(list[prevListIdx]);
}


/* ---------------------------------------------------------------- Central Beat Clock & Bottom Deck */

function tapTempo() {
  const now = performance.now();
  if (clock.taps.length > 0 && (now - clock.taps[clock.taps.length - 1] > 2500)) {
    clock.taps = [];
  }
  clock.taps.push(now);
  if (clock.taps.length > 5) clock.taps.shift();

  if (clock.taps.length >= 2) {
    let sum = 0;
    for (let i = 1; i < clock.taps.length; i++) {
      sum += (clock.taps[i] - clock.taps[i - 1]);
    }
    const avgMs = sum / (clock.taps.length - 1);
    if (avgMs > 0) {
      clock.bpm = Math.min(220, Math.max(40, Math.round((60000 / avgMs) * 10) / 10));
    }
  }

  // Align phase to downbeat on tap
  clock.phase = 0;
  clock.beatInBar = 0;
  flashMetronomeBeat(0);
  updateClockDisplay();

  const tapBtn = $('tapTempoBtn');
  if (tapBtn) {
    tapBtn.classList.add('flash');
    setTimeout(() => tapBtn.classList.remove('flash'), 120);
  }
  showToast(`Tempo set: ${clock.bpm.toFixed(1)} BPM`, 'info', 800);
}

function syncDownbeat() {
  clock.phase = 0;
  clock.beatInBar = 0;
  clock.loopBeat = 0;
  flashMetronomeBeat(0);
  updateClockDisplay();
  showToast('Beat Loop Reset to Downbeat (Beat 1)', 'info', 1000);
}

function setBpm(nv) {
  clock.bpm = Math.min(220, Math.max(40, Math.round(nv * 10) / 10));
  updateClockDisplay();
  touch();
}

function nudgeBpm(delta) {
  setBpm(clock.bpm + delta);
}

function flashMetronomeBeat(beatIdx) {
  const dots = $('beatDots');
  if (!dots) return;
  for (let i = 0; i < dots.children.length; i++) {
    const el = dots.children[i];
    el.classList.toggle('flash', i === beatIdx);
  }
}

function updateClockDisplay() {
  const bpmNum = $('bpmNumber');
  const bpmSlider = $('bpmSlider');
  if (bpmNum) bpmNum.textContent = clock.bpm.toFixed(1);
  if (bpmSlider) bpmSlider.value = clock.bpm;
}

function tickClock(dt) {
  if (clock.source === 'audio') {
    if (levels[LEVEL.beat] > 0.75 && (simT - lastBeatDetectTime > 0.18)) {
      lastBeatDetectTime = simT;
      clock.beatInBar = (clock.beatInBar + 1) % 4;
      clock.loopBeat = (clock.loopBeat + 1) % (clock.loopLength || 32);
      flashMetronomeBeat(clock.beatInBar);
      if (settings.auto && !settings.autoPaused && settings.autoMode === 'beats') {
        if (clock.loopBeat === 0) nextAutoPreset();
      }
    }
  } else {
    const beatDur = 60 / Math.max(20, clock.bpm);
    const prevPhase = clock.phase;
    clock.phase += dt / beatDur;

    const prevBeat = Math.floor(prevPhase);
    const curBeat = Math.floor(clock.phase);

    if (curBeat > prevBeat) {
      clock.beatInBar = curBeat % 4;
      clock.loopBeat = (clock.loopBeat + 1) % (clock.loopLength || 32);
      flashMetronomeBeat(clock.beatInBar);

      if (settings.auto && !settings.autoPaused && settings.autoMode === 'beats') {
        if (clock.loopBeat === 0) {
          nextAutoPreset();
        }
      }
    }
  }

  // Update metronome loop readout
  const loopReadout = $('loopBeatReadout');
  const loopBar = $('loopProgressBar');
  if (loopReadout) loopReadout.textContent = `Beat ${clock.loopBeat + 1} / ${clock.loopLength}`;
  if (loopBar) loopBar.style.width = `${((clock.loopBeat + 1) / clock.loopLength) * 100}%`;
}

function tickAutoCycle(dt) {
  tickClock(dt);
  if (levels[LEVEL.beat] > 0.75 && (simT - lastBeatDetectTime > 0.18)) {
    lastBeatDetectTime = simT;
    beatCount++;
  }

  const all = allPresets();
  const curPreset = all.find((p) => p.id === activeSceneId) || all[sceneIdx] || all[0];
  const targetHold = (curPreset && curPreset.hold != null && curPreset.hold > 0)
    ? curPreset.hold
    : settings.autoEvery;

  if (settings.auto && !settings.autoPaused) {
    if (settings.autoMode === 'beats') {
      const targetBeats = Number(settings.autoBeats) || 32;
      if (beatCount >= targetBeats) {
        beatCount = 0;
        nextAutoPreset();
      }
    } else {
      autoT += dt;
      if (autoT >= targetHold) {
        autoT = 0;
        nextAutoPreset();
      }
    }
  }

  updateTimelineProgress(targetHold);
}

function updateTimelineProgress(targetHold) {
  const prg = $('timelineProgress');
  const elapsedEl = $('timelineElapsed');
  const totalEl = $('timelineTotal');
  const statusEl = $('timelineStatus');
  const nextEl = $('timelineNextScene');
  if (!prg && !elapsedEl) return;

  const isBeat = settings.autoMode === 'beats';
  const target = isBeat ? (Number(settings.autoBeats) || 32) : targetHold;
  const cur = isBeat ? beatCount : autoT;
  const pct = Math.min(100, Math.max(0, (cur / target) * 100));

  if (prg) {
    prg.style.width = `${pct}%`;
    prg.classList.toggle('beat-mode', isBeat);
    prg.classList.toggle('dissolving', scene.ghostA > 0.05);
  }

  if (elapsedEl) {
    elapsedEl.textContent = isBeat ? `Beat ${beatCount}` : `${Math.floor(cur)}s`;
  }
  if (totalEl) {
    totalEl.textContent = isBeat ? `/ ${target} beats` : `/ ${Math.round(target)}s`;
  }
  if (statusEl) {
    if (!settings.auto) statusEl.textContent = 'STOPPED';
    else if (settings.autoPaused) statusEl.textContent = 'PAUSED';
    else if (scene.ghostA > 0.05) statusEl.textContent = `DISSOLVING (${(scene.ghostDur * scene.ghostA).toFixed(1)}s)`;
    else statusEl.textContent = 'HOLDING';
  }
  if (nextEl) {
    const list = getPlaylist();
    const curP = allPresets().find((p) => p.id === activeSceneId) || allPresets()[sceneIdx];
    const nextIdx = (list.indexOf(curP) + 1) % list.length;
    const nextP = list[nextIdx];
    if (nextP) nextEl.textContent = `Next: ${nextP.name}`;
  }
}

function overwritePreset(p) {
  if (!p) return;
  thumbReq = (thumb) => {
    const snap = snapshotPreset(p.name, fx.target, fx.mods, config, thumb, {
      id: p.id || uid(),
      glide: p.glide,
      hold: p.hold,
      enabled: p.enabled !== false,
      user: true,
    });
    const idx = userPresets.findIndex((x) => x.id === snap.id || x.name === snap.name);
    if (idx !== -1) userPresets[idx] = snap;
    else userPresets.push(snap);
    activeSceneId = snap.id;
    renderScenes();
    updateTopSceneBadge();
    touch();
    showToast(`Saved over "${p.name}" with current look!`, 'success');
  };
}

function saveNewPreset(name) {
  const nm = name.trim() || `Scene ${userPresets.length + 1}`;
  thumbReq = (thumb) => {
    const snap = snapshotPreset(nm, fx.target, fx.mods, config, thumb, {
      user: true,
      enabled: true,
    });
    userPresets.push(snap);
    activeSceneId = snap.id;
    renderScenes();
    updateTopSceneBadge();
    touch();
    showToast(`Saved new scene "${nm}"!`, 'success');
  };
}

function duplicatePreset(p) {
  const clone = {
    ...p,
    id: uid(),
    name: `${p.name} (Copy)`,
    user: true,
    enabled: true,
  };
  userPresets.push(clone);
  renderScenes();
  touch();
  showToast(`Duplicated scene as "${clone.name}"`, 'info');
}

function deletePreset(p) {
  if (!p.user) {
    const idx = userPresets.findIndex((x) => x.name === p.name);
    if (idx !== -1) {
      userPresets.splice(idx, 1);
      renderScenes();
      touch();
      showToast(`Reset "${p.name}" to built-in default`, 'info');
      return;
    }
    showToast('Built-in scenes cannot be deleted.', 'warning');
    return;
  }
  userPresets = userPresets.filter((x) => x.id !== p.id && x !== p);
  if (activeSceneId === p.id) {
    activeSceneId = BUILTIN_PRESETS[0].id;
    sceneIdx = 0;
  }
  renderScenes();
  updateTopSceneBadge();
  touch();
  showToast(`Deleted "${p.name}"`, 'info');
}

function recaptureThumbnail(p) {
  thumbReq = (thumb) => {
    p.thumb = thumb;
    const idx = userPresets.findIndex((x) => x.id === p.id || x.name === p.name);
    if (idx !== -1) userPresets[idx].thumb = thumb;
    else if (!p.user) {
      const snap = { ...p, thumb, id: uid(), user: true };
      userPresets.push(snap);
    }
    renderScenes();
    touch();
    showToast(`Updated thumbnail for "${p.name}"`, 'success');
  };
}

function openSceneEditor(p) {
  const modal = $('sceneEditModal');
  const body = $('sceneEditBody');
  if (!modal || !body) return;
  body.replaceChildren();

  const colors = swatchOf(p);
  const bg = p.thumb ? `url(${p.thumb})` : `linear-gradient(135deg, ${colors[0]}, ${colors[1] || colors[0]} 55%, ${colors[2] || colors[0]})`;

  const preview = h('div', { class: 'scene-edit-preview-row' },
    h('div', { class: 'scene-edit-thumb', style: { backgroundImage: bg } }),
    h('div', { class: 'scene-edit-preview-info' },
      h('b', {}, p.name),
      h('span', {}, p.user ? 'Custom Preset' : 'Built-in Preset', ` · ${p.layers ? p.layers.length : 0} layers`)
    ),
    button(h('span', { html: ICONS.camera }), () => recaptureThumbnail(p), 'icon', 'Recapture thumbnail from current canvas')
  );

  const nameInput = h('input', { class: 'name-input', value: p.name, placeholder: 'Scene name' });
  nameInput.addEventListener('change', () => {
    const val = nameInput.value.trim();
    if (!val) return;
    p.name = val;
    if (p.user) {
      const idx = userPresets.findIndex((x) => x.id === p.id);
      if (idx !== -1) userPresets[idx].name = val;
    }
    renderScenes();
    updateTopSceneBadge();
    touch();
    showToast(`Renamed to "${val}"`, 'info');
  });

  const actionRow = h('div', { class: 'btnrow' },
    button([h('span', { html: ICONS.save }), ' Overwrite with Current Look'], () => overwritePreset(p), 'primary'),
    button([h('span', { html: ICONS.copy }), ' Duplicate'], () => { duplicatePreset(p); modal.hidden = true; }),
  );

  const autoToggle = toggle({
    label: 'Include in Auto-Cycle Playlist',
    value: p.enabled !== false,
    onChange: (v) => {
      p.enabled = v;
      const idx = userPresets.findIndex((x) => x.id === p.id || x.name === p.name);
      if (idx !== -1) userPresets[idx].enabled = v;
      else if (!p.user) {
        const snap = { ...p, enabled: v, id: uid(), user: true };
        userPresets.push(snap);
      }
      renderScenes();
      touch();
    }
  });

  const holdSlider = slider({
    label: 'Hold Duration',
    min: 0, max: 180, step: 1,
    value: p.hold != null ? p.hold : 0,
    def: 0,
    fmt: (v) => (v === 0 ? `Global (${settings.autoEvery}s)` : `${v} s`),
    tip: 'Set 0 to follow the global Auto-Cycle timer',
    onInput: (v) => {
      p.hold = v === 0 ? null : v;
      const idx = userPresets.findIndex((x) => x.id === p.id || x.name === p.name);
      if (idx !== -1) userPresets[idx].hold = p.hold;
      else if (!p.user) {
        const snap = { ...p, hold: p.hold, id: uid(), user: true };
        userPresets.push(snap);
      }
      touch();
    }
  });

  const glideSlider = slider({
    label: 'Dissolve Time',
    min: 0, max: 8, step: 0.1,
    value: p.glide != null ? p.glide : 0,
    def: 0,
    fmt: (v) => (v === 0 ? `Global (${settings.glide}s)` : `${v.toFixed(1)} s`),
    tip: 'Set 0 to follow the global Dissolve setting',
    onInput: (v) => {
      p.glide = v === 0 ? null : v;
      const idx = userPresets.findIndex((x) => x.id === p.id || x.name === p.name);
      if (idx !== -1) userPresets[idx].glide = p.glide;
      else if (!p.user) {
        const snap = { ...p, glide: p.glide, id: uid(), user: true };
        userPresets.push(snap);
      }
      touch();
    }
  });

  body.append(
    preview,
    section('Scene Name', nameInput),
    actionRow,
    section('Automation & Transition', autoToggle.el, holdSlider.el, glideSlider.el)
  );

  if (p.user) {
    const delBtn = button([h('span', { html: ICONS.trash }), ' Delete Scene'], () => {
      deletePreset(p);
      modal.hidden = true;
    }, 'danger on');
    body.append(section('Danger Zone', delBtn));
  }

  modal.hidden = false;
}

function exportScenes() {
  const exportData = {
    app: 'Lumex Pro',
    version: 2,
    exportedAt: new Date().toISOString(),
    presets: userPresets,
  };
  const str = JSON.stringify(exportData, null, 2);
  const blob = new Blob([str], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `lumex_scenes_${Date.now()}.json`;
  a.click();
  URL.revokeObjectURL(url);
  showToast('Exported custom scenes to JSON!', 'success');
}

function renderScenes() {
  const bottom = $('bottomPanel');
  if (!bottom) return;
  bottom.replaceChildren();

  const all = allPresets();
  const curActive = all.find((p) => p.id === activeSceneId) || all[sceneIdx] || all[0];

  // ------------------------------------------------ LEFT: Central Beat Clock & Sequencer Deck
  const clockDeck = h('div', { class: 'clock-deck' });

  const clockHead = h('div', { class: 'clock-header' },
    h('div', { class: 'clock-title' }, h('i'), 'Central Beat Clock'),
    segmented({
      options: [['tap', 'Manual Clock'], ['audio', 'Audio In']],
      value: clock.source,
      onChange: (src) => { clock.source = src; touch(); }
    }).el
  );

  // Tap button & BPM readout
  const tapBtn = h('button', {
    class: 'tap-btn', id: 'tapTempoBtn', title: 'Tap repeatedly to set tempo with music',
    onclick: () => tapTempo()
  }, h('b', {}, 'TAP'), h('span', { class: 'tap-sub' }, 'TEMPO'));

  const bpmReadout = h('div', { class: 'bpm-readout-row' },
    h('span', { class: 'bpm-number', id: 'bpmNumber' }, clock.bpm.toFixed(1)),
    h('span', { class: 'bpm-unit' }, 'BPM'),
    h('div', { class: 'bpm-nudge-btns' },
      h('button', { class: 'nudge-btn', onclick: () => nudgeBpm(-1) }, '-1'),
      h('button', { class: 'nudge-btn', onclick: () => nudgeBpm(1) }, '+1'),
      h('button', { class: 'nudge-btn', onclick: () => nudgeBpm(-0.1) }, '-.1'),
      h('button', { class: 'nudge-btn', onclick: () => nudgeBpm(0.1) }, '+.1')
    )
  );

  const bpmSliderInput = h('input', {
    type: 'range', min: 40, max: 220, step: 0.5, value: clock.bpm, id: 'bpmSlider',
    oninput: (e) => setBpm(parseFloat(e.target.value))
  });
  const bpmRow = h('div', { class: 'tempo-tap-row' }, tapBtn, h('div', { class: 'bpm-display-wrap' }, bpmReadout, bpmSliderInput));

  // Metronome & Downbeat reset
  const beatDots = h('div', { class: 'beat-dots', id: 'beatDots' },
    h('div', { class: 'beat-dot downbeat' }, '1'),
    h('div', { class: 'beat-dot' }, '2'),
    h('div', { class: 'beat-dot' }, '3'),
    h('div', { class: 'beat-dot' }, '4')
  );

  const syncBtn = h('button', {
    class: 'downbeat-btn', id: 'syncDownbeatBtn', title: 'Align immediately to Beat 1 (Downbeat/Drop)',
    onclick: () => syncDownbeat()
  }, '⟲ SYNC BEAT 1');

  const metronomeRow = h('div', { class: 'metronome-row' }, beatDots, syncBtn);

  // Beat Loop Length & Transport row
  const loopLenSeg = segmented({
    options: [[4, '4'], [8, '8'], [16, '16'], [32, '32'], [64, '64']],
    value: clock.loopLength,
    onChange: (len) => { clock.loopLength = Number(len); clock.loopBeat = 0; touch(); }
  });

  const loopRow = h('div', { class: 'loop-length-row' },
    h('span', {}, 'Loop Length:'),
    loopLenSeg.el,
    h('span', { id: 'loopBeatReadout', style: { color: 'var(--accent-cyan)', fontWeight: '700' } }, `Beat 1 / ${clock.loopLength}`)
  );

  // Auto-Cycle Transport
  const isPaused = settings.autoPaused;
  const playBtn = h('button', {
    class: 'transport-btn play-btn',
    title: settings.auto ? (isPaused ? 'Resume Auto-Cycle' : 'Pause Auto-Cycle') : 'Start Auto-Cycle',
    onclick: () => {
      if (!settings.auto) { settings.auto = true; settings.autoPaused = false; autoT = 0; beatCount = 0; }
      else { settings.autoPaused = !settings.autoPaused; }
      renderScenes(); touch();
    }
  }, h('span', { html: (settings.auto && !isPaused) ? ICONS.pause : ICONS.play }));

  const prevBtn = h('button', { class: 'transport-btn', title: 'Prev Scene', onclick: () => prevAutoPreset() }, h('span', { html: ICONS.skipPrev }));
  const nextBtn = h('button', { class: 'transport-btn', title: 'Next Scene', onclick: () => nextAutoPreset() }, h('span', { html: ICONS.skipNext }));

  const autoModeSeg = segmented({
    options: [['beats', 'Beat Drop'], ['sequence', 'Seq Timer'], ['shuffle', 'Shuffle']],
    value: settings.autoMode || 'beats',
    onChange: (m) => { settings.autoMode = m; touch(); renderScenes(); }
  });

  const transportBar = h('div', { style: { display: 'flex', alignItems: 'center', gap: '6px' } },
    h('div', { class: 'transport-bar', style: { padding: 0 } }, prevBtn, playBtn, nextBtn),
    autoModeSeg.el
  );

  clockDeck.append(clockHead, bpmRow, metronomeRow, loopRow, transportBar);

  // ------------------------------------------------ RIGHT: Scene Selector Timeline Strip
  const sceneDeck = h('div', { class: 'scene-deck' });

  // Overwrite active scene button
  const saveOverBtn = curActive ? button(
    [h('span', { html: ICONS.save }), ` Save Over "${curActive.name}"`],
    () => overwritePreset(curActive),
    'primary small',
    'Update active scene with live canvas'
  ) : null;

  // Save new scene
  const quickName = h('input', { class: 'name-input', style: { height: '22px', width: '110px', margin: 0 }, placeholder: 'New look…' });
  const quickSaveBtn = button('Save New', () => { saveNewPreset(quickName.value); quickName.value = ''; }, 'small');
  quickName.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') { saveNewPreset(quickName.value); quickName.value = ''; } });

  const filterSeg = segmented({
    options: [['all', 'All'], ['builtin', 'Built-in'], ['user', 'Custom']],
    value: settings.sceneFilter || 'all',
    onChange: (f) => { settings.sceneFilter = f; renderScenes(); touch(); }
  });

  const headerRight = h('div', { style: { display: 'flex', gap: '6px', alignItems: 'center' } },
    saveOverBtn,
    h('div', { style: { display: 'flex', gap: '3px', alignItems: 'center' } }, quickName, quickSaveBtn),
    filterSeg.el,
    button(h('span', { html: ICONS.download }), exportScenes, 'icon small', 'Export Scenes'),
    button(h('span', { html: ICONS.upload }), () => $('sceneImportInput')?.click(), 'icon small', 'Import Scenes')
  );

  const sceneDeckHead = h('div', { class: 'scene-deck-header' },
    h('div', { class: 'scene-deck-title' }, `Scenes (${all.length})`),
    headerRight
  );

  // Horizontal Strip
  const stripScroll = h('div', { class: 'scene-strip-scroll' });

  let filtered = all;
  if (settings.sceneFilter === 'builtin') filtered = filtered.filter((p) => !p.user);
  else if (settings.sceneFilter === 'user') filtered = filtered.filter((p) => p.user);

  filtered.forEach((p, i) => {
    const colors = swatchOf(p);
    const bg = p.thumb ? `url(${p.thumb})` : `linear-gradient(135deg, ${colors[0]}, ${colors[1] || colors[0]} 55%, ${colors[2] || colors[0]})`;
    const isActive = (p.id === activeSceneId) || (!activeSceneId && i === 0);

    const card = h('div', {
      class: `scene-strip-card${isActive ? ' active-scene' : ''}`,
      style: { backgroundImage: bg },
      onclick: () => applyPreset(p)
    },
      h('span', { class: 'shade' }),
      i < 9 ? h('kbd', { class: 'key' }, String(i + 1)) : null,
      isActive ? h('span', { class: 'live-badge' }, 'LIVE') : null,
      h('span', { class: 'nm' }, p.name),
      h('div', { class: 'scene-actions' },
        h('button', {
          class: 'scene-act-btn save-over', title: 'Overwrite with current look',
          onclick: (e) => { e.stopPropagation(); overwritePreset(p); }
        }, h('span', { html: ICONS.save })),
        h('button', {
          class: 'scene-act-btn', title: 'Edit scene settings',
          onclick: (e) => { e.stopPropagation(); openSceneEditor(p); }
        }, h('span', { html: ICONS.pencil })),
        h('button', {
          class: 'scene-act-btn', title: 'Duplicate scene',
          onclick: (e) => { e.stopPropagation(); duplicatePreset(p); }
        }, h('span', { html: ICONS.copy })),
        p.user ? h('button', {
          class: 'scene-act-btn del-btn', title: 'Delete scene',
          onclick: (e) => { e.stopPropagation(); deletePreset(p); }
        }, h('span', { html: ICONS.close })) : null
      )
    );
    stripScroll.append(card);
  });

  sceneDeck.append(sceneDeckHead, stripScroll);
  bottom.append(clockDeck, sceneDeck);
}

/* ----------------------------------------------------------------- audio */

const METERS = [['bass', 'Bass'], ['mid', 'Mid'], ['high', 'High'], ['beat', 'Beat']];

/** Level bars for the music. `full` draws wide labelled rows (with the input level), otherwise slim vertical bars. */
function meter(full) {
  const bars = (full ? [['input', 'Input'], ...METERS] : METERS).map(([k, label]) => {
    const fill = h('i');
    const trk = h('span', { class: 'trk' }, fill);
    return { k, fill, el: h('div', { class: 'bar ' + k, title: label }, full ? h('b', {}, label) : null, trk) };
  });
  const axis = full ? 'scaleX' : 'scaleY';
  return {
    el: h('div', { class: 'meter ' + (full ? 'full' : 'mini') }, bars.map((b) => b.el)),
    tick() {
      const on = audioState.status === 'on';
      for (const b of bars) {
        const v = on ? levels[LEVEL[b.k]] : 0;
        b.fill.style.transform = `${axis}(${v.toFixed(3)})`;
        if (b.k === 'input') b.el.classList.toggle('clip', audioState.clip);
      }
    },
  };
}

const dB = (g) => { const d = Math.round(20 * Math.log10(g)); return (d > 0 ? '+' : '') + d + ' dB'; };

function toggleAudio(on) {
  audioCfg.on = on;
  if (on) startAudio(); else stopAudio();
  touch();
}

function renderAudio() {
  const root = panels.audio;
  if (!root) return;
  root.replaceChildren();
  meters.audio.length = 0;
  const st = audioState;

  const box = h('input', { type: 'checkbox' });
  box.checked = audioCfg.on;
  box.addEventListener('change', () => toggleAudio(box.checked));
  const sw = h('label', { class: 'switch-only', title: 'Listen on / off (A)' }, box, h('span', { class: 'switch' }));
  const text = {
    off: 'Off. Switch on to let the music drive the visuals.',
    starting: 'Starting…',
    on: `Listening · ${st.label}${st.suspended ? ' — click anywhere to start the audio engine' : ''}`,
  }[st.status] || st.message;
  const card = h('div', { class: `lfo-card aud aud-card ${st.status}${st.status === 'on' ? ' on' : ''}` },
    h('div', { class: 'lfo-head' }, h('h3', {}, 'Audio input'), sw),
    h('p', { class: 'aud-status' }, text));
  if (st.status === 'denied' || st.status === 'error') {
    card.append(h('div', { class: 'btnrow' },
      st.status === 'denied' && bridge ? button('Open System Settings', micHelp, 'small primary') : null,
      button('Try again', () => startAudio(), 'small')));
  }
  root.append(card);

  const opts = [['default', 'System default'],
    ...st.devices.filter((d) => d.id !== 'default' && d.id !== 'communications').map((d) => [d.id, d.label]),
    ['demo', 'Demo beat (no input needed)']];
  if (!opts.some(([v]) => v === audioCfg.device)) opts.splice(1, 0, [audioCfg.device, 'Saved input (not connected)']);
  const full = meter(true);
  meters.audio.push(full);
  root.append(
    section('Input',
      select({ label: 'Source', options: opts, value: audioCfg.device, onChange: (v) => { audioCfg.device = v; touch(); if (audioCfg.on) startAudio(); } }).el,
      h('div', { class: 'lfo-body aud' },
        knob({ label: 'Gain', min: 0.25, max: 8, log: true, value: audioCfg.gain, def: 1, fmt: dB, onInput: (v) => { setGain(v); touch(); } }).el,
        h('div', { class: 'col' },
          toggle({ label: 'Auto level', value: audioCfg.auto, onChange: (v) => { audioCfg.auto = v; touch(); } }).el,
          h('p', { class: 'lfo-note' }, 'Auto level keeps every band lively at any volume. Off: set the level by hand with Gain.')))),
    section('Levels', full.el,
      h('p', { class: 'hint' }, 'Keep Input in the middle of its bar and out of the red. The other bars are what the sliders see.')),
    section('Reactions',
      h('div', { class: 'btnrow' },
        button('React to music', randomizeReactions, 'primary', 'Give a handful of effects a Bass / Mid / High / Beat reaction'),
        button('Clear', () => { for (const k in fx.mods) if (isAudio(fx.mods[k])) delete fx.mods[k]; renderFx(); touch(); }, '', 'Remove the music reactions from the effects')),
      h('p', { class: 'hint' }, 'Or open the wave button next to any slider (effects and layers) and pick Bass, Mid, High or Beat. Intensity is how far the music may push it, Smooth how slowly it lets go. The Audio knob at the top of the FX tab scales them all.')),
  );
}

const audioPill = $('audioStatus');
let beatDot = false;

function updateAudioStatus() {
  const s = audioState.status;
  audioPill.className = 'status pill' + (s === 'on' ? ' aud-on' : s === 'denied' || s === 'error' ? ' err' : '');
  beatDot = false;
  audioPill.lastElementChild.textContent = { off: 'Audio off', starting: 'Audio…', on: audioCfg.device === 'demo' ? 'Audio · demo' : 'Audio on', denied: 'Audio blocked', error: 'Audio problem' }[s];
}

function tickAudioDot() {
  const b = audioState.status === 'on' && levels[LEVEL.beat] > 0.5;
  if (b !== beatDot) { beatDot = b; audioPill.classList.toggle('beat', b); }
}
audioPill.addEventListener('click', () => showTab('audio'));

/* ---------------------------------------------------------------- output */

let displays = [];

async function refreshDisplays() {
  if (!bridge) return;
  displays = await bridge.listDisplays();
  if (!settings.display || (settings.display !== 'window' && !displays.some((d) => String(d.id) === settings.display))) {
    const ext = displays.find((d) => !d.primary);
    settings.display = ext ? String(ext.id) : 'window';
  }
  renderOutput();
}

let outCard = null;
function updateOutputCard() {
  if (!outCard) return;
  outCard.classList.toggle('ok', out.connected);
  outCard.firstChild.textContent = out.connected ? 'Live' : 'Not open';
  outCard.lastChild.textContent = out.connected
    ? `${out.w} × ${out.h} px · ${out.fps || 0} fps`
    : 'Open the output window, then drag it to the beamer or pick a display.';
}

function renderOutput() {
  const root = panels.output;
  if (!root) return;
  root.replaceChildren();

  const sec = section('Beamer window');
  outCard = h('div', { class: 'card' }, h('b'), h('div', { class: 'sub' }));
  updateOutputCard();
  sec.append(outCard);

  if (bridge) {
    const opts = [...displays.map((d) => [String(d.id), `${d.label}${d.primary ? ' (this screen)' : ''} · ${d.width}×${d.height}`]), ['window', 'Window (drag it yourself)']];
    sec.append(select({ label: 'Show on', options: opts, value: settings.display || 'window', onChange: (v) => { settings.display = v; touch(); } }).el);
    sec.append(h('div', { class: 'btnrow' },
      button(out.connected ? 'Close output' : 'Open output', toggleOutput, out.connected ? '' : 'primary'),
      button('Fullscreen', () => bridge.toggleFullscreen(), '', 'Toggle fullscreen on the screen the window is on'),
    ));
    if (displays.length < 2) sec.append(h('p', { class: 'hint' }, 'Only one screen found. Connect the beamer and it shows up here automatically.'));
  } else {
    sec.append(h('div', { class: 'btnrow' }, button(out.connected ? 'Close output' : 'Open output', toggleOutput, out.connected ? '' : 'primary')));
    sec.append(h('p', { class: 'hint' }, 'Drag the output window to the beamer, then double-click it for fullscreen.'));
  }
  root.append(sec);

  root.append(section('Picture',
    select({
      label: 'Resolution', value: String(settings.outH),
      options: [['720', '720p · smooth, fastest'], ['1080', '1080p · sharper'], ['1440', '1440p · sharpest']],
      onChange: (v) => { settings.outH = Number(v); sendConfig(); touch(); },
    }).el,
    select({
      label: 'Aspect', value: settings.aspectMode,
      options: [['auto', 'Match output window'], ['16:9', '16:9'], ['16:10', '16:10'], ['4:3', '4:3'], ['1:1', 'Square']],
      onChange: (v) => { settings.aspectMode = v; setAspect(aspectFromMode()); touch(); },
    }).el,
    toggle({
      label: 'Test pattern on output', value: settings.testPattern,
      onChange: (v) => { settings.testPattern = v; link.send({ type: 'testpattern', on: v }); touch(); },
    }).el,
    h('p', { class: 'hint' }, 'Resolution only affects the output window. Use the test pattern to line up the picture with the screen edges.'),
  ));
}

/* -------------------------------------------------------------- shortcuts */

let prevSpeed = 1;
addEventListener('keydown', (e) => {
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  const t = e.target;
  if (t instanceof HTMLInputElement && t.type !== 'range' && t.type !== 'checkbox' && t.type !== 'color') return;
  if (t instanceof HTMLSelectElement) return;
  const k = e.key;
  if (k === 'Escape') { $('help').hidden = true; return; }
  if (k === ' ') {
    e.preventDefault();
    if (fx.target.speed > 0) { prevSpeed = fx.target.speed; fx.set({ speed: 0 }); } else fx.set({ speed: prevSpeed || 1 });
    syncFxSliders();
  } else if (k === 'b' || k === 'B') toggleBlackout();
  else if (k === 'l' || k === 'L') { master.on = !master.on; renderFx(); touch(); }
  else if (k === 'a' || k === 'A') { toggleAudio(!audioCfg.on); renderAudio(); }
  else if (k === 'c') clearCurrent();
  else if (k === 'C') clearCurrent(true);
  else if (k === 'd') setTool('draw');
  else if (k === 'e') setTool('erase');
  else if (k === 'p') setTool('interact');
  else if (k === 'v') setView(settings.view === 'final' ? 'source' : 'final');
  else if (k === 'o') toggleOutput();
  else if (k === 'f') bridge?.toggleFullscreen();
  else if (k === 'k') {
    if (fx.target.kN > 0) { fx.lastN = fx.target.kN; fx.set({ kN: 0 }); } else fx.set({ kN: fx.lastN || 6 });
    syncFxSliders(); touch();
  } else if (k === '[' || k === ']') {
    brush.size = clamp(Math.round(brush.size * (k === ']' ? 1.2 : 1 / 1.2)), 2, 140);
    sizeSlider?.set(brush.size); updateCursor(); touch();
  } else if (k === '?' || k === '/') $('help').hidden = false;
  else if (/^[1-9]$/.test(k)) {
    const p = [...BUILTIN_PRESETS, ...userPresets][Number(k) - 1];
    if (p) applyPreset(p);
  } else return;
});
$('helpClose').addEventListener('click', () => ($('help').hidden = true));
$('help').addEventListener('click', (e) => { if (e.target === $('help')) $('help').hidden = true; });

$('sceneEditClose')?.addEventListener('click', () => { $('sceneEditModal').hidden = true; });
$('sceneEditModal')?.addEventListener('click', (e) => { if (e.target === $('sceneEditModal')) $('sceneEditModal').hidden = true; });

$('sceneImportInput')?.addEventListener('change', (e) => {
  const file = e.target.files?.[0];
  if (!file) return;
  e.target.value = '';
  const reader = new FileReader();
  reader.onload = () => {
    try {
      const data = JSON.parse(reader.result);
      const incoming = Array.isArray(data) ? data : (data.presets || []);
      if (!incoming.length) throw new Error('No valid presets found in JSON');
      let count = 0;
      for (const item of incoming) {
        if (!item.name || !item.layers) continue;
        const copy = { ...item, id: uid(), user: true };
        userPresets.push(copy);
        count++;
      }
      renderScenes();
      touch();
      showToast(`Imported ${count} scene(s) into library!`, 'success');
    } catch (err) {
      showToast('Import error: ' + err.message, 'warning');
    }
  };
  reader.readAsText(file);
});

$('activeScenePill')?.addEventListener('click', () => { showTab('scenes'); });

/* ------------------------------------------------------------------ boot */

buildTabs();
renderFx();
renderLayers();
renderScenes();
renderAudio();
renderOutput();
ensureBrushLayer();
renderDock();
renderTopActions();
setView(settings.view);
fitStage();
if (bridge) {
  refreshDisplays();
  bridge.onDisplays((d) => { displays = d; refreshDisplays(); });
  bridge.onOutputClosed(() => { out.connected = false; renderTopActions(); renderOutput(); });
}
setInterval(() => { if (out.connected) { updateStatus(); updateOutputCard(); } }, 1000);
updateAudioStatus();
updateTopSceneBadge();
onAudioChange(() => { renderAudio(); updateAudioStatus(); });
refreshDevices();
if (audioCfg.on) startAudio();
requestAnimationFrame(frame);
window.__lumen = {
  fx, config, scene, brush, mouse, settings, out, get fps() { return fpsAvg; }, applyPreset,
  audio: { cfg: audioCfg, state: audioState, levels, ctx: audioContext, input: audioInputNode, start: startAudio, stop: stopAudio, update: updateAudio },
};
