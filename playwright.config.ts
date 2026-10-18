import { defineConfig, devices } from '@playwright/test';
import { defineBddConfig } from 'playwright-bdd';

const testDir = defineBddConfig({
  features: 'src/tests/e2e/features/**/*.feature',
  steps: 'src/tests/e2e/steps/**/*.ts',
  outputDir: 'src/tests/e2e/.features-gen',
});

const SHARED = ['catalogue', 'settings', 'launcher', 'tasks'];

export default defineConfig({
  testDir,
  fullyParallel: true,
  workers: 4,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:5174',
    trace: 'on-first-retry',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
      testIgnore: SHARED.map((feature) => `**/${feature}.feature.spec.js`),
    },
    ...SHARED.map((feature, index) => ({
      name: feature,
      use: { ...devices['Desktop Chrome'] },
      testMatch: `**/${feature}.feature.spec.js`,
      dependencies: [index === 0 ? 'chromium' : SHARED[index - 1]],
    })),
  ],
  webServer: [
    {
      command: 'node src/tests/e2e/fixtures/gateway.mjs 3003',
      url: 'http://127.0.0.1:3003/health',
      reuseExistingServer: !process.env.CI,
    },
    {
      command:
        'rm -rf backend/data/e2e.db backend/data/e2e.db-wal backend/data/e2e.db-shm backend/data/e2e-apps && printf e2e > backend/data/e2e-token && cargo run --locked --manifest-path backend/axum/Cargo.toml',

      env: {
        PORT: '3001',
        RUSTC_WRAPPER: '',
        NAS_DB_PATH: 'backend/data/e2e.db',
        NAS_PACKAGES_MODE: 'real',
        NAS_GITHUB_API: 'http://127.0.0.1:3002',
        NAS_CATALOGUE_PATH: 'src/tests/e2e/fixtures/catalogue.json',
        NAS_APPS_DIR: 'backend/data/e2e-apps',
        NAS_APP_SOCKETS: 'backend/data/e2e-sockets',
        NAS_TERMINAL_MODE: 'real',
        NAS_TERMINAL_SOCKET: 'backend/data/e2e-terminal.sock',
        NAS_LAUNCHER_TOKEN_PATH: 'backend/data/e2e-token',
      },
      url: 'http://127.0.0.1:3001/health',
      timeout: 300_000,
      reuseExistingServer: !process.env.CI,
    },
    {
      command: 'node src/tests/e2e/fixtures/shell.mjs backend/data/e2e-terminal.sock 3005',
      url: 'http://127.0.0.1:3005/',
      reuseExistingServer: !process.env.CI,
    },
    {
      command: 'node src/tests/e2e/fixtures/forge.mjs 3002',
      url: 'http://127.0.0.1:3002/repos/example/nas-file-explorer/releases/latest',
      reuseExistingServer: !process.env.CI,
    },
    {
      command:
        'node src/tests/e2e/fixtures/app.mjs backend/data/e2e-sockets/fileExplorer.sock 3004',
      url: 'http://127.0.0.1:3004/who',
      reuseExistingServer: !process.env.CI,
    },
    {
      command: 'npm run dev -- --mode e2e --host 127.0.0.1 --port 5174 --strictPort',
      env: {
        NAS_API_TARGET: 'http://127.0.0.1:3001',
        NAS_GATEWAY_TARGET: 'http://127.0.0.1:3003',
      },
      url: 'http://127.0.0.1:5174',
      reuseExistingServer: !process.env.CI,
    },
  ],
});
