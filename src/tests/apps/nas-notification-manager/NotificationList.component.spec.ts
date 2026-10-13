import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import NotificationList from '@/apps/nas-notification-manager/components/NotificationList.vue';
import { useNotificationsStore } from '@/apps/nas-notification-manager/store';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const backup = {
  id: '1',
  title: 'Backup',
  message: 'Copied 3 folders',
  tone: 'success' as const,
  at: 0,
  read: false,
  source: 'backup',
};

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('en');
  const list = [backup, { ...backup, id: '2', read: true }];
  vi.mocked(request)
    .mockReset()
    .mockImplementation(async (path) => (path.startsWith('/notifications?') ? list : []) as never);
  useNotificationsStore().data = list;
});

describe('NotificationList', () => {
  it('lays out a card with its origin in the heading and its actions beside the text', () => {
    const wrapper = mount(NotificationList);
    const card = wrapper.get('[data-testid="notification_1"]');

    expect(card.get('.notification_header strong').text()).toBe('Backup');
    expect(card.get('.notification_header .notification_meta').text()).toContain('backup');
    const content = card.get('.notification_content');
    expect(content.get('p').text()).toBe('Copied 3 folders');
    expect(content.element.lastElementChild?.classList).toContain('notification_actions');
  });

  it('offers to mark as read only what is unread', () => {
    const wrapper = mount(NotificationList);

    expect(wrapper.find('[aria-label="Mark as read Backup"]').exists()).toBe(true);
    expect(wrapper.get('[data-testid="notification_2"]').findAll('button')).toHaveLength(2);
  });

  it('translates what the launcher wrote, and names its source as a person knows it', () => {
    setLocale('it');
    useNotificationsStore().data = [
      {
        ...backup,
        id: '3',
        source: 'nas-system-settings',
        title: 'Launcher update interrupted',
        message: 'The NAS went down during the installing step.',
        key: 'launcherUpdateInterrupted',
        params: { step: 'installing', version: '0.1.2' },
      },
      { ...backup, id: '4', source: 'system', key: 'somethingUnknown' },
    ];
    const wrapper = mount(NotificationList);

    const updated = wrapper.get('[data-testid="notification_3"]');
    expect(updated.get('strong').text()).toBe('Aggiornamento del launcher interrotto');
    expect(updated.get('.notification_content p').text()).toBe(
      'Aggiornamento interrotto durante la fase di installazione. Versione in uso 0.1.2.',
    );
    expect(updated.get('[data-testid="notification_source_3"]').text()).toBe('Impostazioni');
    const unknown = wrapper.get('[data-testid="notification_4"]');
    expect(unknown.get('strong').text()).toBe('Backup');
    expect(unknown.get('[data-testid="notification_source_4"]').text()).toBe('Sistema');
  });

  it('offers the log only for the warnings and errors, which nas-log-manager holds', () => {
    useNotificationsStore().data = [
      { ...backup, tone: 'info' },
      { ...backup, id: '2', tone: 'warning' },
      { ...backup, id: '3', tone: 'error' },
    ];
    const wrapper = mount(NotificationList);

    expect(wrapper.find('[data-testid="notification_log_1"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="notification_log_2"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="notification_log_3"]').exists()).toBe(true);
  });

  it('opens the place a notification names from the words of its message', async () => {
    setLocale('it');
    useNotificationsStore().data = [
      { ...backup, id: '5', tone: 'info', key: 'discoveryOff', source: 'nas-security-center' },
    ];
    useAppsStore().installed = [
      { id: 'nas-security-center', name: 'Security Center', info: '', system: false },
    ];
    const wrapper = mount(NotificationList);
    const message = wrapper.get('[data-testid="notification_5"] .notification_content p');
    expect(message.text()).toContain('modificabile da Security Center, sezione Accesso.');

    await wrapper.get('[data-testid="notice_link_5"]').trigger('click');
    const shell = useShellStore();
    expect(shell.locations['nas-security-center']).toBe('?section=access');
  });

  it('opens nas-log-manager on the notification, and only once it is installed', async () => {
    useNotificationsStore().data = [{ ...backup, tone: 'warning', log: '/var/log/backup.log' }];
    const wrapper = mount(NotificationList);
    const log = () => wrapper.get('[data-testid="notification_log_1"]');
    expect(log().attributes('disabled')).toBeDefined();
    expect(log().attributes('title')).toContain('Log');

    useAppsStore().installed = [{ id: 'nas-log-manager', name: 'Log', info: '', system: false }];
    await flushPromises();
    await log().trigger('click');
    const shell = useShellStore();
    expect(shell.activeId).toBe('nas-log-manager');
    expect(shell.locations['nas-log-manager']).toBe('?notification=1');
  });

  it('marks as read, removes, and filters by the source of a card', async () => {
    const store = useNotificationsStore();
    const wrapper = mount(NotificationList);

    await wrapper.get('[aria-label="Mark as read Backup"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/notifications/1', {
      method: 'PATCH',
      body: JSON.stringify({ read: true }),
    });

    await wrapper
      .get('[data-testid="notification_1"] [aria-label="Remove Backup"]')
      .trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/notifications/1', { method: 'DELETE', body: undefined });

    await wrapper.get('[data-testid="notification_1"] .notification_source').trigger('click');
    expect(store.source).toBe('backup');
  });
});
