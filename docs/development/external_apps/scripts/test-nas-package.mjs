#!/usr/bin/env node
// Standalone NAS package checker. Node.js 22.18+, built-in modules only.
// Full checks execute the packaged application the way the NAS does: on a unix socket handed to
// it with --listen, never on a port; --archive-only never executes it.
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import {
  chmodSync,
  closeSync,
  existsSync,
  linkSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readdirSync,
  readlinkSync,
  realpathSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { request as httpRequest } from 'node:http';
import { tmpdir } from 'node:os';
import { basename, dirname, join, posix, relative, resolve, sep } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { parseArgs } from 'node:util';
import { gunzipSync } from 'node:zlib';

function check(condition, message) {
  if (!condition) throw new Error(message);
}

function packagePath(value) {
  check(
    typeof value === 'string' && value && !/[\\\0]/.test(value),
    `Invalid package path: ${value}`,
  );
  check(
    !posix.isAbsolute(value) && !value.split('/').includes('..'),
    `Unsafe package path: ${value}`,
  );
  return posix.normalize(value).replace(/\/$/, '') || '.';
}

function packagedFile(root, name) {
  const file = join(root, packagePath(name));
  check(existsSync(file) && statSync(file).isFile(), `Missing packaged file: ${name}`);
  const location = relative(root, realpathSync(file));
  check(location !== '..' && !location.startsWith(`..${sep}`), `Path escapes package: ${name}`);
  return file;
}

function octal(buffer) {
  const text = buffer.toString('ascii').replace(/\0.*$/, '').trim();
  check(/^[0-7]*$/.test(text), 'Unsupported or invalid tar numeric field');
  const number = text ? Number.parseInt(text, 8) : 0;
  check(Number.isSafeInteger(number), 'Tar numeric field is too large');
  return number;
}

function tarString(buffer) {
  return buffer.toString('utf8').replace(/\0.*$/, '');
}

function paxFields(body) {
  const fields = {};
  let offset = 0;
  while (offset < body.length) {
    const space = body.indexOf(32, offset);
    check(space > offset, 'Invalid PAX record');
    const lengthText = body.subarray(offset, space).toString('ascii');
    check(/^[0-9]+$/.test(lengthText), 'Invalid PAX length');
    const length = Number(lengthText);
    check(
      Number.isSafeInteger(length) &&
        length > space - offset + 2 &&
        offset + length <= body.length &&
        body[offset + length - 1] === 10,
      'Invalid PAX length',
    );
    const record = body.subarray(space + 1, offset + length - 1).toString('utf8');
    const equals = record.indexOf('=');
    check(equals > 0, 'Invalid PAX field');
    const key = record.slice(0, equals);
    check(!key.startsWith('GNU.sparse'), 'Sparse tar entries are not supported');
    fields[key] = record.slice(equals + 1);
    offset += length;
  }
  return fields;
}

// Read USTAR/PAX and GNU long-name records before writing anything. Extraction is
// deliberately implemented with filesystem calls, never a shell or a tar subprocess.
function tarEntries(data) {
  const entries = new Map();
  let globalFields = {};
  let nextFields = {};
  let offset = 0;
  let terminated = false;
  while (offset + 512 <= data.length) {
    const header = data.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) {
      check(
        offset + 1024 <= data.length && data.subarray(offset).every((byte) => byte === 0),
        'Invalid tar end marker or trailing data',
      );
      terminated = true;
      break;
    }
    const sum = header.reduce(
      (total, byte, index) => total + (index >= 148 && index < 156 ? 32 : byte),
      0,
    );
    check(sum === octal(header.subarray(148, 156)), 'Invalid tar header checksum');
    const type = String.fromCharCode(header[156] || 48);
    const extension = ['x', 'g', 'L', 'K'].includes(type);
    const fields = { ...globalFields, ...nextFields };
    const size =
      !extension && fields.size !== undefined
        ? Number(fields.size)
        : octal(header.subarray(124, 136));
    check(
      Number.isSafeInteger(size) && size >= 0 && offset + 512 + size <= data.length,
      'Invalid or truncated tar entry',
    );
    const body = data.subarray(offset + 512, offset + 512 + size);
    offset += 512 + Math.ceil(size / 512) * 512;
    if (type === 'x' || type === 'g') {
      if (type === 'g') globalFields = { ...globalFields, ...paxFields(body) };
      else nextFields = { ...nextFields, ...paxFields(body) };
      continue;
    }
    if (type === 'L' || type === 'K') {
      nextFields[type === 'L' ? 'path' : 'linkpath'] = tarString(body);
      continue;
    }
    const prefix =
      header.subarray(257, 263).toString() === 'ustar\0'
        ? tarString(header.subarray(345, 500))
        : '';
    const name = packagePath(
      fields.path ?? `${prefix ? `${prefix}/` : ''}${tarString(header.subarray(0, 100))}`,
    );
    const mode = octal(header.subarray(100, 108));
    check(['0', '1', '2', '5'].includes(type), `Unsupported archive entry: ${name}`);
    check(!(mode & 0o6000), `Privileged file mode: ${name}`);
    check(!entries.has(name), `Duplicate archive path: ${name}`);
    check(name !== '.' || type === '5', 'Archive root must be a directory');
    entries.set(name, {
      name,
      type,
      mode,
      body,
      target: fields.linkpath ?? tarString(header.subarray(157, 257)),
    });
    nextFields = {};
  }
  check(terminated && Object.keys(nextFields).length === 0, 'Truncated tar archive');
  return entries;
}

