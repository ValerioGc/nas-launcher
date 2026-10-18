import { beforeEach, expect, it } from 'vitest';
import { appEntries, fileEntry, settingEntries } from '@/apps/nas-global-search/entries';
import { resolveApp } from '@/apps/registry';
import { sectionIcons } from '@/apps/nas-system-settings/search';
import { setLocale } from '@/i18n';
import type { SearchResult } from '@/apps/nas-global-search/types';
import type { ShellApp } from '@/types/types';

function app(id: string, name: string, system: boolean): ShellApp {
  return { id, name, info: '', system, entry: resolveApp(id) };
}

const apps = [
  app('nas-system-settings', 'Settings', true),
  app('nas-global-search', 'Global search', true),
  app('photos', 'Photo Gallery', false),
];

beforeEach(() => setLocale('en'));

it('finds applications by name, without accents or case, and never itself', () => {
  expect(appEntries(apps, 'SETT', '').map((entry) => entry.app)).toEqual(['nas-system-settings']);
  expect(appEntries(apps, 'search', '')).toEqual([]);
  expect(appEntries([app('music', 'Città', false)], 'citta', '')).toHaveLength(1);
});

it('keeps only system or only installed applications when asked', () => {
  expect(appEntries(apps, 'e', 'installed').map((entry) => entry.app)).toEqual(['photos']);
  expect(appEntries(apps, 'e', 'system').map((entry) => entry.app)).toEqual([
    'nas-system-settings',
  ]);
});

it('finds settings entries by label or section and opens their section', () => {
  const found = settingEntries('scheduled', '');
  expect(found).toEqual([
    expect.objectContaining({ app: 'nas-system-settings', query: 'power', kind: 'setting' }),
  ]);
  expect(settingEntries('scheduled', 'general')).toEqual([]);

  expect(settingEntries('theme', 'general')).toHaveLength(2);
});

it('opens a file in the provider on its query string, and on nothing else', () => {
  const file: SearchResult = {
    id: 'nas-file-manager:7',
    name: 'Report',
    path: '/docs/report.pdf',
    type: 'document',
    extension: 'pdf',
    size: 10,
    modified: '',
    app_id: 'nas-file-manager',
    target: '?path=%2Fdocs',
  };
  expect(fileEntry(file)).toMatchObject({ app: 'nas-file-manager', location: '?path=%2Fdocs' });
  expect(fileEntry({ ...file, target: 'https://example.test/' }).location).toBeUndefined();
});

it('shows a settings entry with the icon of its section', () => {
  const [power] = settingEntries('scheduled', '');
  expect(power?.icon).toBe(sectionIcons.power);
  for (const entry of settingEntries('', '')) {
    expect(entry.icon).toBe(sectionIcons[entry.query as keyof typeof sectionIcons]);
  }
});

it('shows a folder and a file with icons of their own', () => {
  const file: SearchResult = {
    id: 'nas-file-manager:7',
    name: 'Report',
    path: '/docs/report.pdf',
    type: 'document',
    extension: 'pdf',
    size: 10,
    modified: '',
    app_id: 'nas-file-manager',
    target: '?path=%2Fdocs',
  };
  const document = fileEntry(file).icon;
  const folder = fileEntry({ ...file, type: 'folder' }).icon;
  expect(document).toBeTruthy();
  expect(folder).toBeTruthy();
  expect(document).not.toBe(folder);
  expect(fileEntry({ ...file, type: 'image' }).icon).toBe(document);
});
