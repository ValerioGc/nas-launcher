import { afterEach, describe, expect, it } from 'vitest';
import { t, setLocale, formatDate } from '@/i18n';
import launcherIt from '@/i18n/launcher/it';
import launcherEn from '@/i18n/launcher/en';
import managerIt from '@/i18n/apps/nas-app-manager/it';
import managerEn from '@/i18n/apps/nas-app-manager/en';
import notificationsIt from '@/i18n/apps/nas-notification-manager/it';
import notificationsEn from '@/i18n/apps/nas-notification-manager/en';
import searchIt from '@/i18n/apps/nas-global-search/it';
import searchEn from '@/i18n/apps/nas-global-search/en';
import settingsIt from '@/i18n/apps/nas-system-settings/it';
import settingsEn from '@/i18n/apps/nas-system-settings/en';

const catalogs = {
  launcher: [launcherIt, launcherEn],
  manager: [managerIt, managerEn],
  notifications: [notificationsIt, notificationsEn],
  search: [searchIt, searchEn],
  settings: [settingsIt, settingsEn],
} as const;

afterEach(() => setLocale('it'));

describe('localization', () => {
  it.each(Object.entries(catalogs))('has matching %s catalogs', (_name, [italian, english]) => {
    expect(Object.keys(english).sort()).toEqual(Object.keys(italian).sort());
  });

  it('keeps every key in a single file', () => {
    const seen = new Map<string, string>();
    const duplicates: string[] = [];
    for (const [name, [italian]] of Object.entries(catalogs)) {
      for (const key of Object.keys(italian)) {
        const owner = seen.get(key);
        if (owner) {
          duplicates.push(`${key}: ${owner} e ${name}`);
        }
        seen.set(key, name);
      }
    }
    expect(duplicates).toEqual([]);
  });

  it('changes document language and interpolates without HTML', () => {
    setLocale('en');
    expect(t('save')).toBe('Save');
    expect(document.documentElement.lang).toBe('en');
    expect(t('closeApp', { name: '<script>' })).toBe('Close <script>');
    setLocale('it');
    expect(t('save')).toBe('Salva');
  });

  it('reads the applications catalogs too', () => {
    expect(t('networkSettings')).toBe('Rete');
    setLocale('en');
    expect(t('searchResults')).toBe('Search results');
  });

  it('handles invalid dates', () => {
    expect(formatDate('invalid')).toBe('');
  });
});
