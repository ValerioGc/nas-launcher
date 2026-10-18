import { defineConfig } from 'vitest/config';
import { commonConfig } from './vite.config.common.ts';

export default defineConfig({
  ...commonConfig,
  test: {
    projects: [
      {
        extends: true,
        test: { name: 'unit', environment: 'jsdom', include: ['src/tests/**/*.unit.spec.ts'] },
      },
      {
        extends: true,
        test: {
          name: 'component',
          environment: 'jsdom',
          include: ['src/tests/**/*.component.spec.ts'],
        },
      },
      {
        extends: true,
        test: {
          name: 'scripts',
          environment: 'node',
          include: ['src/tests/scripts/**/*.spec.ts'],
        },
      },
    ],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      reportsDirectory: 'coverage',
      include: ['src/**/*.{ts,vue}'],
      exclude: ['src/tests/**', 'src/main.ts'],
    },
  },
});
