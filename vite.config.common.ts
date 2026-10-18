import { fileURLToPath, URL } from 'node:url';

import vue from '@vitejs/plugin-vue';
import type { UserConfig } from 'vite';

import combineSelectors from 'postcss-combine-duplicated-selectors';
import purgecss from '@fullhuman/postcss-purgecss';
import autoprefixer from 'autoprefixer';
import cssnano from 'cssnano';

const gatewayLogout = process.env.NAS_GATEWAY_LOGOUT ?? '/auth/logout';

/**
 * The classes the components put together at runtime (`common_spinner_${size}`). PurgeCSS only
 * keeps a class whose whole name it finds in the sources, so without these the variants vanish
 * from the build. `src/tests/scripts/build.spec.ts` checks every prefix is here.
 */
export const RUNTIME_CLASS_PREFIXES = [
  'alert_banner_',
  'common_icon_',
  'common_skeleton_',
  'common_spinner_',
  'desktop_grid_',
  'icon_sample_',
  'notification_',
  'notification_toast_timer_',
  'task_',
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
          content: [
            './index.html',
            './public/**/*.html',
            './src/**/*.vue',
            './src/**/*.ts',
            './src/**/*.scss',
          ],
          safelist: {
            standard: [
              /^v-/,
              /^dark/,
              ...RUNTIME_CLASS_PREFIXES.map((prefix) => new RegExp(`^${prefix}`)),
            ],
          },
          defaultExtractor: (content) => content.match(/[\w-/:]+(?<!:)/g) ?? [],
        }),
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
