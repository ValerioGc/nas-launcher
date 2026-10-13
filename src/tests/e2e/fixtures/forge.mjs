/**
 * A forge that answers like GitHub does, on loopback.
 *
 * The package machinery reaches the internet, which is exactly what a test must not do. Pointing
 * `NAS_GITHUB_API` here makes the whole path real without a packet leaving the machine.
 *
 *   node src/tests/e2e/fixtures/forge.mjs [port]
 */
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const port = Number(process.argv[2] ?? 3002);
const version = '1.2.0';
const asset = `file-explorer-${version}-linux-x64.tar.gz`;

const staging = mkdtempSync(join(tmpdir(), 'nas-os-forge-'));
const archive = join(staging, asset);
execFileSync('tar', [
  '--create',
  '--gzip',
  '--file',
  archive,
  '--directory',
  join(here, 'package'),
  '.',
]);

const body = readFileSync(archive);
const checksum = `${createHash('sha256').update(body).digest('hex')}  ${asset}\n`;

const release = {
  tag_name: `v${version}`,
  assets: [
    { name: asset, url: `http://127.0.0.1:${port}/assets/${asset}` },
    { name: `${asset}.sha256`, url: `http://127.0.0.1:${port}/assets/${asset}.sha256` },
  ],
};

createServer((request, answer) => {
  const { url } = request;

  if (url?.endsWith('/releases/latest')) {
    answer.writeHead(200, { 'content-type': 'application/json' });
    answer.end(JSON.stringify(release));
    return;
  }

  if (url === `/assets/${asset}`) {
    answer.writeHead(200, { 'content-type': 'application/octet-stream' });
    answer.end(body);
    return;
  }

  if (url === `/assets/${asset}.sha256`) {
    answer.writeHead(200, { 'content-type': 'text/plain' });
    answer.end(checksum);
    return;
  }

  answer.writeHead(404, { 'content-type': 'application/json' });
  answer.end(JSON.stringify({ message: 'Not Found' }));
}).listen(port, '127.0.0.1', () => {
  console.log(`fixture forge on http://127.0.0.1:${port}, serving ${asset}`);
});
