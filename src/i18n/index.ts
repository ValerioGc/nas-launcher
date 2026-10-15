import { readonly, ref } from 'vue';
import type { Locale } from '@/types/types';

import launcherIt from './launcher/it';
import launcherEn from './launcher/en';
import managerIt from './apps/nas-app-manager/it';
import managerEn from './apps/nas-app-manager/en';
import notificationsIt from './apps/nas-notification-manager/it';
import notificationsEn from './apps/nas-notification-manager/en';
import searchIt from './apps/nas-global-search/it';
import searchEn from './apps/nas-global-search/en';
import settingsIt from './apps/nas-system-settings/it';
import settingsEn from './apps/nas-system-settings/en';

export type { Locale };

/**
 * Catalogs for each supported locale, combining the launcher and all applications.
 */
const catalogs: Record<Locale, Record<string, string>> = {
  it: {
    ...launcherIt,
    ...managerIt,
    ...notificationsIt,
    ...searchIt,
    ...settingsIt,
  },
  en: {
    ...launcherEn,
    ...managerEn,
    ...notificationsEn,
    ...searchEn,
    ...settingsEn,
  },
};

const current = ref<Locale>('it');
export const locale = readonly(current);

/**
 * Switches language, ignoring anything that is not one of the languages that exist.
 */
export function setLocale(value: Locale) {
  if (!Object.hasOwn(catalogs, value)) {
    return;
  }

  current.value = value;
  document.documentElement.lang = value;
}

export function t(key: string, params: Record<string, string | number> = {}): string {
  const catalog = catalogs[current.value];
  return (catalog[key] ?? key).replace(/\{(\w+)\}/g, (match, name: string) =>
    String(params[name] ?? match),
  );
}

/**
 * Formats a date according to the current locale.
 */
export function formatDate(value: string | number) {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? ''
    : new Intl.DateTimeFormat(current.value, { dateStyle: 'short', timeStyle: 'short' }).format(
        date,
      );
}
