import { Scene } from './scene.js';
import { Pipeline } from './pipeline.js';
import { openLink } from './link.js';
import { registerAsset, loadUserMedia } from './media.js';

loadUserMedia();

const link = openLink();
const canvas = document.getElementById('c');
const grid = document.getElementById('grid');
const wait = document.getElementById('wait');
const pipe = new Pipeline(canvas);

let scene = null;
let aspect = 16 / 9;
let frame = null;
let frameAt = 0;
let ready = false;
let frames = 0;

function fit() {
  const dpr = window.devicePixelRatio || 1;
  const cw = innerWidth * dpr, ch = innerHeight * dpr;
  let w = cw, h = cw / aspect;
  if (h > ch) { h = ch; w = ch * aspect; }
  const k = Math.min(1, 2560 / Math.max(w, h));
  canvas.width = Math.round(w * k);
  canvas.height = Math.round(h * k);
  canvas.style.width = w / dpr + 'px';
  canvas.style.height = h / dpr + 'px';
  grid.width = canvas.width; grid.height = canvas.height;
  grid.style.width = canvas.style.width; grid.style.height = canvas.style.height;
  drawGrid();
}

function drawGrid() {
  const g = grid.getContext('2d'), w = grid.width, h = grid.height;
  g.clearRect(0, 0, w, h);
  g.strokeStyle = 'rgba(255,255,255,.55)';
  g.fillStyle = 'rgba(255,255,255,.8)';
  g.lineWidth = Math.max(1, h / 540);
  const cols = 16, rows = 9;
  g.beginPath();
  for (let i = 0; i <= cols; i++) { g.moveTo((w * i) / cols, 0); g.lineTo((w * i) / cols, h); }
  for (let j = 0; j <= rows; j++) { g.moveTo(0, (h * j) / rows); g.lineTo(w, (h * j) / rows); }
  g.stroke();
  g.strokeStyle = '#ff2d95';
  g.lineWidth = Math.max(2, h / 270);
  g.strokeRect(g.lineWidth / 2, g.lineWidth / 2, w - g.lineWidth, h - g.lineWidth);
  g.beginPath();
  g.arc(w / 2, h / 2, h * 0.4, 0, Math.PI * 2);
  g.moveTo(w / 2 - 20, h / 2); g.lineTo(w / 2 + 20, h / 2);
  g.moveTo(w / 2, h / 2 - 20); g.lineTo(w / 2, h / 2 + 20);
  g.stroke();
  g.font = `600 ${Math.round(h / 22)}px -apple-system, system-ui, sans-serif`;
  g.textAlign = 'center';
  g.fillText(`${canvas.width} × ${canvas.height}`, w / 2, h / 2 + h * 0.46);
}

function applyConfig(msg) {
  aspect = msg.aspect;
  if (!scene) scene = new Scene(msg.outH, aspect);
  else scene.resize(msg.outH, aspect);
  scene.setConfig(msg.config);
  fit();
}

function drawLayer(id) {
  const l = scene && scene.get(id);
  return l && l.type === 'draw' ? l : null;
}

link.on((msg) => {
  switch (msg.type) {
    case 'init':
      ready = true;
      applyConfig(msg);
      break;
    case 'config':
      if (ready) applyConfig(msg);
      break;
    case 'frame':
      frame = msg;
      frameAt = performance.now();
      break;
    case 'xfade':
      scene?.crossfade(msg.dur);
      break;
    case 'stroke': {
      const l = drawLayer(msg.layer);
      if (!l) break;
      if (msg.meta) l.beginStroke(msg.meta);
      l.addPoints(msg.id, msg.pts);
      break;
    }
    case 'end':
      drawLayer(msg.layer)?.endStroke(msg.id);
      break;
    case 'clear':
      for (const l of scene?.layers || []) if (l.type === 'draw' && (msg.layer === '*' || msg.layer === l.id)) l.clear();
      break;
    case 'layerState':
      drawLayer(msg.layer)?.importState(msg.state);
      break;
    case 'imageAsset':
      registerAsset(msg.id, msg.dataUrl, msg.name);
      break;
    case 'testpattern':
      grid.style.display = msg.on ? 'block' : 'none';
      break;
    case 'close':
      window.close();
      break;
  }
});

function hello() {
  if (!ready) link.send({ type: 'hello', w: innerWidth, h: innerHeight });
}

function report() {
  link.send({ type: 'alive', w: innerWidth, h: innerHeight, fps: frames });
  frames = 0;
}

addEventListener('resize', () => { fit(); report(); });
addEventListener('pagehide', () => link.send({ type: 'bye' }));
addEventListener('keydown', (e) => { if (e.key === 'Escape') window.close(); });
addEventListener('dblclick', () => {
  if (document.fullscreenElement) document.exitFullscreen();
  else document.documentElement.requestFullscreen?.().catch(() => {});
});

let last = performance.now();
function tick(now) {
  requestAnimationFrame(tick);
  const dt = Math.min(0.1, (now - last) / 1000);
  last = now;
  if (!scene || !frame) return;
  const extra = Math.min(0.1, (now - frameAt) / 1000) * frame.fx.speed;
  scene.restore(frame.layers, extra);
  scene.applyMods(frame.lm || {});
  scene.compose(frame.t + extra, dt);
  pipe.render(scene.canvas, frame.fx, dt, frame.fx.black);
  frames++;
  if (!wait.classList.contains('gone')) wait.classList.add('gone');
}

window.__lumenOut = { get scene() { return scene; }, pipe, get frame() { return frame; } };

fit();
hello();
setInterval(hello, 700);
setInterval(report, 1000);
requestAnimationFrame(tick);
