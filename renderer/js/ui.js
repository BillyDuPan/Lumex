/* Small DOM helpers for the control surface. No framework. */

const svg = (d, extra = '') =>
  `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" ${extra}>${d}</svg>`;

export const ICONS = {
  eye: svg('<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>'),
  eyeOff: svg('<path d="M9.9 5.2A9.7 9.7 0 0 1 12 5c6.4 0 10 7 10 7a17 17 0 0 1-3.2 4.1M6.6 6.6A17 17 0 0 0 2 12s3.6 7 10 7a9.6 9.6 0 0 0 4.4-1.1"/><path d="M3 3l18 18"/>'),
  up: svg('<path d="M6 15l6-6 6 6"/>'),
  down: svg('<path d="M6 9l6 6 6-6"/>'),
  trash: svg('<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>'),
  pencil: svg('<path d="M4 20l1-4L16.5 4.5a2.1 2.1 0 0 1 3 3L8 19l-4 1z"/>'),
  close: svg('<path d="M6 6l12 12M18 6L6 18"/>'),
  help: svg('<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.4-1 1-1 1.7M12 17h.01"/>'),
  screen: svg('<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>'),
  power: svg('<path d="M12 3v9M6.3 6.8a8 8 0 1 0 11.4 0"/>'),
  wave: svg('<path d="M2 12c2.5-8 5-8 7.5 0s5 8 7.5 0c1.2-3.8 2.8-5.4 5-5.4"/>', 'width="14" height="14"'),
  bars: svg('<path d="M5 20V12M10 20V6M15 20V10M20 20V4"/>', 'width="14" height="14"'),
  play: svg('<polygon points="6 4 20 12 6 20 6 4" fill="currentColor"/>', 'width="14" height="14"'),
  pause: svg('<rect x="6" y="4" width="4" height="16" fill="currentColor"/><rect x="14" y="4" width="4" height="16" fill="currentColor"/>', 'width="14" height="14"'),
  skipNext: svg('<polygon points="5 4 15 12 5 20 5 4" fill="currentColor"/><line x1="19" y1="5" x2="19" y2="19"/>', 'width="14" height="14"'),
  skipPrev: svg('<polygon points="19 20 9 12 19 4 19 20" fill="currentColor"/><line x1="5" y1="19" x2="5" y2="5"/>', 'width="14" height="14"'),
  save: svg('<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/>'),
  copy: svg('<rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>'),
  camera: svg('<path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"/><circle cx="12" cy="13" r="4"/>'),
  shuffle: svg('<polyline points="16 3 21 3 21 8"/><line x1="4" y1="20" x2="21" y2="3"/><polyline points="21 16 21 21 16 21"/><line x1="15" y1="15" x2="21" y2="21"/><line x1="4" y1="4" x2="9" y2="9"/>', 'width="14" height="14"'),
  clock: svg('<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>', 'width="14" height="14"'),
  check: svg('<polyline points="20 6 9 17 4 12"/>', 'width="14" height="14"'),
  download: svg('<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>', 'width="14" height="14"'),
  upload: svg('<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/>', 'width="14" height="14"'),
  chevronRight: svg('<polyline points="9 18 15 12 9 6"/>', 'width="14" height="14"'),
  chevronDown: svg('<polyline points="6 9 12 15 18 9"/>', 'width="14" height="14"'),
  music: svg('<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>', 'width="14" height="14"'),
  grid: svg('<rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/>', 'width="14" height="14"'),
  list: svg('<line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>', 'width="14" height="14"'),
  refresh: svg('<path d="M23 4v6h-6"/><path d="M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>', 'width="14" height="14"'),
};

export function h(tag, attrs = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v == null || v === false) continue;
    if (k === 'class') el.className = v;
    else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k === 'html') el.innerHTML = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  for (const kid of kids.flat()) {
    if (kid == null || kid === false) continue;
    el.append(kid.nodeType ? kid : document.createTextNode(kid));
  }
  return el;
}

