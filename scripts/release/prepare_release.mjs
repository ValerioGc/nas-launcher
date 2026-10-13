// Local release gate: formats the code, runs every check and test, then builds the interface, the
// release binary, the user manual and the release package.
// The build pipeline only verifies, builds and publishes, so this is where a release is tested.

import { execSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

const CARGO = '--manifest-path backend/Cargo.toml';

const steps = [
  ['Syncing the npm lockfile of the pipelines', 'pnpm run lock:npm'],
  ['Generating the API document and its types', 'pnpm run api:generate'],
  ['Formatting code', 'pnpm run format'],
  ['Checking formatting', 'pnpm run format:check'],
  ['Checking Rust formatting', `cargo fmt ${CARGO} --check`],
  ['Linting', 'pnpm run lint'],
  ['Running clippy', `cargo clippy ${CARGO} --all-targets -- -D warnings`],
  ['Running unit tests', 'pnpm run test:unit'],
  ['Running component tests', 'pnpm run test:component'],
  ['Running end-to-end tests', 'pnpm run test:e2e'],
  ['Running Rust tests', `cargo test ${CARGO}`],
  ['Building the interface', 'pnpm run build'],
  ['Building the release binary', `cargo build --release --locked ${CARGO}`],
  ['Building the user manual', 'pnpm run manual:build'],
  ['Building the release package', 'pnpm run release:package'],
];

export function prepareRelease() {
  steps.forEach(([label, command], index) => {
    console.log(`\n▶ ${index + 1}/${steps.length} | ${label}`);
    execSync(command, { stdio: 'inherit' });
  });

  console.log('\n✅ Release preparation complete.');
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    prepareRelease();
  } catch {
    console.error('\n❌ Release preparation failed.');
    process.exit(1);
  }
}
