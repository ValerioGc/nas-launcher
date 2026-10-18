import { spawn } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import { createServer as createHttpServer } from 'node:http';
import { createServer } from 'node:net';
import { dirname } from 'node:path';

const socket = process.argv[2];
const port = Number(process.argv[3] ?? 3005);

if (!socket) {
  console.error('usage: shell.mjs <socket> [port]');
  process.exit(2);
}

mkdirSync(dirname(socket), { recursive: true });
rmSync(socket, { force: true });

createServer({ pauseOnConnect: true }, (connection) => {
  const shell = spawn('bash', ['--noprofile', '--norc', '-s'], {
    cwd: '/',
    stdio: [connection, connection, connection],
  });
  shell.on('exit', () => connection.destroy());
}).listen(socket);

createHttpServer((_request, response) => response.end('ok')).listen(port, '127.0.0.1');
