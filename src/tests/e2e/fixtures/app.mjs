/**
 * A stand-in for an installed application: one page, served over a unix socket.
 *
 * It is what an application really is from the launcher's side, a separate process on a socket of
 * its own, so the proxy can be exercised for real, with a browser loading the page from the
 * application's own origin.
 *
 *   node src/tests/e2e/fixtures/app.mjs <socket> [port]
 */
import { createServer } from 'node:http';
import { mkdirSync, rmSync } from 'node:fs';
import { dirname } from 'node:path';

const socket = process.argv[2];
const port = Number(process.argv[3] ?? 3004);

if (!socket) {
  console.error('usage: app.mjs <socket> [port]');
  process.exit(2);
}

const page = `<!doctype html>
<html lang="it"><head><meta charset="utf-8"><title>Esplora file</title></head>
<body><h1 data-testid="app_heading">Esplora file</h1>
<p data-testid="app_user"></p>
<p data-testid="app_theme">-</p>
<p data-testid="app_locale">-</p>
<button type="button" data-testid="app_notify">Avvisa</button>
<script>
  document.querySelector('[data-testid="app_user"]').textContent =
    document.documentElement.dataset.user ?? '';

  var shell = null;
  window.addEventListener('message', function (event) {
    var message = event.data;
    if (!message || message.nas !== 1 || message.type !== 'context') {
      return;
    }
    shell = event.origin;
    document.querySelector('[data-testid="app_theme"]').textContent = message.context.theme;
    document.querySelector('[data-testid="app_locale"]').textContent = message.context.locale;
  });

  document.querySelector('[data-testid="app_notify"]').addEventListener('click', function () {
    if (shell) {
      window.parent.postMessage(
        { nas: 1, type: 'notify', title: 'Copia completata', message: '30 GB', tone: 'success' },
        shell,
      );
    }
  });

  window.parent.postMessage({ nas: 1, type: 'ready' }, '*');
</script>
</body></html>`;

function handler(request, answer) {
  const user = request.headers['x-nas-user'] ?? '';

  if (request.url?.startsWith('/api/search?')) {
    answer.writeHead(200, { 'content-type': 'application/json' });
    answer.end(JSON.stringify([]));
    return;
  }

  if (request.url === '/who') {
    answer.writeHead(200, { 'content-type': 'text/plain; charset=utf-8' });
    answer.end(user);
    return;
  }

  answer.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  answer.end(page.replace('<html lang="it">', `<html lang="it" data-user="${user}">`));
}

mkdirSync(dirname(socket), { recursive: true });
rmSync(socket, { force: true });

createServer(handler).listen(socket, () => {
  console.log(`fixture application on ${socket}`);
});
createServer(handler).listen(port, '127.0.0.1', () => {
  console.log(`readiness probe on http://127.0.0.1:${port}`);
});

for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    rmSync(socket, { force: true });
    process.exit(0);
  });
}
