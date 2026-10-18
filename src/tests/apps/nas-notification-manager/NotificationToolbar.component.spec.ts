import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import NotificationToolbar from '@/apps/nas-notification-manager/components/NotificationToolbar.vue';
import { useNotificationsStore } from '@/apps/nas-notification-manager/store';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { AppNotification } from '@/apps/nas-notification-manager/types';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const notification = (id: string, read: boolean): AppNotification => ({
  id,
  title: 'Backup',
  message: 'Done',
  tone: 'info',
  at: 0,
  read,
  source: 'system',
});

async function mountWith(list: AppNotification[]) {
  vi.mocked(request).mockResolvedValue(list);
  await useNotificationsStore().load();
  return mount(NotificationToolbar);
}

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('en');
  vi.mocked(request).mockReset();
});

describe('NotificationToolbar', () => {
  it('counts the unread notifications and offers to read them all', async () => {
    const wrapper = await mountWith([notification('1', false), notification('2', true)]);

    expect(wrapper.get('.notifications_count').text()).toBe('1');
    await wrapper.get('[data-testid="notifications_read_all"]').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/notifications/read', {
      method: 'POST',
      body: undefined,
    });
  });

  it('has nothing to mark when everything is read', async () => {
    const wrapper = await mountWith([notification('1', true)]);

    expect(wrapper.find('.notifications_count').exists()).toBe(false);
    expect(
      wrapper.get('[data-testid="notifications_read_all"]').attributes('disabled'),
    ).toBeDefined();
  });

  it('clears the read notifications and reloads on request', async () => {
    const wrapper = await mountWith([notification('1', true)]);

    await wrapper.get('[data-testid="notifications_clear_read"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/notifications/read/clear', {
      method: 'POST',
      body: undefined,
    });

    vi.mocked(request).mockClear();
    await wrapper.get('[data-testid="notifications_refresh"]').trigger('click');
    expect(request).toHaveBeenCalledWith(expect.stringMatching(/^\/notifications\?/));
  });

  it('removes the delivery settings action', async () => {
    const wrapper = await mountWith([]);

    expect(wrapper.find('[data-testid="notifications_settings"]').exists()).toBe(false);
  });
});