const dec = (step) => (step >= 1 ? 0 : Math.min(4, Math.ceil(-Math.log10(step))));

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

/**
 * Toast notifications for feedback (Adobe style).
 */
export function showToast(message, type = 'info', duration = 2400) {
  let container = document.getElementById('toastContainer');
  if (!container) {
    container = h('div', { id: 'toastContainer', class: 'toast-container' });
    document.body.append(container);
  }
  const toast = h('div', { class: `toast toast-${type}` },
    h('span', { class: 'toast-msg' }, message)
  );
  container.append(toast);
  requestAnimationFrame(() => toast.classList.add('show'));
  setTimeout(() => {
    toast.classList.remove('show');
    setTimeout(() => toast.remove(), 250);
  }, duration);
}

/**
 * Rotary knob: drag up/down (hold Shift for fine), scroll, double-click to reset.
 * `log` maps the travel exponentially, handy for rates.
 */
export function knob({ label, min, max, value, def, log = false, fmt, onInput }) {
  const C = 21, R = 16, A0 = 135, SWEEP = 270;
  const toPos = (v) => clamp01(log ? Math.log(v / min) / Math.log(max / min) : (v - min) / (max - min));
  const fromPos = (p) => (log ? min * Math.pow(max / min, p) : min + p * (max - min));
  const pt = (deg) => [C + R * Math.cos((deg * Math.PI) / 180), C + R * Math.sin((deg * Math.PI) / 180)];
  const arc = (p) => {
    const [x0, y0] = pt(A0), [x1, y1] = pt(A0 + SWEEP * p);
    return `M${x0.toFixed(2)} ${y0.toFixed(2)}A${R} ${R} 0 ${SWEEP * p > 180 ? 1 : 0} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
  };
  const NS = 'http://www.w3.org/2000/svg';
  const mk = (tag, attrs) => { const e = document.createElementNS(NS, tag); for (const k in attrs) e.setAttribute(k, attrs[k]); return e; };
  const root = mk('svg', { viewBox: '0 0 42 42', width: 40, height: 40 });
  const track = mk('path', { d: arc(1), class: 'k-track' });
  const fill = mk('path', { class: 'k-fill' });
  const tick = mk('line', { class: 'k-tick', x1: C, y1: C, x2: C, y2: C });
  root.append(track, fill, tick);
  const out = h('output', { class: 'kv' });
  let v = value;
  const draw = () => {
    const p = toPos(v);
    fill.setAttribute('d', p > 0.002 ? arc(p) : '');
    const [x, y] = pt(A0 + SWEEP * p);
    tick.setAttribute('x1', C + (x - C) * 0.35); tick.setAttribute('y1', C + (y - C) * 0.35);
    tick.setAttribute('x2', x); tick.setAttribute('y2', y);
    out.textContent = fmt ? fmt(v) : v.toFixed(2);
  };
  const set = (nv) => { v = nv; draw(); };
  const commit = (p) => { v = fromPos(clamp01(p)); draw(); onInput(v); };
  let drag = null;
  root.addEventListener('pointerdown', (e) => {
    root.setPointerCapture(e.pointerId);
    drag = { y: e.clientY, p: toPos(v) };
    e.preventDefault();
  });
  root.addEventListener('pointermove', (e) => {
    if (drag) commit(drag.p + ((drag.y - e.clientY) / 150) * (e.shiftKey ? 0.2 : 1));
  });
  root.addEventListener('pointerup', () => { drag = null; });
  root.addEventListener('dblclick', () => { if (def !== undefined) { v = def; draw(); onInput(v); } });
  root.addEventListener('wheel', (e) => { e.preventDefault(); commit(toPos(v) - Math.sign(e.deltaY) * 0.03); }, { passive: false });
  draw();
  const el = h('div', { class: 'knob', title: 'Drag up/down · double-click to reset' }, root, out, h('span', { class: 'kl' }, label));
  return { el, set };
}

/**
 * Labelled range input with Adobe style editable number value on click.
 * Double-click the label or track to reset to `def`.
 */
export function slider({ label, min, max, step = 0.01, value, def, fmt, unit = '', tip, onInput, mod }) {
  const out = h('output', { class: 'val', title: 'Click to type exact number · Double-click label to reset' });
  const input = h('input', { type: 'range', min, max, step });
  const show = (v) => {
    out.textContent = fmt ? fmt(v) : Number(v).toFixed(dec(step)) + (unit ? ' ' + unit : '');
    input.style.setProperty('--p', ((v - min) / (max - min)) * 100 + '%');
  };
  const set = (v) => { input.value = v; show(Number(v)); };
  input.addEventListener('input', () => {
    const v = Number(input.value);
    show(v);
    onInput(v);
  });
  const reset = () => {
    if (def === undefined) return;
    set(def);
    onInput(def);
  };
  input.addEventListener('dblclick', reset);

  // Adobe-style direct numeric input on clicking value
  out.addEventListener('click', (e) => {
    if (out.querySelector('input')) return;
    const curVal = Number(input.value);
    const numInput = h('input', {
      type: 'number',
      min, max, step,
      value: curVal,
      class: 'val-input'
    });
    out.textContent = '';
    out.append(numInput);
    numInput.focus();
    numInput.select();
    const commitVal = () => {
      let nv = parseFloat(numInput.value);
      if (isNaN(nv)) nv = curVal;
      nv = Math.max(min, Math.min(max, nv));
      set(nv);
      onInput(nv);
    };
    numInput.addEventListener('keydown', (ke) => {
      ke.stopPropagation();
      if (ke.key === 'Enter') commitVal();
      else if (ke.key === 'Escape') show(curVal);
    });
    numInput.addEventListener('blur', commitVal);
  });

  const track = h('span', { class: 'track' }, input);
  const row = h('div', { class: 'row slider' },
    h('span', { class: 'lab', ondblclick: reset, title: tip || 'Double-click to reset' }, label), track, out);
  const el = h('div', { class: 'slwrap' }, row);

  if (mod) {
    const band = h('i', { class: 'band', hidden: true });
    const mark = h('i', { class: 'mark', hidden: true });
    track.append(band, mark);
    const drawer = h('div', { class: 'drawer', hidden: true });
    const entry = () => mod.store[mod.key];
    const btn = h('button', { type: 'button', class: 'modbtn', title: 'Modulate: let this control swing (LFO) or follow the music', html: ICONS.wave });
    const sync = () => {
      const e = entry(), on = !!(e && e.on), aud = on && mod.isAudio(e);
      btn.classList.toggle('on', on);
      el.classList.toggle('aud', aud);
      btn.innerHTML = aud ? ICONS.bars : ICONS.wave;
      drawer.hidden = !on;
      if (!on) { mark.hidden = true; band.hidden = true; }
    };
    const build = () => {
      const e = entry();
      const pick = segmented({
        options: mod.sources, value: e.src || 'lfo',
        onChange: (s) => { e.src = s; e.ph = 0; mod.onChange(); build(); sync(); },
      }).el;
      const depth = knob({ label: 'Intensity', min: 0, max: 1, value: e.depth, def: 0.3, fmt: (d) => Math.round(d * 100) + '%', onInput: (d) => { e.depth = d; mod.onChange(); } }).el;
      if (mod.isAudio(e)) {
        const duck = h('button', {
          type: 'button', class: 'duck' + (e.inv ? ' on' : ''), html: '↓ Duck', title: 'Pull the slider down with the music instead of pushing it up',
          onclick: () => { e.inv = !e.inv; duck.classList.toggle('on', e.inv); mod.onChange(); },
        });
        drawer.replaceChildren(pick, h('div', { class: 'knobs' }, depth,
          knob({ label: 'Smooth', min: 0, max: 1, value: e.smooth ?? 0.35, def: 0.35, fmt: mod.smoothFmt, onInput: (v) => { e.smooth = v; mod.onChange(); } }).el, duck));
      } else {
        drawer.replaceChildren(pick,
          segmented({ options: mod.shapes, value: e.shape, onChange: (s) => { e.shape = s; mod.onChange(); } }).el,
          h('div', { class: 'knobs' },
            knob({ label: 'Speed', min: mod.rateRange[0], max: mod.rateRange[1], log: true, value: e.rate, def: 0.15, fmt: (r) => (r >= 1 ? r.toFixed(1) : r.toFixed(2)) + ' Hz', onInput: (r) => { e.rate = r; mod.onChange(); } }).el,
            depth));
      }
    };
    btn.addEventListener('click', () => {
      let e = entry();
      if (!e) { e = mod.store[mod.key] = mod.newEntry(); build(); } else { e.on = !e.on; if (e.on) build(); }
      sync();
      mod.onChange();
    });
    row.append(btn);
    el.append(drawer);
    if (entry()?.on) build();
    sync();
    mod.register({ mark, band, input, min, max, r: [min, max], span: [0, 0], entry, live: mod.live });
  } else {
    row.append(h('span', { class: 'modbtn slot' }));
  }
  set(value);
  return { el, set, input };
}

/** Moves a slider's live marker and swing band to where the modulation currently is. */
export function updateMarker(m, reach) {
  const e = m.entry();
  const visible = !!(e && e.on) && m.input.isConnected;
  m.mark.hidden = m.band.hidden = !visible;
  if (!visible) return;
  const span = m.max - m.min, base = Number(m.input.value);
  const at = (v) => clamp01((v - m.min) / span);
  const r = reach(e, m.r, m.span);
  const lo = at(base + r[0]), hi = at(base + r[1]);
  m.mark.style.left = `calc(7px + (100% - 14px) * ${at(m.live())})`;
  m.band.style.left = `calc(7px + (100% - 14px) * ${lo})`;
  m.band.style.width = `calc((100% - 14px) * ${hi - lo})`;
}

export function toggle({ label, value, onChange }) {
  const input = h('input', { type: 'checkbox' });
  input.checked = !!value;
  input.addEventListener('change', () => onChange(input.checked));
  const el = h('label', { class: 'row toggle' }, h('span', { class: 'lab' }, label), input, h('span', { class: 'switch' }));
  return { el, set: (v) => { input.checked = !!v; } };
}

export function select({ label, options, value, onChange }) {
  const sel = h('select', {}, options.map(([v, t]) => h('option', { value: v }, t)));
  sel.value = value;
  sel.addEventListener('change', () => onChange(sel.value));
  const el = h('label', { class: 'row select' }, label ? h('span', { class: 'lab' }, label) : null, sel);
  return { el, sel, set: (v) => { sel.value = v; } };
}

export function segmented({ label, options, value, onChange, cls = '' }) {
  const btns = options.map(([v, t, title]) =>
    h('button', { type: 'button', 'data-v': v, title, class: v === value ? 'on' : '', onclick: () => set(v, true) }, t));
  const wrap = h('div', { class: 'seg ' + cls }, btns);
  function set(v, fire) {
    for (const b of btns) b.classList.toggle('on', b.dataset.v === v);
    if (fire) onChange(v);
  }
  const el = label ? h('div', { class: 'row seg-row' + (options.length > 3 ? ' stack' : '') }, h('span', { class: 'lab' }, label), wrap) : wrap;
  return { el, set };
}

export function button(text, onclick, cls = '', title) {
  return h('button', { type: 'button', class: 'btn ' + cls, onclick, title }, text);
}

export function section(title, ...body) {
  const head = title ? h('div', { class: 'group-head' }, h('h3', {}, title)) : null;
  return h('section', { class: 'group' }, head, ...body);
}
