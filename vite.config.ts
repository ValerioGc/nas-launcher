import { defineConfig } from 'vite';
import { commonConfig } from './vite.config.common.ts';
import vue from '@vitejs/plugin-vue';
import { stripTestIds, stripTestIdsNodeTransform } from './scripts/build/strip-test-ids.ts';

// Plugins used only in production builds.
const productionPlugins = [
  vue({
    template: {
      compilerOptions: {
        nodeTransforms: [stripTestIdsNodeTransform],
      },
    },
  }),
  stripTestIds(),
];

export default defineConfig(({ command, mode }) => ({
  ...commonConfig,
  plugins: command === 'build' ? productionPlugins : commonConfig.plugins,
  server: {
    ...(commonConfig.server ?? {}),

    // E2E runs use fixed sources: avoid exhausting the system's inotify watchers.
    watch: mode === 'e2e' ? null : { ignored: ['**/target/**', '**/backend/data/**'] },
    proxy: {
      '/api': process.env.NAS_API_TARGET ?? 'http://127.0.0.1:3000',
      '/apps': { target: process.env.NAS_API_TARGET ?? 'http://127.0.0.1:3000', ws: true },
      '/auth': process.env.NAS_GATEWAY_TARGET ?? 'http://127.0.0.1:3003',
    },
  },
}));
