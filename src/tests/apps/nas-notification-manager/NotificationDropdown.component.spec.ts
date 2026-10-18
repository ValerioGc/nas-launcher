import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, expect, it, vi } from 'vitest';
import NotificationDropdown from '@/apps/nas-notification-manager/components/NotificationDropdown.vue';
import { useNotificationsStore } from '@/apps/nas-notification-manager/store';
import { useShellStore } from '@/stores/shellStore';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset();
  useNotificationsStore().items = [
    {
      id: '1',
      title: 'Backup',
      message: 'Done',
      tone: 'success',
      at: 0,
      read: false,
      source: 'system',
    },
  ];
});

it('deletes through its icon without opening the app or marking the notification read', async () => {
  let finish!: () => void;
  vi.mocked(request).mockReturnValue(
    new Promise<void>((resolve) => {
      finish = resolve;
    }),
  );

  const shell = useShellStore();
  const open = vi.spyOn(shell, 'open');
  const wrapper = mount(NotificationDropdown);
  await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
  expect(wrapper.get('[data-testid="common_dropdown_content"]').classes()).toEqual(
    expect.arrayContaining(['common_dropdown_content_end', 'common_dropdown_content_wide']),
  );
  const remove = wrapper.get('[aria-label="Rimuovi Backup"]');
  expect(remove.text()).toBe('');
  expect(remove.attributes('title')).toBe('Rimuovi');
  expect(remove.find('[data-testid="common_icon"]').exists()).toBe(true);
  await remove.trigger('click');
  expect(remove.attributes('disabled')).toBeDefined();
  await remove.trigger('click');
  expect(request).toHaveBeenCalledExactlyOnceWith('/notifications/1', { method: 'DELETE' });
  finish();
  await flushPromises();
  expect(useNotificationsStore().unread).toBe(0);
  expect(wrapper.get('[data-testid="notification_empty"]').text()).toContain('Nessuna notifica');
  expect(wrapper.find('[aria-label="Rimuovi Backup"]').exists()).toBe(false);
  expect(open).not.toHaveBeenCalled();
  expect(wrapper.get('[data-testid="common_dropdown_trigger"]').attributes('aria-expanded')).toBe(
    'true',
  );
  wrapper.unmount();
});

it('keeps the notification and allows retrying after a failed delete', async () => {
  vi.mocked(request).mockRejectedValue(new Error('offline'));
  const wrapper = mount(NotificationDropdown);
  await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
  await wrapper.get('[aria-label="Rimuovi Backup"]').trigger('click');
  await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).not.toBe('');
  expect(wrapper.get('[aria-label="Rimuovi Backup"]').attributes('disabled')).toBeUndefined();
  expect(useNotificationsStore().unread).toBe(1);
  wrapper.unmount();
});

it('lists the first unread notifications and counts those it leaves out', async () => {
  const store = useNotificationsStore();
  store.items = Array.from({ length: 8 }, (_, index) => ({
    id: String(index),
    title: `Notice ${index}`,
    message: '',
    tone: 'info' as const,
    at: 0,
    read: false,
    source: 'system',
  }));
  const wrapper = mount(NotificationDropdown);
  await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');

  expect(wrapper.find('[data-testid="notification_menu_5"]').exists()).toBe(false);
  expect(wrapper.findAll('[data-testid^="notification_menu_"]')).toHaveLength(5);
  expect(wrapper.get('[data-testid="notification_open_all"]').text()).toContain('+3');
  wrapper.unmount();
});

it('opens the notification application and closes itself', async () => {
  const open = vi.spyOn(useShellStore(), 'open').mockImplementation(() => undefined);
  const wrapper = mount(NotificationDropdown);
  await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');

  await wrapper.get('[data-testid="notification_open_all"]').trigger('click');
  expect(open).toHaveBeenCalledWith('nas-notification-manager');
  expect(wrapper.find('[data-testid="common_dropdown_content"]').exists()).toBe(false);
  wrapper.unmount();
});
