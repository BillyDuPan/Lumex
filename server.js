// Tiny static file server for the renderer/ folder.
// Used by Electron (main.js) and standalone via `npm run web`.
const http = require('http');
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, 'renderer');
const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.gif': 'image/gif',
  '.ico': 'image/x-icon',
};

function startServer(preferredPort = 5177) {
  const server = http.createServer((req, res) => {
    let rel = decodeURIComponent(new URL(req.url, 'http://x').pathname);
    if (rel === '/') rel = '/control.html';
    const file = path.normalize(path.join(ROOT, rel));
    if (!file.startsWith(ROOT)) {
      res.writeHead(403).end();
      return;
    }
    fs.readFile(file, (err, data) => {
      if (err) {
        res.writeHead(404).end('Not found');
        return;
      }
      res.writeHead(200, {
        'Content-Type': TYPES[path.extname(file)] || 'application/octet-stream',
        'Cache-Control': 'no-store',
      });
      res.end(data);
    });
  });

  return new Promise((resolve, reject) => {
    const listen = (port) => {
      server.once('error', (e) => {
        if (e.code === 'EADDRINUSE' && port !== 0) listen(0);
        else reject(e);
      });
      server.listen(port, '127.0.0.1', () => resolve({ server, port: server.address().port }));
    };
    listen(preferredPort);
  });
}

module.exports = { startServer };

if (require.main === module) {
  startServer(Number(process.env.PORT) || 5177).then(({ port }) => {
    console.log(`Lumen running:`);
    console.log(`  control  http://127.0.0.1:${port}/control.html`);
    console.log(`  output   http://127.0.0.1:${port}/output.html`);
  });
}