function extract(entries, root) {
  // No entry may be written underneath a file or link, regardless of archive order.
  for (const { name } of entries.values()) {
    for (let parent = posix.dirname(name); parent !== '.'; parent = posix.dirname(parent)) {
      check(
        !entries.has(parent) || entries.get(parent).type === '5',
        `Non-directory archive parent: ${parent}`,
      );
    }
  }
  function resolveLink(name, depth = 0) {
    check(depth < 64, 'Cyclic or excessively nested archive link');
    const resolved = [];
    const pending = name.split('/');
    while (pending.length) {
      const part = pending.shift();
      if (!part || part === '.') continue;
      if (part === '..') {
        check(resolved.length, 'Archive link escapes package');
        resolved.pop();
        continue;
      }
      resolved.push(part);
      const entry = entries.get(resolved.join('/'));
      if (entry && ['1', '2'].includes(entry.type)) {
        check(
          entry.target && !/[\\\0]/.test(entry.target) && !posix.isAbsolute(entry.target),
          `Unsafe archive link: ${entry.name}`,
        );
        const base = entry.type === '2' ? resolved.slice(0, -1) : [];
        return resolveLink([...base, entry.target, ...pending].join('/'), depth + 1);
      }
    }
    return resolved.join('/') || '.';
  }
  const links = new Map();
  for (const entry of entries.values()) {
    if (['1', '2'].includes(entry.type)) {
      const target = resolveLink(entry.name);
      if (entry.type === '1')
        check(entries.get(target)?.type === '0', `Invalid hardlink target: ${entry.name}`);
      links.set(entry.name, target);
    }
  }
  for (const entry of entries.values()) {
    const destination = join(root, entry.name);
    if (entry.type === '5') mkdirSync(destination, { recursive: true });
    if (entry.type === '0') {
      mkdirSync(dirname(destination), { recursive: true });
      writeFileSync(destination, entry.body, { flag: 'wx', mode: (entry.mode & 0o755) | 0o600 });
      chmodSync(destination, (entry.mode & 0o755) | 0o600);
    }
  }
  for (const entry of entries.values()) {
    if (!links.has(entry.name)) continue;
    const destination = join(root, entry.name);
    mkdirSync(dirname(destination), { recursive: true });
    if (entry.type === '1') linkSync(join(root, links.get(entry.name)), destination);
    else symlinkSync(entry.target, destination);
  }
}

