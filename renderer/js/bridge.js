/**
 * Universal runtime bridge for Lumex:
 * Seamlessly interfaces with Tauri v2 IPC, Electron IPC, or browser fallbacks.
 */

class UniversalBridge {
  constructor() {
    this.displays = [];
    this.displayListeners = new Set();
    this.closeListeners = new Set();
    this.openListeners = new Set();
    this.init();
  }

  async init() {
    if (typeof window !== 'undefined' && window.__TAURI__ && window.__TAURI__.event) {
      try {
        await window.__TAURI__.event.listen('projector:opened', () => {
          this.openListeners.forEach((fn) => {
            try { fn(); } catch (e) { console.error(e); }
          });
        });
        await window.__TAURI__.event.listen('projector:closed', () => {
          this.closeListeners.forEach((fn) => {
            try { fn(); } catch (e) { console.error(e); }
          });
        });
        await window.__TAURI__.event.listen('monitors:changed', async () => {
          const list = await this.listDisplays();
          this.displayListeners.forEach((fn) => {
            try { fn(list); } catch (e) { console.error(e); }
          });
        });
      } catch (err) {
        console.warn('[Bridge] event listen error:', err);
      }
    }
  }

  get isTauri() {
    return typeof window !== 'undefined' && !!(window.__TAURI__ && window.__TAURI__.core);
  }

  async listDisplays() {
    if (this.isTauri) {
      try {
        const monitors = await window.__TAURI__.core.invoke('get_monitors');
        this.displays = monitors.map((m, idx) => ({
          id: m.index !== undefined ? m.index : idx,
          label: m.name ? `${m.name} (${m.width}×${m.height})` : `Display ${idx + 1} (${m.width}×${m.height})`,
          primary: !!m.primary,
          width: m.width,
          height: m.height,
        }));
        return this.displays;
      } catch (err) {
        console.warn('[Bridge] get_monitors failed, using fallback:', err);
      }
    }

    // Web fallback
    const sw = typeof window !== 'undefined' ? window.screen.width : 1920;
    const sh = typeof window !== 'undefined' ? window.screen.height : 1080;
    return [
      { id: 0, label: `Primary Screen (${sw}×${sh})`, primary: true, width: sw, height: sh },
      { id: 'window', label: 'Floating Output Window', primary: false, width: 1280, height: 720 },
    ];
  }

  async openOutput(opts) {
    if (this.isTauri) {
      try {
        const sel = opts?.displayId;
        const monitorIndex = (sel != null && sel !== 'window') ? Number(sel) : null;
        return await window.__TAURI__.core.invoke('open_projector', { monitorIndex });
      } catch (err) {
        console.warn('[Bridge] open_projector failed:', err);
      }
    }

    // Browser popup fallback
    if (typeof window !== 'undefined') {
      window.open('output.html', 'LumexOutput', 'width=1280,height=720,menubar=no,toolbar=no');
    }
  }

  async closeOutput() {
    if (this.isTauri) {
      try {
        return await window.__TAURI__.core.invoke('close_projector');
      } catch (err) {
        console.warn('[Bridge] close_projector failed:', err);
      }
    }
  }

  async toggleFullscreen() {
    if (this.isTauri) {
      try {
        return await window.__TAURI__.core.invoke('toggle_projector_fullscreen');
      } catch (err) {
        console.warn('[Bridge] toggle_projector_fullscreen failed:', err);
      }
    }
  }

  onDisplays(cb) {
    this.displayListeners.add(cb);
    this.listDisplays().then(cb).catch(console.error);
  }

  onOutputOpened(cb) {
    this.openListeners.add(cb);
  }

  onOutputClosed(cb) {
    this.closeListeners.add(cb);
  }

  async micStatus() {
    return 'granted';
  }

  async askMic() {
    return true;
  }

  openMicSettings() {
    console.log('[Bridge] openMicSettings');
  }
}

export const bridge = new UniversalBridge();
if (typeof window !== 'undefined') {
  window.lumen = bridge;
}
