import { defineAsyncComponent, h, type AsyncComponentLoader, type Component } from 'vue';
import { t } from '@/i18n';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import type { AppEntry } from '@/types/types';

import appManagerLogo from './nas-app-manager/logo.svg';
import globalSearchLogo from './nas-global-search/logo.svg';
import globalSettingsLogo from './nas-system-settings/logo.svg';
import notificationManagerLogo from './nas-notification-manager/logo.svg';
import defaultLogo from './default_logo.svg';

/**
 * A fullscreen internal application is split into its own chunk: while it arrives, its panel shows a spinner
 * instead of an empty window.
 */
function lazy(loader: AsyncComponentLoader): Component {
  return defineAsyncComponent({
    loader,
    loadingComponent: () => h(CommonSpinner, { size: 'huge', centered: true }),
    delay: 0,
  });
}

/**
 * Application reachable via a dedicated button
 */
export const SETTINGS_APP_ID = 'nas-system-settings';

/**
 * Internal applications. The `id` is the one returned by `/api/apps`: it is the key with which the shell
 * opens the application, and it must match the backend's identifier.
 */
export const internalApps: Record<string, AppEntry> = {
  'nas-app-manager': {
    label: 'appManager',
    presentation: 'fullscreen',
    icon: appManagerLogo,
    source: {
      kind: 'internal',
      component: lazy(() => import('./nas-app-manager/AppManager.vue')),
    },
  },
  'nas-global-search': {
    label: 'globalSearch',
    presentation: 'modal',
    icon: globalSearchLogo,
    source: {
      kind: 'internal',
      component: defineAsyncComponent(() => import('./nas-global-search/SearchApp.vue')),
    },
  },
  'nas-notification-manager': {
    label: 'notificationManager',
    presentation: 'fullscreen',
    icon: notificationManagerLogo,
    source: {
      kind: 'internal',
      component: lazy(() => import('./nas-notification-manager/NotificationManager.vue')),
    },
  },
  [SETTINGS_APP_ID]: {
    label: 'globalSettings',
    presentation: 'fullscreen',
    icon: globalSettingsLogo,
    source: {
      kind: 'internal',
      component: lazy(() => import('./nas-system-settings/GlobalSettings.vue')),
    },
  },
};

/**
 * Where an installed application is served: `/apps/<id>/`, on the address the launcher was opened
 * at. The gateway forwards it to the unix socket of its process, so whatever reaches the NAS
 * reaches its applications, with nothing to configure on the network.
 */
export function externalUrl(id: string): string {
  return `/apps/${encodeURIComponent(id)}/`;
}

const externalLabels: Record<string, string> = {
  'nas-system-terminal': 'systemTerminal',
  'nas-log-manager': 'logViewer',
  'nas-system-monitor': 'systemMonitor',
};

/**
 * An unknown id in the registry is an application installed at runtime.
 * It lives outside the launcher, in its own process, and opens in an iframe.
 */
function externalApp(id: string): AppEntry {
  return {
    label: externalLabels[id] ?? id,
    presentation: 'fullscreen',
    icon: defaultLogo,
    source: { kind: 'external', url: externalUrl(id) },
  };
}

/**
 * The single point where an id becomes something the shell knows how to open, internal or external
 */
export function resolveApp(id: string): AppEntry {
  return (Object.hasOwn(internalApps, id) ? internalApps[id] : undefined) ?? externalApp(id);
}

/**
 * The icon the shell draws
 */
export function appIcon(id: string, packaged?: string): string {
  return packaged || resolveApp(id).icon;
}

/**
 * Gets the localized description of an application of the launcher, retaining the backend one for
 * the applications it does not know.
 */
export function appInfo(id: string, fallback: string): string {
  const entry = Object.hasOwn(internalApps, id) ? internalApps[id] : undefined;
  if (!entry) {
    return fallback;
  }
  const key = `${entry.label}Info`;
  const translated = t(key);
  return translated === key ? fallback : translated;
}

/**
 * Gets the localized name known by the launcher, retaining the backend name when no label exists.
 */
export function appLabel(id: string, fallback: string): string {
  const { label = id } = resolveApp(id);
  const translated = t(label);
  return translated === label ? fallback : translated;
}