function validateManifest(root, args) {
  check(existsSync(join(root, 'nas-app.json')), 'nas-app.json missing at archive root');
  const manifest = JSON.parse(readFileSync(packagedFile(root, 'nas-app.json'), 'utf8'));
  const object = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);
  check(object(manifest), 'Manifest must be a JSON object');
  // app and run are what the NAS cannot do without: the identity, and the program to execute.
  for (const field of ['app', 'run']) check(object(manifest[field]), `${field} must be an object`);
  // The rest is optional, exactly as the launcher reads it, and checked when it is declared.
  for (const field of ['ui', 'permissions'])
    check(manifest[field] === undefined || object(manifest[field]), `${field} must be an object`);
  check(manifest.manifest_version === 1, 'Expected manifest_version 1');
  const { id, name, version, icon } = manifest.app;
  check(typeof id === 'string' && /^[A-Za-z0-9_-]{1,64}$/.test(id), 'Invalid app.id');
  check(typeof name === 'string' && name.trim(), 'Missing app.name');
  check(
    typeof version === 'string' && /^[0-9]+(?:\.[0-9]+)*(?:-[0-9A-Za-z.-]+)?$/.test(version),
    'Invalid app.version',
  );
  if (args['expected-id']) check(id === args['expected-id'], 'app.id does not match --expected-id');
  if (args['expected-version'])
    check(
      version === args['expected-version'].replace(/^v/, ''),
      'app.version does not match --expected-version',
    );
  const escaped = version.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  check(
    new RegExp(`(?<![0-9A-Za-z])${escaped}(?![0-9A-Za-z]|\\.[0-9])`).test(basename(args.archive)),
    'Archive name must carry the manifest version',
  );
  if (args['asset-template'])
    // Only {version}: the catalogue of the NAS substitutes that and nothing else, so a template
    // carrying anything more would pass here and fail the real installation.
    check(
      basename(args.archive) === args['asset-template'].replaceAll('{version}', version),
      'Archive name does not match --asset-template',
    );
  if (icon !== undefined) {
    check(typeof icon === 'string' && icon, 'app.icon must be a packaged path');
    packagedFile(root, icon);
  }
  const executable = packagedFile(root, manifest.run.exec);
  check(statSync(executable).mode & 0o111, 'run.exec is not executable');
  // Declaring it is optional (the launcher derives the path from the id whatever the file says)
  // but declaring a different one is a lie about where the application will be reached.
  check(
    manifest.run.socket === undefined || manifest.run.socket === `/run/nas-os/apps/${id}.sock`,
    'run.socket must be /run/nas-os/apps/<id>.sock, or absent',
  );
  check(
    manifest.ui?.protocol === undefined || manifest.ui.protocol === 1,
    'Expected ui.protocol 1',
  );
  check(
    manifest.permissions?.paths === undefined ||
      (Array.isArray(manifest.permissions.paths) &&
        manifest.permissions.paths.every(
          (path) =>
            typeof path === 'string' && posix.isAbsolute(path) && !path.split('/').includes('..'),
        )),
    "permissions.paths must contain absolute paths without '..'",
  );
  check(
    manifest.provides === undefined || Array.isArray(manifest.provides),
    'provides must be an array',
  );
  for (const capability of manifest.provides ?? []) {
    check(
      object(capability) && typeof capability.capability === 'string' && capability.capability,
      'Missing capability name',
    );
    check(
      Number.isInteger(capability.version) && capability.version > 0,
      'Invalid capability version',
    );
    check(
      typeof capability.endpoint === 'string' &&
        capability.endpoint.length <= 256 &&
        /^\/[A-Za-z0-9/_.-]*$/.test(capability.endpoint) &&
        !capability.endpoint.startsWith('//') &&
        !capability.endpoint.includes('..') &&
        capability.endpoint !== '/internal' &&
        !capability.endpoint.startsWith('/internal/'),
      'Capability endpoints must be public absolute paths',
    );
  }
  return { manifest, executable };
}

function unpack(args, root) {
  const checksum = readFileSync(args.checksum ?? `${args.archive}.sha256`, 'utf8');
  const match = /^([0-9a-fA-F]{64}) [ *]([^\r\n]+)\r?\n?$/.exec(checksum);
  check(
    match && match[2] === basename(args.archive),
    'Checksum must use sha256sum format and name this archive',
  );
  const limit = args['max-unpacked-mib'] * 1024 * 1024;
  check(statSync(args.archive).size <= limit, 'Archive exceeds --max-unpacked-mib');
  const compressed = readFileSync(args.archive);
  check(
    createHash('sha256').update(compressed).digest('hex') === match[1].toLowerCase(),
    'SHA-256 checksum mismatch',
  );
  const data =
    compressed[0] === 0x1f && compressed[1] === 0x8b
      ? gunzipSync(compressed, { maxOutputLength: limit })
      : compressed;
  extract(tarEntries(data), root);
  return validateManifest(root, args);
}

