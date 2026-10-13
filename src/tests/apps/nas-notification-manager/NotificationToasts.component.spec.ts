import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import NotificationToasts from '@/apps/nas-notification-manager/components/NotificationToasts.vue';
import { useNotificationsStore } from '@/apps/nas-notification-manager/store';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('en');
});

describe('NotificationToasts', () => {
  it('shows each toast with its tone, title and message', () => {
    const store = useNotificationsStore();
    store.toast({ id: 'a', tone: 'error', title: 'Backend down', message: 'Try again' });
    const wrapper = mount(NotificationToasts);

    const toast = wrapper.get('[data-testid="notification_toast_a"]');
    expect(toast.get('[role="alert"]').text()).toContain('Backend down');
    expect(toast.text()).toContain('Try again');
  });

  it('closes a toast from its close button', async () => {
    const store = useNotificationsStore();
    store.toast({ id: 'a', tone: 'info', title: 'Saved' });
    const wrapper = mount(NotificationToasts);

    await wrapper.get('[data-testid="alert_banner_close"]').trigger('click');
    expect(store.toasts).toEqual([]);
  });

  it('runs its action and goes away', async () => {
    const store = useNotificationsStore();
    const run = vi.fn();
    store.toast({ id: 'a', tone: 'info', title: 'New', action: { label: 'View', run } });
    const wrapper = mount(NotificationToasts);

    await wrapper.get('button:not([data-testid="alert_banner_close"])').trigger('click');
    expect(run).toHaveBeenCalledOnce();
    expect(store.toasts).toEqual([]);
  });

  it('counts down the time a toast stays with a bar that ends it', async () => {
    const store = useNotificationsStore();
    store.toast({ id: 'a', tone: 'warning', title: 'Update', duration: 12000 });
    store.toast({ id: 'b', tone: 'error', title: 'Backend down' });
    const wrapper = mount(NotificationToasts);

    const timer = wrapper.get('[data-testid="notification_toast_timer_a"]');
    expect(timer.attributes('style')).toContain('animation-duration: 12000ms');
    expect(timer.classes()).toContain('notification_toast_timer_warning');
    expect(wrapper.find('[data-testid="notification_toast_timer_b"]').exists()).toBe(false);

    await timer.trigger('animationend');
    expect(store.toasts.map((toast) => toast.id)).toEqual(['b']);
  });

  it('opens the place its message names and goes away', async () => {
    const store = useNotificationsStore();
    const notification = {
      id: '8',
      title: 'Network discovery off',
      message: 'Off',
      tone: 'info' as const,
      at: 0,
      read: false,
      source: 'nas-security-center',
      key: 'discoveryOff',
    };
    vi.mocked(request).mockResolvedValue(undefined);
    useAppsStore().installed = [
      { id: 'nas-security-center', name: 'Security Center', info: '', system: false },
    ];
    store.toast({ id: '8', tone: 'info', title: 'Network discovery off', notification });
    const wrapper = mount(NotificationToasts);

    const toast = wrapper.get('[data-testid="notification_toast_8"]');
    expect(toast.text()).toContain('Security Center, Access section');
    await wrapper.get('[data-testid="notice_link_8"]').trigger('click');
    expect(useShellStore().locations['nas-security-center']).toBe('?section=access');
    expect(store.toasts).toEqual([]);
  });
});
