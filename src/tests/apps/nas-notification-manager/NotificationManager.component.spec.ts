import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import NotificationManager from '@/apps/nas-notification-manager/NotificationManager.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { AppNotification } from '@/apps/nas-notification-manager/types';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const backup: AppNotification = {
  id: '1',
  title: 'Backup',
  message: 'Done',
  tone: 'success',
  source: 'nas-backup-manager',
  at: 0,
  read: false,
};

function answer(items: AppNotification[] = [], sources: string[] = []) {
  vi.mocked(request).mockImplementation((path: string) =>
    Promise.resolve((path.startsWith('/notifications/sources') ? sources : items) as never),
  );
}

function lastQuery() {
  return vi
    .mocked(request)
    .mock.calls.map(([path]) => path)
    .filter((path) => path.startsWith('/notifications?'))
    .at(-1);
}

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset();
  answer();
});

describe('notification manager', () => {
  it('shows the empty state and disables actions and filters when the log is empty', async () => {
    const wrapper = mount(NotificationManager);
    await flushPromises();
    expect(wrapper.get('[data-testid="notifications_empty"]').text()).toContain('Nessuna notifica');
    for (const id of [
      'tone_all',
      'tone_error',
      'source',
      'query',
      'unread_input',
      'read_all',
      'clear_read',
      'refresh',
      'reset',
    ]) {
      expect(
        wrapper.get(`[data-testid="notifications_${id}"]`).attributes('disabled'),
      ).toBeDefined();
    }
    expect(wrapper.find('[data-testid="notifications_settings"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('keeps filters editable when the current filter has no matching notifications', async () => {
    answer([], ['system']);
    const wrapper = mount(NotificationManager);
    await flushPromises();
    await wrapper.get('[data-testid="notifications_tone_warning"]').trigger('click');
    await flushPromises();
    expect(lastQuery()).toBe('/notifications?limit=500&tone=warning');
    const warning = wrapper.get('[data-testid="notifications_tone_warning"]');
    expect(warning.attributes('aria-pressed')).toBe('true');
    expect(warning.attributes('disabled')).toBeUndefined();
    expect(wrapper.get('[data-testid="notifications_tone_all"]').attributes('aria-pressed')).toBe(
      'false',
    );
    expect(wrapper.get('[data-testid="notifications_empty"]').text()).toContain(
      'Nessuna notifica trovata',
    );
    expect(wrapper.find('[data-testid="notifications_matches"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('applies the source filter without a button to press', async () => {
    answer([], ['nas-app-manager', 'system']);
    const wrapper = mount(NotificationManager);
    await flushPromises();

    const source = wrapper.get('[data-testid="notifications_source"]');
    expect(source.text()).toContain('Gestore applicazioni');
    expect(source.text()).toContain('Sistema');
    expect(wrapper.find('button[type="submit"]').exists()).toBe(false);

    await source.setValue('system');
    await flushPromises();
    expect(lastQuery()).toBe('/notifications?limit=500&source=system');
    wrapper.unmount();
  });

  it('opens again without the filters of the last time it was closed', async () => {
    answer([backup], ['nas-backup-manager']);
    const first = mount(NotificationManager);
    await flushPromises();
    await first.get('[data-testid="notifications_source"]').setValue('nas-backup-manager');
    await flushPromises();
    first.unmount();

    const again = mount(NotificationManager);
    await flushPromises();
    expect(
      (again.get('[data-testid="notifications_source"]').element as HTMLSelectElement).value,
    ).toBe('');
    expect(lastQuery()).toBe('/notifications?limit=500');
    again.unmount();
  });

  it('clears every filter at once and says how many rows matched', async () => {
    answer([backup], ['nas-backup-manager']);
    const wrapper = mount(NotificationManager);
    await flushPromises();

    const reset = () => wrapper.get('[data-testid="notifications_reset"]');
    expect(reset().attributes('disabled')).toBeDefined();

    await wrapper.get('[data-testid="notifications_unread_input"]').setValue(true);
    await wrapper.get('[data-testid="notifications_tone_success"]').trigger('click');
    await flushPromises();
    expect(lastQuery()).toBe('/notifications?limit=500&tone=success&read=false');
    expect(wrapper.get('[data-testid="notifications_matches"]').text()).toContain('1 notifiche');

    await reset().trigger('click');
    await flushPromises();
    expect(lastQuery()).toBe('/notifications?limit=500');
    expect(reset().attributes('disabled')).toBeDefined();
    expect(wrapper.get('[data-testid="notifications_tone_all"]').attributes('aria-pressed')).toBe(
      'true',
    );
    wrapper.unmount();
  });

  it('searches the text as the page shows it, without asking the launcher again', async () => {
    answer(
      [
        { ...backup, key: 'networkWired', title: 'Network changed' },
        { ...backup, id: '9', title: 'Backup', message: 'Copied' },
      ],
      [],
    );
    const wrapper = mount(NotificationManager);
    await flushPromises();
    const asked = vi.mocked(request).mock.calls.length;

    await wrapper.get('[data-testid="notifications_query"]').setValue('cavo');
    await flushPromises();
    expect(vi.mocked(request).mock.calls).toHaveLength(asked);
    expect(wrapper.findAll('.notification_row')).toHaveLength(1);
    expect(wrapper.get('[data-testid="notifications_matches"]').text()).toContain('1 notifiche');
    wrapper.unmount();
  });

  it('filters by the source written on a row', async () => {
    answer([backup], ['nas-backup-manager']);
    const wrapper = mount(NotificationManager);
    await flushPromises();
    await wrapper.get('[title="Mostra solo nas-backup-manager"]').trigger('click');
    await flushPromises();
    expect(lastQuery()).toBe('/notifications?limit=500&source=nas-backup-manager');
    wrapper.unmount();
  });

  it('marks a notification read through the API', async () => {
    answer([backup]);
    const wrapper = mount(NotificationManager);
    await flushPromises();
    const row = wrapper.get('[data-testid="notification_1"]');
    const read = row.get('[aria-label="Segna come letta Backup"]');
    expect(read.text()).toBe('');
    await read.trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/notifications/1', {
      method: 'PATCH',
      body: '{"read":true}',
    });
    wrapper.unmount();
  });

  it('removes a notification from its own row', async () => {
    answer([{ ...backup, read: true }]);
    const wrapper = mount(NotificationManager);
    await flushPromises();
    await wrapper.get('[aria-label="Rimuovi Backup"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/notifications/1', { method: 'DELETE', body: undefined });
    wrapper.unmount();
  });

  it('offers the actions that work on the whole list', async () => {
    answer([{ ...backup, id: '2', title: 'Copia', tone: 'info', source: 'system' }]);
    const wrapper = mount(NotificationManager);
    await flushPromises();
    await wrapper.get('[data-testid="notifications_read_all"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/notifications/read', {
      method: 'POST',
      body: undefined,
    });
    await wrapper.get('[data-testid="notifications_clear_read"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/notifications/read/clear', {
      method: 'POST',
      body: undefined,
    });
    wrapper.unmount();
  });
});
