import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { hasLog, noticeMessage, noticeParts } from '@/apps/nas-notification-manager/text';
import { setLocale } from '@/i18n';
import type { AppNotification } from '@/apps/nas-notification-manager/types';

const notice = (fields: Partial<AppNotification>): AppNotification => ({
  id: '1',
  title: 'Title',
  message: 'As written',
  tone: 'info',
  at: 0,
  read: false,
  source: 'system',
  ...fields,
});

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('en');
});

describe('the text of a notification', () => {
  it('splits the message around the words that open the place it names', () => {
    const parts = noticeParts(notice({ key: 'discoveryOff' }));
    expect(parts?.link).toBe('Security Center, Access section');
    expect(parts?.before).toContain('can be changed in ');
    expect(parts?.after).toBe('.');
    expect(parts?.target).toEqual({ app: 'nas-security-center', location: '?section=access' });
    expect(noticeMessage(notice({ key: 'discoveryOff' }))).toContain(
      'in Security Center, Access section.',
    );
  });

  it('has no link for a notification without a known key', () => {
    expect(noticeParts(notice({}))).toBeNull();
    expect(noticeParts(notice({ key: 'settingsSaved' }))).toBeNull();
    expect(noticeParts(notice({ key: 'toString' }))).toBeNull();
  });

  it('leaves the log to the warnings and errors', () => {
    expect(hasLog(notice({ tone: 'info' }))).toBe(false);
    expect(hasLog(notice({ tone: 'success' }))).toBe(false);
    expect(hasLog(notice({ tone: 'warning' }))).toBe(true);
    expect(hasLog(notice({ tone: 'error' }))).toBe(true);
  });
});
