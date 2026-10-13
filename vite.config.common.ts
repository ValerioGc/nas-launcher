import { fileURLToPath, URL } from 'node:url';

import vue from '@vitejs/plugin-vue';
import type { UserConfig } from 'vite';

import combineSelectors from 'postcss-combine-duplicated-selectors';
import purgecss from '@fullhuman/postcss-purgecss';
import autoprefixer from 'autoprefixer';
import cssnano from 'cssnano';

/**
 * Common Vite configuration for the NasOS launcher.
 */
const gatewayLogout = process.env.NAS_GATEWAY_LOGOUT ?? '/auth/logout';

/**
 * The classes the components put together at runtime (`common_spinner_${size}`): PurgeCSS only
 * keeps a class whose whole name it finds in the sources, so without these the variants vanish
 * from the build. `src/tests/scripts/build.spec.ts` checks every prefix is here.
 */
export const RUNTIME_CLASS_PREFIXES = [
  'alert_banner_',
  'common_icon_',
  'common_skeleton_',
  'common_spinner_',
  'desktop_grid_',
  'gauge_',
  'icon_sample_',
  'notification_',
  'notification_toast_timer_',
  'task_',
  'terminal_line_',
];

export const commonConfig: UserConfig = {
  plugins: [vue()],
  define: {
    __GATEWAY_LOGOUT_URL__: JSON.stringify(gatewayLogout),
  },
  css: {
    preprocessorOptions: {
      scss: {
        additionalData: `@use '@/styles/global' as *;\n`,
      },
    },

    postcss: {
      plugins: [
        combineSelectors({ removeDuplicatedProperties: true }),
        purgecss({
          content: ['./public/**/*.html', './src/**/*.vue', './src/**/*.ts', './src/**/*.scss'],
          safelist: {
            standard: [
              /^v-/,
              /^dark/,
              ...RUNTIME_CLASS_PREFIXES.map((prefix) => new RegExp(`^${prefix}`)),
            ],
          },
          defaultExtractor: (content) => content.match(/[\w-/:]+(?<!:)/g) ?? [],
        }),
        // Both read the browsers of `browserslist` in package.json: the same as the default target of
        // Vite, so the prefixes added here are the ones its minifier keeps.
        autoprefixer(),
        cssnano({ preset: 'default' }),
      ],
    },
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '@tests': fileURLToPath(new URL('./src/tests', import.meta.url)),
      '@scripts': fileURLToPath(new URL('./scripts', import.meta.url)),
    },
  },
};
