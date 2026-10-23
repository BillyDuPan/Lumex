/**
 * Control <-> Output bidirectional communication link.
 * Uses BroadcastChannel natively shared between Tauri windows.
 */

export function openLink() {
  const listeners = new Set();
  const clientId = 'c_' + Math.random().toString(36).slice(2, 10);

  function dispatch(msg) {
    if (!msg || typeof msg !== 'object') return;
    if (msg.__sender === clientId) return;

    listeners.forEach((fn) => {
      try {
        fn(msg);
      } catch (e) {
        console.error('[Link] listener error:', e);
      }
    });
  }

  let bc = null;
  try {
    if (typeof BroadcastChannel !== 'undefined') {
      bc = new BroadcastChannel('lumen-link');
      bc.onmessage = (e) => dispatch(e.data);
    }
  } catch (err) {
    console.warn('[Link] BroadcastChannel unavailable:', err);
  }

  return {
    send: (rawMsg) => {
      const msg = { ...rawMsg, __sender: clientId };
      if (bc) {
        try {
          bc.postMessage(msg);
        } catch (e) {
          console.warn('[Link] postMessage error:', e);
        }
      }
    },
    on: (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    close: () => {
      listeners.clear();
      if (bc) {
        try { bc.close(); } catch (_) {}
      }
    }
  };
}
