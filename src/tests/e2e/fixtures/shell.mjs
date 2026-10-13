/**
 * A stand-in for nas-os-terminal.socket. Every connection gets a bash of its own, reading commands
 * from it and writing the output back, as systemd does on the NAS. A page on the port says it is up.
 *
 *   node src/tests/e2e/fixtures/shell.mjs <socket> [port]
 */
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
