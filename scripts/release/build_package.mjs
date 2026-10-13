// Assembles the release package: the installation and the configuration of the launcher, with the
// binary, the built interface and the backend helpers next to the installer, and the user manual at
// its root, which the wizard copies next to its installation manual. The wizard downloads it, and so
// does the update of the launcher.
// Run after `npm run build`, `cargo build --release` and `npm run manual:build`.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { basename, join } from 'node:path';
import { pruneAssets } from './prune_assets.mjs';

const root = join(import.meta.dirname, '..', '..');
const { version } = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
const assets = join(root, 'release-assets');
const staging = join(assets, 'package');
const installer = join(staging, 'packages', 'ui_installation');
const binary = join(root, 'backend', 'target', 'release', 'nas-os-axum');
const ui = join(root, 'dist');
const manual = join(assets, `nas_os_user_manual_${version}.pdf`);

// The name the launcher looks for when it updates itself.
const archive = `nas-os-${version}-linux-x64.tar.gz`;

for (const required of [binary, ui, manual]) {
  if (!existsSync(required)) {
    throw new Error(`${required} is missing: build it before packaging.`);
  }
}

function helpers(directory) {
  return readdirSync(directory, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith('.sh'))
    .map((entry) => join(entry.parentPath, entry.name));
}

rmSync(staging, { recursive: true, force: true });
cpSync(join(root, 'system', 'packages'), join(staging, 'packages'), { recursive: true });
cpSync(binary, join(installer, 'bin', 'nas-os-axum'));
cpSync(ui, join(installer, 'ui'), { recursive: true });
cpSync(join(root, 'config', 'production.env'), join(installer, 'files', 'nas-os.env'));
const bundledApps = join(root, 'system', 'apps');
if (existsSync(bundledApps)) {
  cpSync(bundledApps, join(installer, 'apps'), { recursive: true });
}

cpSync(manual, join(staging, `nas_os_user_manual_${version}.pdf`));

mkdirSync(join(installer, 'helpers'));
for (const helper of helpers(join(root, 'backend', 'axum', 'src'))) {
  cpSync(helper, join(installer, 'helpers', basename(helper)));
}

execFileSync('tar', ['--owner=0', '--group=0', '-czf', join(assets, archive), '-C', staging, '.'], {
  stdio: 'inherit',
});
rmSync(staging, { recursive: true, force: true });

const digest = createHash('sha256')
  .update(readFileSync(join(assets, archive)))
  .digest('hex');
writeFileSync(join(assets, `${archive}.sha256`), `${digest}  ${archive}\n`);

console.log(`Release package written to ${join(assets, archive)}.`);

// Only this release and the one before it stay in the folder.
for (const name of pruneAssets(assets, version, ['nas-os-', 'nas_os_user_manual_'])) {
  console.log(`Removed ${name}`);
}