// What the launcher will connect to is a unix socket named after the application; a TCP port is
// a door around the launcher, so finding one open is a failure and not a detail.
function checkListener(pid, socket) {
  // The whole process group, not just the child: an application is free to hold its socket in a
  // worker, and a port opened by one would be just as open.
  const group = readdirSync('/proc').filter((entry) => {
    if (!/^\d+$/.test(entry)) return false;
    try {
      // The fields after the command, which is in parentheses and may contain spaces itself.
      const stat = readFileSync(`/proc/${entry}/stat`, 'utf8');
      return Number(stat.slice(stat.lastIndexOf(')') + 2).split(' ')[2]) === pid;
    } catch (error) {
      if (error.code === 'ENOENT' || error.code === 'ESRCH') return false;
      throw error;
    }
  });
  check(group.length, 'The application is no longer running');
  const inodes = new Set();
  for (const member of group) {
    let descriptors;
    try {
      descriptors = readdirSync(`/proc/${member}/fd`);
    } catch (error) {
      if (error.code === 'ENOENT' || error.code === 'EACCES') continue;
      throw error;
    }
    for (const descriptor of descriptors) {
      try {
        const match = /^socket:\[(\d+)\]$/.exec(readlinkSync(`/proc/${member}/fd/${descriptor}`));
        if (match) inodes.add(match[1]);
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
      }
    }
  }
  for (const table of ['tcp', 'tcp6']) {
    for (const line of readFileSync(`/proc/${pid}/net/${table}`, 'utf8')
      .trim()
      .split('\n')
      .slice(1)) {
      const fields = line.trim().split(/\s+/);
      check(
        fields[3] !== '0A' || !inodes.has(fields[9]),
        'Application opens a TCP listener: it must serve its unix socket only',
      );
    }
  }
  // Num RefCount Protocol Flags Type St Inode Path, and 00010000 is the listening flag.
  const wanted = realpathSync(dirname(socket)) + sep + basename(socket);
  const listening = readFileSync(`/proc/${pid}/net/unix`, 'utf8')
    .trim()
    .split('\n')
    .slice(1)
    .map((line) => line.trim().split(/\s+/))
    .some(
      (fields) =>
        fields[3] === '00010000' && inodes.has(fields[6]) && fields.slice(7).join(' ') === wanted,
    );
  check(listening, 'The unix socket is not listened on by the declared executable');
}

