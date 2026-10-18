import { t } from '@/i18n';
import { appInfo, appLabel, SETTINGS_APP_ID } from '@/apps/registry';
import { sectionIcons, sectionLabels, settingsEntries } from '@/apps/nas-system-settings/search';
import fileIcon from '@/assets/icons/file.svg';
import folderIcon from '@/assets/icons/folder.svg';
import type { SettingsSection } from '@/apps/nas-system-settings/types/search';
import type { ShellApp } from '@/types/types';
import type { AppOrigin, SearchEntry, SearchResult } from '@/apps/nas-global-search/types';

const SEARCH_APP_ID = 'nas-global-search';

function normalize(text: string) {
  return text.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase();
}

function matches(query: string, ...texts: string[]) {
  const wanted = normalize(query.trim());
  return texts.some((text) => normalize(text).includes(wanted));
}

/**
 * The applications whose name or description contains the query.
 * @param apps - The applications of the shell.
 * @param query - What was typed.
 * @param origin - Only system or only installed applications, or both.
 */
export function appEntries(apps: ShellApp[], query: string, origin: AppOrigin): SearchEntry[] {
  return apps
    .filter((app) => app.id !== SEARCH_APP_ID)
    .filter((app) => !origin || app.system === (origin === 'system'))
    .map((app) => ({
      key: `app:${app.id}`,
      kind: 'app' as const,
      title: appLabel(app.id, app.name),
      detail: appInfo(app.id, app.info),
      icon: app.entry.icon,
      app: app.id,
    }))
    .filter((entry) => matches(query, entry.title, entry.detail));
}

/**
 * The settings entries whose label or section contains the query.
 * @param query - What was typed.
 * @param section - Only one section, or all of them.
 */
export function settingEntries(query: string, section: SettingsSection | ''): SearchEntry[] {
  return settingsEntries
    .filter((entry) => !section || entry.section === section)
    .map((entry) => ({
      key: `setting:${entry.section}:${entry.label}`,
      kind: 'setting' as const,
      title: t(entry.label),
      detail: `${t('settings')} / ${t(sectionLabels[entry.section])}`,
      icon: sectionIcons[entry.section],
      app: SETTINGS_APP_ID,
      query: entry.section,
    }))
    .filter((entry) => matches(query, entry.title, entry.detail));
}

/**
 * A file found by a provider. Its `target` becomes the query string the provider opens on, and
 * anything else is ignored.
 * @param result - The file.
 */
export function fileEntry(result: SearchResult): SearchEntry {
  return {
    key: `file:${result.id}`,
    kind: 'file',
    title: result.name,
    detail: result.path,
    icon: result.type === 'folder' ? folderIcon : fileIcon,
    app: result.app_id,
    location: result.target.startsWith('?') ? result.target : undefined,
  };
}
