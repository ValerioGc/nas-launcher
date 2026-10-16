import { formatDate, t } from '@/i18n';
import { appLabel } from '@/apps/registry';
import { useAppsStore } from '@/stores/appsStore';
import type { AppNotification } from './types';

/**
 * A value to fill in: a time when its name ends in `At`, a word the catalogue translates, such as
 * an action or a step, or else the value as it came.
 */
function localized(name: string, value: string) {
  if (name.endsWith('At') && Number.isFinite(Number(value))) {
    return formatDate(Number(value));
  }
  const key = `noticeValue_${value}`;
  const translated = t(key);
  return translated === key ? value : translated;
}

/**
 * The place a notification names in its message, which its words `{link}` open: an application
 * and the query string of the page it opens on. Known by key only, so that what an application
 * writes in a notification never decides what the shell opens.
 */
const NOTICE_LINKS: Record<string, NoticeTarget> = {
  discoveryOff: { app: 'nas-security-center', location: '?section=access' },
};

/** Where the link of a notification leads. */
export interface NoticeTarget {
  app: string;
  location: string;
}

/** A message split around the words that open the place it names. */
export interface NoticeParts {
  before: string;
  link: string;
  after: string;
  target: NoticeTarget;
}

/** The words of the link of a notification, or `undefined` when it has none. */
function linkLabel(item: AppNotification) {
  if (!item.key || !Object.hasOwn(NOTICE_LINKS, item.key)) {
    return undefined;
  }
  const key = `notice_${item.key}_link`;
  const label = t(key);
  return label === key ? undefined : label;
}

function translated(item: AppNotification, part: 'title' | 'message', link?: string) {
  if (!item.key) {
    return item[part];
  }
  const key = `notice_${item.key}_${part}`;
  const params = Object.fromEntries(
    Object.entries(item.params ?? {}).map(([name, value]) => [name, localized(name, value)]),
  );
  const text = t(key, link === undefined ? params : { ...params, link });
  return text === key ? item[part] : text.trim();
}

/** The title in the language of the interface, or the one the notification came with. */
export function noticeTitle(item: AppNotification) {
  return translated(item, 'title');
}

/** The message as plain text, the words of its link included. */
export function noticeMessage(item: AppNotification) {
  return translated(item, 'message', linkLabel(item));
}

/** The message around its link, for a page that makes the words clickable; null without one. */
export function noticeParts(item: AppNotification): NoticeParts | null {
  const link = linkLabel(item);
  const target = item.key ? NOTICE_LINKS[item.key] : undefined;
  if (link === undefined || !target) {
    return null;
  }
  const [before = '', after] = translated(item, 'message', '\u0000').split('\u0000');
  return after === undefined ? null : { before, link, after, target };
}

/** Whether nas-log-manager has something to show for a notification: a warning or an error with a log file. */
export function hasLog(item: AppNotification) {
  return (item.tone === 'warning' || item.tone === 'error') && Boolean(item.log);
}

/**
 * The notifications shown on screen at the first sign-in after they were written, since nobody
 * was there to see them arrive: the end of the first configuration, for one.
 */
export const SIGN_IN_NOTICES: readonly string[] = ['discoveryOff'];

/** Who wrote a notification, by the name a person knows it by rather than its identifier. */
export function sourceLabel(source: string) {
  const key = `source_${source}`;
  const known = t(key);
  if (known !== key) {
    return known;
  }
  const installed = useAppsStore().installed.find((app) => app.id === source);
  return appLabel(source, installed?.name ?? source);
}
