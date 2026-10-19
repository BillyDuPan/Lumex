const { app, BrowserWindow, screen, ipcMain, powerSaveBlocker, session, systemPreferences, shell } = require('electron');
const { startServer } = require('./server');

let base = '';
let control = null;
let output = null;
let blocker = null;

const webPrefs = { backgroundThrottling: false, contextIsolation: true, sandbox: true };

function displayList() {
  const primary = screen.getPrimaryDisplay().id;
  return screen.getAllDisplays().map((d, i) => ({
    id: d.id,
    label: d.label || `Display ${i + 1}`,
    primary: d.id === primary,
    width: d.size.width,
    height: d.size.height,
  }));
}

function createControl() {
  control = new BrowserWindow({
    width: 1480, height: 920, minWidth: 1000, minHeight: 640,
    title: 'Lumen – Control', backgroundColor: '#0a0b0f',
    webPreferences: { ...webPrefs, autoplayPolicy: 'no-user-gesture-required', preload: require('path').join(__dirname, 'preload.js') },
  });
  control.loadURL(`${base}/control.html`);
  control.on('closed', () => {
    control = null;
    closeOutput();
    app.quit();
  });
}

function closeOutput() {
  if (blocker !== null) { powerSaveBlocker.stop(blocker); blocker = null; }
  if (output && !output.isDestroyed()) output.destroy();
  output = null;
}

function openOutput(displayId) {
  closeOutput();
  const display = displayId != null
    ? screen.getAllDisplays().find((d) => d.id === displayId)
    : null;

  const common = {
    backgroundColor: '#000000', title: 'Lumen – Output', show: false,
    webPreferences: webPrefs,
  };
  if (display) {
    const b = display.bounds;
    output = new BrowserWindow({
      ...common, x: b.x, y: b.y, width: b.width, height: b.height,
      frame: false, hasShadow: false, resizable: false, fullscreenable: false,
    });
    output.setAlwaysOnTop(true, 'screen-saver');
    output.once('ready-to-show', () => {
      output.show();
      output.setSimpleFullScreen(true);
    });
  } else {
    output = new BrowserWindow({ ...common, width: 960, height: 540, useContentSize: true });
    output.once('ready-to-show', () => output.show());
  }

  output.webContents.on('before-input-event', (_e, input) => {
    if (input.type === 'keyDown' && input.key === 'Escape') {
      closeOutput();
      control?.webContents.send('output:closed');
    }
  });
  output.on('closed', () => {
    output = null;
    control?.webContents.send('output:closed');
  });
  output.loadURL(`${base}/output.html`);
  blocker = powerSaveBlocker.start('prevent-display-sleep');
}

const MAC_MIC_SETTINGS = 'x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone';

// Only the control window may listen, and only to audio. Everything else keeps Electron's default (allowed).
function guardPermissions() {
  const ses = session.defaultSession;
  const isControl = (wc) => !!wc && !!control && !control.isDestroyed() && wc === control.webContents;
  ses.setPermissionRequestHandler((wc, permission, done, details) => {
    if (permission !== 'media') return done(true);
    const types = details.mediaTypes || [];
    done(isControl(wc) && types.includes('audio') && !types.includes('video'));
  });
  ses.setPermissionCheckHandler((wc, permission, _origin, details) =>
    permission !== 'media' || (isControl(wc) && details.mediaType !== 'video'));
}

ipcMain.handle('mic:status', () => (process.platform === 'darwin' ? systemPreferences.getMediaAccessStatus('microphone') : 'granted'));
ipcMain.handle('mic:ask', async () => (process.platform === 'darwin' ? systemPreferences.askForMediaAccess('microphone') : true));
ipcMain.handle('mic:settings', () => (process.platform === 'darwin' ? shell.openExternal(MAC_MIC_SETTINGS) : undefined));

ipcMain.handle('displays:list', () => displayList());
ipcMain.handle('output:open', (_e, { displayId }) => openOutput(displayId));
ipcMain.handle('output:close', () => closeOutput());
ipcMain.handle('output:fullscreen', () => {
  if (!output || output.isDestroyed()) return;
  output.setSimpleFullScreen(!output.isSimpleFullScreen());
});

app.whenReady().then(async () => {
  const { port } = await startServer(5177);
  base = `http://127.0.0.1:${port}`;
  guardPermissions();
  createControl();

  const push = () => control?.webContents.send('displays', displayList());
  for (const ev of ['display-added', 'display-removed', 'display-metrics-changed']) screen.on(ev, push);

  app.on('activate', () => { if (!control) createControl(); });
});

app.on('window-all-closed', () => app.quit());
