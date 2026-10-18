import { beforeEach, describe, expect, it } from 'vitest';
import {
  appInfo,
  appLabel,
  externalUrl,
  internalApps,
  resolveApp,
  SETTINGS_APP_ID,
} from '@/apps/registry';
import { setLocale } from '@/i18n';

beforeEach(() => setLocale('it'));

describe('application registry', () => {
  it('localizes the renamed external applications without changing their source', () => {
    expect(appLabel('nas-log-manager', 'Log')).toBe('Visualizzatore log');
    expect(appLabel('nas-system-monitor', 'Monitor di sistema')).toBe('Monitor sistema');
    setLocale('en');
    expect(appLabel('nas-log-manager', 'Log')).toBe('Log viewer');
    expect(resolveApp('nas-log-manager').source.kind).toBe('external');
    expect(resolveApp('nas-system-monitor').source.kind).toBe('external');
  });
  it.each(Object.keys(internalApps))('opens %s from inside the launcher', (id) => {
    const entry = resolveApp(id);

    expect(entry.source.kind).toBe('internal');
    expect(entry.icon).toBeTruthy();
    expect(['fullscreen', 'modal']).toContain(entry.presentation);
  });

  it('treats an unknown id as an external application', () => {
    const entry = resolveApp('mediaServer');

    expect(entry.source).toEqual({ kind: 'external', url: '/apps/mediaServer/' });
    expect(entry.label).toBe('mediaServer');
    expect(entry.icon).toBeTruthy();
  });

  it('serves each external application under a path of its own', () => {
    expect(externalUrl('fileExplorer')).toBe('/apps/fileExplorer/');
    expect(externalUrl('a b')).toBe('/apps/a%20b/');
  });

  it('is the settings application that the menu reaches with its own button', () => {
    expect(SETTINGS_APP_ID).toBe('nas-system-settings');
    expect(internalApps[SETTINGS_APP_ID]).toBeDefined();
  });

  it('gives a known application the name of the active locale', () => {
    expect(appLabel('nas-app-manager', 'Application manager')).toBe('Gestore applicazioni');

    setLocale('en');
    expect(appLabel('nas-app-manager', 'Application manager')).toBe('Application manager');
  });

  it('describes every application of the launcher in the active locale', () => {
    for (const id of Object.keys(internalApps)) {
      expect(appInfo(id, 'from the backend')).not.toBe('from the backend');
    }
    setLocale('en');
    expect(appInfo('mediaServer', 'Riproduce musica')).toBe('Riproduce musica');
  });

  it('keeps the name the backend gave to an application it does not know', () => {
    expect(appLabel('mediaServer', 'Server multimediale')).toBe('Server multimediale');
  });
});
