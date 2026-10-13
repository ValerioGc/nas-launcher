// Keeps package-lock.json, the lockfile the pipelines install from with `npm ci`, in step with
// package.json while the project is installed locally with pnpm.
//
//   node scripts/npm-lock.mjs --sync     # rewrites the lockfile, installs nothing
//   node scripts/npm-lock.mjs --check    # fails when package.json asks for what the lock lacks
//
// pnpm runs the --sync as `postinstall` after an install that changes something. `pnpm add`,
// `update` and `remove` do not run it: `pnpm run lint` runs the --check, which says to run
// `pnpm run lock:npm`. Under npm, as in the pipelines, the --sync does nothing: there the lockfile
// is what installs.

import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const FIELDS = ['dependencies', 'devDependencies', 'optionalDependencies'];

function fail(message) {
  console.error(`package-lock.json: ${message}`);
  process.exit(1);
}

function sync() {
  const agent = process.env.npm_config_user_agent ?? '';
  if (!agent.startsWith('pnpm/') || process.env.CI) {
    return;
  }
  execFileSync(
    'npm',
    ['install', '--package-lock-only', '--ignore-scripts', '--no-audit', '--no-fund'],
    { stdio: 'inherit' },
  );
}

function check() {
  const manifest = JSON.parse(readFileSync('package.json', 'utf8'));
  let lock;
  try {
    lock = JSON.parse(readFileSync('package-lock.json', 'utf8'));
  } catch {
    fail('missing or unreadable: run pnpm run lock:npm');
  }
  const root = lock.packages?.[''] ?? {};
  const differences = FIELDS.flatMap((field) => {
    const wanted = manifest[field] ?? {};
    const locked = root[field] ?? {};
    return [...new Set([...Object.keys(wanted), ...Object.keys(locked)])]
      .filter((name) => wanted[name] !== locked[name])
      .map(
        (name) => `  ${name}: package.json ${wanted[name] ?? '-'}, lockfile ${locked[name] ?? '-'}`,
      );
  });
  if (differences.length > 0) {
    fail(
      `not in step with package.json, and npm ci would refuse it:\n${differences.join('\n')}\n` +
        'run pnpm run lock:npm',
    );
  }
}

if (process.argv[2] === '--sync') {
  sync();
} else if (process.argv[2] === '--check') {
  check();
} else {
  fail('usage: node scripts/npm-lock.mjs --sync | --check');
}
