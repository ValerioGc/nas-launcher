import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import MainPage from '@/views/MainPage.vue';
import { useShellStore } from '@/stores/shellStore';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const manager = { id: 'nas-app-manager', name: 'Apps', info: '', system: true };

const search = { id: 'nas-global-search', name: 'Ricerca', info: '', system: true };

const monitor = { id: 'nas-system-monitor', name: 'Monitor', info: '', system: false };

const general = {
  nasName: 'nas',
  theme: 'system',
  language: 'it',
  createShortcut: false,
  desktopIconSize: 'large',
  accent: '',
  background: {
    mode: 'gradient',
    image: '/wallpapers/landscape.svg',
    blur: 0,
    intensity: 35,
    color: '#258ac0',
    direction: 135,
    pattern: 'multiple',
    size: 60,
  },
};

/**
 * Answers the requests for applications and general settings with predefined data.
 */
function answer(apps: unknown[] = [manager, search, monitor], notifications: unknown[] = []) {
  vi.mocked(request).mockImplementation((path: string) => {
    if (path === '/apps') {
      return Promise.resolve(apps as never);
    }

    if (path.startsWith('/notifications')) {
      return Promise.resolve(notifications as never);
    }

    return Promise.resolve((path === '/settings/general' ? general : []) as never);
  });
}

async function open() {
  const wrapper = mount(MainPage, { attachTo: document.body });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe = vi.fn();
      disconnect = vi.fn();
    },
  );
  sessionStorage.clear();
  setActivePinia(createPinia());
  setLocale('it');

  vi.stubGlobal('matchMedia', () => ({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));

  vi.useFakeTimers();
  vi.mocked(request).mockReset();
  answer();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

describe('launcher shell', () => {
  it('reads what it needs once, and shows the desktop', async () => {
    const wrapper = await open();

    expect(request).toHaveBeenCalledWith('/apps');
    expect(request).toHaveBeenCalledWith('/desktop/shortcuts');
    expect(request).toHaveBeenCalledWith('/settings/general');
    expect(wrapper.find('[data-testid="app_host"]').isVisible()).toBe(false);
    expect(wrapper.find('[data-testid="app_tabs"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('opens the application menu and the application that was picked', async () => {
    const wrapper = await open();
    const shell = useShellStore();

    await wrapper.get('[data-testid="header_menu_toggle"]').trigger('click');
    expect(wrapper.find('[data-testid="app_menu"]').exists()).toBe(true);

    await wrapper.get('[data-testid="app_menu_app_nas-app-manager"]').trigger('click');
    expect(shell.activeId).toBe('nas-app-manager');
    await flushPromises();
    expect(wrapper.find('[data-testid="app_tabs"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('keeps one drawer at a time, and closes it when a window opens', async () => {
    const wrapper = await open();

    await wrapper.get('[data-testid="header_process_toggle"]').trigger('click');
    expect(wrapper.find('[data-testid="process_panel"]').exists()).toBe(true);

    const taskToggle = wrapper.get('[data-testid="header_task_toggle"]');
    (taskToggle.element as HTMLElement).focus();
    await taskToggle.trigger('pointerdown');
    await taskToggle.trigger('click');
    expect(wrapper.find('[data-testid="process_panel"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="task_panel"]').exists()).toBe(true);

    await wrapper.get('[data-testid="task_panel"]').trigger('keydown', { key: 'Escape' });
    expect(document.activeElement).toBe(taskToggle.element);

    await taskToggle.trigger('click');

    useShellStore().open('nas-app-manager');
    await flushPromises();
    expect(wrapper.find('[data-testid="task_panel"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('closes a widget on an outside click and still toggles its header control', async () => {
    const wrapper = await open();

    await wrapper.get('[data-testid="header_process_toggle"]').trigger('click');
    expect(wrapper.find('[data-testid="process_panel"]').exists()).toBe(true);
    await wrapper.get('[data-testid="widget_drawer_backdrop"]').trigger('click');
    expect(wrapper.find('[data-testid="process_panel"]').exists()).toBe(false);

    await wrapper.get('[data-testid="header_process_toggle"]').trigger('click');
    await wrapper.get('[data-testid="header_process_toggle"]').trigger('pointerdown');
    await wrapper.get('[data-testid="header_process_toggle"]').trigger('click');
    expect(wrapper.find('[data-testid="process_panel"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('opens the search with what was typed in the header', async () => {
    const wrapper = await open();
    await wrapper.get('[data-testid="header_search_input"]').setValue('foto');
    await wrapper.get('form[role="search"]').trigger('submit');

    const shell = useShellStore();
    expect(shell.modalId).toBe('nas-global-search');
    expect(shell.launchQuery).toBe('foto');
    wrapper.unmount();
  });

  it('refreshes the notifications on a mutation and on its own timer', async () => {
    const wrapper = await open();
    const before = vi
      .mocked(request)
      .mock.calls.filter(([path]) => path.startsWith('/notifications')).length;

    answer(
      [manager, search],
      [
        {
          id: '12',
          title: 'Settings saved',
          message: 'The general settings were changed.',
          source: 'nas-system-settings',
          tone: 'success',
          at: 1,
          read: false,
          key: 'settingsSaved',
          params: { section: 'general' },
        },
      ],
    );
    window.dispatchEvent(new CustomEvent('nas:mutation', { detail: '/settings/general' }));
    await flushPromises();
    expect(wrapper.get('[data-testid="notification_toast_12"]').text()).toContain(
      'Impostazioni salvate',
    );
    await vi.advanceTimersByTimeAsync(15_000);
    await flushPromises();

    const after = vi
      .mocked(request)
      .mock.calls.filter(([path]) => path.startsWith('/notifications')).length;
    expect(after).toBeGreaterThan(before);
    wrapper.unmount();
  });

  it('stops its timer and its listener when it goes away', async () => {
    const wrapper = await open();
    wrapper.unmount();
    await vi.runOnlyPendingTimersAsync();

    expect(vi.getTimerCount()).toBe(0);
  });

  it('reports a register it could not read as a toast with a retry', async () => {
    vi.mocked(request).mockRejectedValue(new Error('down'));
    const wrapper = await open();

    const apps = wrapper.get('[data-testid="notification_toast_apps"]');
    expect(apps.text()).toContain('Applicazioni non disponibili');
    expect(wrapper.get('[data-testid="notification_toast_settings"]').text()).toContain(
      'Impostazioni non disponibili',
    );

    vi.mocked(request).mockResolvedValue([]);
    await apps.get('button:not([data-testid="alert_banner_close"])').trigger('click');
    await flushPromises();
    expect(wrapper.find('[data-testid="notification_toast_apps"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