function htmlAssets(html) {
  const urls = new Set();
  const decode = (value) =>
    value.replace(/&(?:#(x[0-9a-f]+|[0-9]+)|amp|quot|apos|lt|gt);/gi, (all, code) => {
      if (code)
        return String.fromCodePoint(
          code[0].toLowerCase() === 'x' ? Number.parseInt(code.slice(1), 16) : Number(code),
        );
      return { '&amp;': '&', '&quot;': '"', '&apos;': "'", '&lt;': '<', '&gt;': '>' }[
        all.toLowerCase()
      ];
    });
  // Ignore comments and script contents, which can contain strings resembling tags.
  const markup = html
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/(<script\b(?:[^>"']|"[^"]*"|'[^']*')*>)[\s\S]*?<\/script\s*>/gi, '$1</script>');
  for (const match of markup.matchAll(
    /<(script|img|source|link)\b((?:[^>"']|"[^"]*"|'[^']*')*)>/gi,
  )) {
    const attributes = Object.fromEntries(
      [...match[2].matchAll(/([^\s=]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/g)].map(
        (attribute) => [
          attribute[1].toLowerCase(),
          decode(attribute[2] ?? attribute[3] ?? attribute[4]),
        ],
      ),
    );
    if (match[1].toLowerCase() !== 'link' && attributes.src) urls.add(attributes.src);
    if (
      match[1].toLowerCase() === 'link' &&
      attributes.href &&
      (attributes.rel ?? '')
        .toLowerCase()
        .split(/\s+/)
        .some((rel) => ['stylesheet', 'modulepreload', 'preload', 'icon'].includes(rel))
    )
      urls.add(attributes.href);
  }
  return urls;
}

// One HTTP request over the unix socket. `fetch` does not speak unix, and the application is not
// reachable any other way: which is the whole point of the contract.
function fetchOverSocket(socket, path, timeout) {
  return new Promise((accept, reject) => {
    const call = httpRequest({ socketPath: socket, path, method: 'GET', timeout }, (response) => {
      const chunks = [];
      response.on('data', (chunk) => chunks.push(chunk));
      response.on('error', reject);
      response.on('end', () =>
        accept({
          status: response.statusCode,
          mime: response.headers['content-type']?.split(';')[0],
          body: Buffer.concat(chunks),
        }),
      );
    });
    call.once('timeout', () => call.destroy(new Error('the application did not answer in time')));
    call.once('error', reject);
    call.end();
  });
}

async function smokeTest(args, manifest, executable) {
  const cwd = mkdtempSync(join(tmpdir(), 'nas-cwd-'));
  const socket = join(cwd, `${manifest.app.id}.sock`);
  // A unix socket path is a fixed field in the kernel: on the NAS it is nowhere near the limit,
  // in a temporary directory it can be, and the error the kernel gives back says only EINVAL.
  check(Buffer.byteLength(socket) <= 107, `Socket path is too long for a unix socket: ${socket}`);
  async function get(path) {
    check(path.startsWith('/'), `Expected an origin-relative URL: ${path}`);
    const response = await fetchOverSocket(socket, path, 2000);
    check(response.status === 200, `${path}: expected HTTP 200, received ${response.status}`);
    return response;
  }
  const environment = { ...process.env, NAS_APP_ID: manifest.app.id, NAS_APP_SOCKET: socket };
  for (const item of args.env) {
    const equals = item.indexOf('=');
    check(equals > 0, 'Invalid --env');
    environment[item.slice(0, equals)] = item.slice(equals + 1);
  }
  const logPath = join(cwd, 'application.log');
  const log = openSync(logPath, 'w');
  // Exactly how the NAS starts it: run-app.sh execs the program with the socket to listen on.
  const child = spawn(executable, ['--listen', socket], {
    cwd,
    env: environment,
    detached: true,
    stdio: ['ignore', log, log],
  });
  let stopped = false;
  let spawnError;
  const exit = new Promise((accept) => {
    child.once('error', (error) => {
      spawnError = error;
      stopped = true;
      accept({ error });
    });
    child.once('exit', (code, signal) => {
      stopped = true;
      accept({ code, signal });
    });
  });
  const cancel = () => child.kill('SIGTERM');
  process.once('SIGINT', cancel);
  process.once('SIGTERM', cancel);
  try {
    const deadline = Date.now() + args.timeout * 1000;
    let ui;
    let lastError;
    while (Date.now() < deadline) {
      check(
        !stopped,
        `Application exited: ${spawnError?.message ?? child.exitCode ?? child.signalCode}`,
      );
      try {
        ui = await get(args['ui-path']);
        break;
      } catch (error) {
        lastError = error.message;
        await delay(100);
      }
    }
    check(ui, `UI did not become ready: ${lastError}`);
    check(ui.mime === 'text/html' && ui.body.toString().trim(), 'UI must serve nonempty HTML');
    checkListener(child.pid, socket);
    let checkedAssets = 0;
    for (const asset of htmlAssets(ui.body.toString())) {
      // The application is served at the root of its own origin: an absolute URL somewhere else
      // is somebody else's business, a relative one has to be in the package.
      const url = new URL(asset, new URL(args['ui-path'], 'http://application.invalid'));
      if (url.origin !== 'http://application.invalid') continue;
      const result = await get(url.pathname + url.search);
      check(
        result.body.length && result.mime !== 'text/html',
        `Missing or HTML-fallback asset: ${url.pathname}`,
      );
      checkedAssets++;
    }
    for (const path of args['api-path']) {
      const result = await get(path);
      check(result.mime === 'application/json', `${path}: expected JSON`);
      const value = JSON.parse(result.body.toString());
      if (args['version-field'])
        check(
          value?.[args['version-field']] === manifest.app.version,
          `${path}: runtime version differs from manifest`,
        );
    }
    check(!stopped, 'Application stopped during smoke test');
    child.kill('SIGTERM');
    const result = await Promise.race([exit, delay(5000, null, { ref: false })]);
    check(result, 'Application did not stop within 5 seconds of SIGTERM');
    check(
      result.code === 0 || result.signal === 'SIGTERM',
      `Unexpected shutdown exit: ${result.code ?? result.signal}`,
    );
    console.log(
      `PASS runtime: unix socket, UI, ${checkedAssets} local assets, ${args['api-path'].length} API endpoint(s), no TCP listener, SIGTERM shutdown`,
    );
  } catch (error) {
    console.error(readFileSync(logPath, 'utf8').slice(-8000));
    throw error;
  } finally {
    process.removeListener('SIGINT', cancel);
    process.removeListener('SIGTERM', cancel);
    try {
      if (child.pid) process.kill(-child.pid, 'SIGKILL');
    } catch (error) {
      if (error.code !== 'ESRCH') {
        console.error(`FAIL: unable to clean up process group: ${error.message}`);
        process.exitCode = 1;
        child.kill('SIGKILL');
      }
    }
    await exit;
    closeSync(log);
    rmSync(cwd, { recursive: true, force: true });
  }
}

const help = `Usage: node test-nas-package.mjs ARCHIVE [options]
Standalone checker (Node.js 22.18+). Full checks execute the packaged app on Linux the way the
NAS does: <run.exec> --listen <socket>, answering HTTP on that unix socket and on nothing else.
  --archive-only           Validate without executing the app
  --checksum FILE          Default: <archive>.sha256
  --expected-id ID         Expected manifest ID
  --expected-version TAG   Expected version (optional v prefix)
  --asset-template NAME    Catalogue filename template, with {version} as its only placeholder
  --ui-path PATH           HTML endpoint (default /)
  --api-path PATH          JSON GET endpoint; repeatable (default /health)
  --version-field FIELD    Check this field on each JSON response
  --env KEY=VALUE          Extra environment variable; repeatable
  --timeout SECONDS        Startup timeout (default 15)
  --max-unpacked-mib SIZE  Archive memory limit (default 512 MiB)
  --help                  Show this help
`;

async function main() {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      help: { type: 'boolean' },
      'archive-only': { type: 'boolean' },
      checksum: { type: 'string' },
      'expected-id': { type: 'string' },
      'expected-version': { type: 'string' },
      'asset-template': { type: 'string' },
      'ui-path': { type: 'string', default: '/' },
      'api-path': { type: 'string', multiple: true },
      'version-field': { type: 'string' },
      env: { type: 'string', multiple: true, default: [] },
      timeout: { type: 'string', default: '15' },
      'max-unpacked-mib': { type: 'string', default: '512' },
    },
  });
  if (values.help) {
    console.log(help);
    return;
  }
  check(positionals.length === 1, help);
  const args = {
    ...values,
    archive: resolve(positionals[0]),
    timeout: Number(values.timeout),
    'max-unpacked-mib': Number(values['max-unpacked-mib']),
    'api-path': values['api-path'] ?? ['/health'],
  };
  check(Number.isFinite(args.timeout) && args.timeout > 0, 'Invalid timeout');
  check(
    Number.isSafeInteger(args['max-unpacked-mib']) && args['max-unpacked-mib'] > 0,
    'Invalid size limit',
  );
  const root = mkdtempSync(join(tmpdir(), 'nas-install-'));
  try {
    const { manifest, executable } = unpack(args, root);
    console.log(
      `PASS package: ${manifest.app.id} ${manifest.app.version}, checksum, paths, manifest, executable and icon`,
    );
    if (!args['archive-only']) {
      check(
        process.platform === 'linux',
        'Runtime check requires Linux; use --archive-only elsewhere',
      );
      await smokeTest(args, manifest, executable);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
});
