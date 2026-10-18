#!/usr/bin/env node
// Assemble an already-built NAS release: one tar.gz plus its .sha256, laid out the way the
// launcher unpacks it. Requires Node and GNU tar 1.29 or newer.
//
// Nothing here is specific to one project: the identity comes from nas-app.json and the binary
// from Cargo, so an application adapts it with options instead of by editing the code. In a build
// pipeline, after building the interface and the release binary:
//
//   node scripts/package-nas.mjs --version "$GITHUB_REF_NAME"
//   node scripts/test-nas-package.mjs release-assets/*.tar.gz --expected-version "$GITHUB_REF_NAME"
//
// The archive is reproducible: the same tree produces the same bytes, whatever the clock says.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, posix, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const help = `Usage: node scripts/package-nas.mjs [options]
  --output DIRECTORY   Where to write the asset (default release-assets)
  --version TAG        Release tag to check against the manifest (optional v prefix)
  --manifest FILE      Application manifest (default nas-app.json)
  --cargo FILE         Cargo manifest of the backend (default backend/axum/Cargo.toml)
  --binary FILE        Compiled binary (default: the release binary Cargo names)
  --ui DIRECTORY       Built interface, copied as dist/ (default dist)
  --icon FILE          Source of the icon the manifest declares (default assets/logo.svg)
  --help               Show this help
`;

/** A path inside the package: relative, and unable to climb out of it. */
function packagePath(value, field) {
  if (typeof value !== 'string' || !value || /[\\\0]/.test(value))
    throw new Error(`${field} is not a packaged path: ${value}`);
  if (posix.isAbsolute(value) || value.split('/').includes('..'))
    throw new Error(`${field} has to stay inside the package: ${value}`);
  return posix.normalize(value);
}

try {
  const { values } = parseArgs({
    options: {
      output: { type: 'string', default: 'release-assets' },
      version: { type: 'string' },
      manifest: { type: 'string', default: 'nas-app.json' },
      cargo: { type: 'string', default: 'backend/axum/Cargo.toml' },
      binary: { type: 'string' },
      ui: { type: 'string', default: 'dist' },
      icon: { type: 'string', default: 'assets/logo.svg' },
      help: { type: 'boolean' },
    },
  });
  if (values.help) {
    console.log(help);
  } else {
    const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
    const inside = (path) => resolve(root, path);
    const manifest = JSON.parse(readFileSync(inside(values.manifest), 'utf8'));
    const { id, version, icon } = manifest.app ?? {};
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(id ?? '')) throw new Error(`Invalid app.id: ${id}`);
    if (!version) throw new Error('The manifest declares no app.version');

    // Cargo parses its own manifest, including workspace-inherited versions, and names the binary
    // it produces: neither is guessed from the directory.
    const metadata = JSON.parse(
      execFileSync(
        'cargo',
        [
          'metadata',
          '--no-deps',
          '--offline',
          '--format-version',
          '1',
          '--manifest-path',
          inside(values.cargo),
        ],
        { encoding: 'utf8' },
      ),
    );
    if (metadata.packages.length !== 1)
      throw new Error(`${values.cargo} declares ${metadata.packages.length} packages, expected 1`);
    const [crate] = metadata.packages;
    const executableName = (crate.targets.find((target) => target.kind.includes('bin')) ?? crate)
      .name;

    const versions = {
      [values.manifest]: version,
      'package.json': JSON.parse(readFileSync(inside('package.json'), 'utf8')).version,
      [values.cargo]: crate.version,
    };
    if (values.version) versions['release tag'] = values.version.replace(/^v/, '');
    for (const [name, value] of Object.entries(versions))
      if (value !== version)
        throw new Error(`${name}: ${value} differs from manifest version ${version}`);

    if (process.platform !== 'linux' || !['x64', 'arm64'].includes(process.arch))
      throw new Error('Build the NAS package natively on Linux x64 or arm64');
    const binary = values.binary
      ? inside(values.binary)
      : join(dirname(inside(values.cargo)), '../target/release', executableName);
    if (!existsSync(binary)) throw new Error(`Build the release binary first: ${binary}`);
    if (!existsSync(join(inside(values.ui), 'index.html')))
      throw new Error(`Build the interface first: ${join(values.ui, 'index.html')}`);

    const output = resolve(values.output);
    mkdirSync(output, { recursive: true });
    // The name carries the version, because that is what the catalogue of the NAS substitutes.
    const asset = join(output, `${id}-${version}-linux-${process.arch}.tar.gz`);
    const staging = mkdtempSync(join(tmpdir(), 'nas-package-'));
    try {
      // run.exec is read out of the manifest by the NAS and executed: it has to be a path inside
      // the package here too, or this script would write outside its own staging directory.
      const executable = join(staging, packagePath(manifest.run?.exec, 'run.exec'));
      mkdirSync(dirname(executable), { recursive: true });
      copyFileSync(binary, executable);
      chmodSync(executable, 0o755);
      cpSync(inside(values.ui), join(staging, 'dist'), { recursive: true });
      copyFileSync(inside(values.manifest), join(staging, 'nas-app.json'));
      if (icon !== undefined) {
        const source = inside(values.icon);
        if (!existsSync(source))
          throw new Error(`The manifest declares an icon, ${source} is not there`);
        copyFileSync(source, join(staging, packagePath(icon, 'app.icon')));
      }
      execFileSync('tar', [
        '--create',
        // gzip writes the time of the build into its own header unless it is told not to.
        '--use-compress-program=gzip -n -9',
        '--format=posix',
        // Two builds of the same tree produce the same archive: no owner, no build timestamp, one
        // order. SOURCE_DATE_EPOCH is what a reproducible pipeline sets. The two times tar would
        // otherwise record in a PAX header are the ones nobody chose and nobody reads.
        '--pax-option=delete=atime,delete=ctime',
        '--sort=name',
        '--owner=0',
        '--group=0',
        '--numeric-owner',
        `--mtime=@${process.env.SOURCE_DATE_EPOCH ?? 0}`,
        '--file',
        asset,
        '--directory',
        staging,
        '.',
      ]);
    } finally {
      rmSync(staging, { recursive: true, force: true });
    }
    const digest = createHash('sha256').update(readFileSync(asset)).digest('hex');
    // sha256sum's own format, which is what the NAS reads beside the asset.
    writeFileSync(`${asset}.sha256`, `${digest}  ${basename(asset)}\n`);
    console.log(`${asset} (${statSync(asset).size} bytes)`);
  }
} catch (error) {
  console.error(`FAIL: ${error.message}`);
  process.exitCode = 1;
}
