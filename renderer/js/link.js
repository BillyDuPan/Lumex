/** Control <-> output messaging. Both windows share one origin, so BroadcastChannel is enough. */
export function openLink() {
  const bc = new BroadcastChannel('lumen-link');
  return {
    send: (msg) => bc.postMessage(msg),
    on: (fn) => { bc.onmessage = (e) => fn(e.data); },
  };
}
