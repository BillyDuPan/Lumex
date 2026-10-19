const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('lumen', {
  listDisplays: () => ipcRenderer.invoke('displays:list'),
  openOutput: (opts) => ipcRenderer.invoke('output:open', opts),
  closeOutput: () => ipcRenderer.invoke('output:close'),
  toggleFullscreen: () => ipcRenderer.invoke('output:fullscreen'),
  onDisplays: (cb) => ipcRenderer.on('displays', (_e, list) => cb(list)),
  onOutputClosed: (cb) => ipcRenderer.on('output:closed', () => cb()),
  micStatus: () => ipcRenderer.invoke('mic:status'),
  askMic: () => ipcRenderer.invoke('mic:ask'),
  openMicSettings: () => ipcRenderer.invoke('mic:settings'),
});
