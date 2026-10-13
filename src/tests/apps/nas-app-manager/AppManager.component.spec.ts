import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import AppManager from '@/apps/nas-app-manager/AppManager.vue';
import { useAppsStore } from '@/stores/appsStore';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const systemApp = {
  id: 'nas-system-settings',
  name: 'Settings',
  info: '',
  system: true,
  version: '1.0.0',
};

const optionalApp = {
  id: 'mediaServer',
  name: 'Media',
  info: '',
  system: false,
  version: '1.0.0',
  running: false,
};

const offered = {
  id: 'mediaServer',
  name: 'Media',
  info: '',
  repository: 'example/media-server',
  installedVersion: null,
  latestVersion: null,
  updateAvailable: false,
};

function answer(rows: unknown[], configured = true) {
  vi.mocked(request)
    .mockReset()
    .mockImplementation((path: string) => {
      if (path === '/apps/credentials') {
        return Promise.resolve({
          configured,
          packagesEnabled: true,
          appsManaged: true,
        } as never);
      }
      return Promise.resolve(rows as never);
    });
}

function paths() {
  return vi
    .mocked(request)
    .mock.calls.map(([path]) => path)
    .filter((path) => !['/apps/credentials', '/tasks', '/settings/apps'].includes(path));
}

describe('application management', () => {
  beforeEach(() => setLocale('it'));
  it('protects system apps in the UI and confirms installs before calling the API', async () => {
    setActivePinia(createPinia());
    vi.mocked(request)
      .mockReset()
      .mockImplementation((path: string) => {
        if (path === '/apps/credentials') {
          return Promise.resolve({ configured: true, packagesEnabled: true } as never);
        }
        return Promise.resolve((path === '/apps/available' ? [offered] : [systemApp]) as never);
      });
    const wrapper = mount(AppManager, { attachTo: document.body });
    await flushPromises();
    const system = wrapper.get('[data-testid="app_manager_app_nas-system-settings"]');
    expect(system.text()).not.toContain('Di sistema');
    expect(system.attributes('aria-disabled')).toBe('true');
    await system.trigger('click');
    expect(wrapper.find('[data-testid="app_manager_actions"]').exists()).toBe(false);
    await wrapper
      .findAll('button')
      .find((b) => b.text() === 'Disponibili')!
      .trigger('click');
    await flushPromises();
    await wrapper
      .findAll('button')
      .find((b) => b.text() === 'Media')!
      .trigger('click');
    await wrapper
      .findAll('button')
      .find((b) => b.text() === 'Installa')!
      .trigger('click');
    expect(paths().every((path) => path.startsWith('/apps'))).toBe(true);
    document.querySelector<HTMLButtonElement>('.common_modal_footer button:last-child')!.click();
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/apps/mediaServer/install', {
      method: 'POST',
      body: undefined,
    });
    wrapper.unmount();
  });

  it('hides the system apps on request and keeps the desktop shortcut on the context menu', async () => {
    setActivePinia(createPinia());
    answer([systemApp, { ...optionalApp, system: false }]);
    const wrapper = mount(AppManager, { attachTo: document.body });
    await flushPromises();
    const rows = () => wrapper.findAll('[data-testid^="app_manager_app_"]');
    expect(rows()).toHaveLength(2);
    const toggle = wrapper.get('[data-testid="app_manager_show_system_input"]');
    expect((toggle.element as HTMLInputElement).checked).toBe(true);
    await toggle.setValue(false);
    expect(rows()).toHaveLength(1);
    expect(wrapper.text()).not.toContain('Di sistema');
    await toggle.setValue(true);
    await wrapper.get('[data-testid="app_manager_app_nas-system-settings"]').trigger('contextmenu');
    await flushPromises();
    const item = document.querySelector<HTMLButtonElement>('[role="menuitem"]')!;
    expect(item.textContent?.trim()).toBe('Aggiungi al desktop');
    item.click();
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/desktop/shortcuts/nas-system-settings', {
      method: 'POST',
      body: undefined,
    });

    wrapper.unmount();
  });

  it('offers to remove the shortcut of an application already on the desktop', async () => {
    setActivePinia(createPinia());
    vi.mocked(request)
      .mockReset()
      .mockResolvedValueOnce([
        { kind: 'app', target: 'nas-system-settings', name: null, position: 0 },
      ])
      .mockResolvedValue([systemApp]);
    const store = useAppsStore();
    store.installed = [{ ...systemApp }];
    await store.shortcut('globalSettings');
    const wrapper = mount(AppManager, { attachTo: document.body });
    await flushPromises();
    await wrapper.get('[data-testid="app_manager_app_nas-system-settings"]').trigger('contextmenu');
    await flushPromises();
    const item = document.querySelector<HTMLButtonElement>('[data-testid="app_manager_shortcut"]')!;
    expect(item.textContent?.trim()).toBe('Rimuovi collegamento');
    item.click();
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/desktop/shortcuts/nas-system-settings', {
      method: 'DELETE',
      body: undefined,
    });
    wrapper.unmount();
  });

  it('starts from the preview, shows commands only for a selection and keeps it while switching view', async () => {
    setActivePinia(createPinia());
    answer([optionalApp]);
    const wrapper = mount(AppManager);
    await flushPromises();
    const actions = () => wrapper.find('[data-testid="app_manager_actions"]');
    const catalog = () => wrapper.get('[data-testid="app_manager_catalog"]');
    expect(actions().exists()).toBe(false);
    expect(catalog().classes()).toContain('manager_preview');
    await catalog().get('button').trigger('click');
    expect(actions().exists()).toBe(true);
    expect(actions().text()).toContain('Disinstalla');
    const list = wrapper.get('[data-testid="app_manager_view_list"]');
    const preview = wrapper.get('[data-testid="app_manager_view_preview"]');
    expect(preview.attributes('aria-pressed')).toBe('true');
    expect(preview.text()).toBe('');
    expect(preview.attributes('aria-label')).toBe('Anteprima');
    expect(preview.find('[data-testid="common_icon"]').exists()).toBe(true);
    expect(list.attributes('aria-label')).toBe('Lista');
    expect(catalog().get('img').attributes('src')).toBeTruthy();
    expect(catalog().get('button').text()).toContain('Media');
    await list.trigger('click');
    expect(list.attributes('aria-pressed')).toBe('true');
    expect(preview.attributes('aria-pressed')).toBe('false');
    expect(catalog().classes()).not.toContain('manager_preview');
    expect(catalog().find('thead').exists()).toBe(true);
    expect(
      catalog().get('[data-testid="app_manager_app_mediaServer"]').attributes('aria-pressed'),
    ).toBe('true');
    await preview.trigger('click');
    expect(catalog().classes()).toContain('manager_preview');
    expect(paths()).toEqual(['/apps/installed']);
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Disponibili')!
      .trigger('click');
    await flushPromises();
    expect(actions().exists()).toBe(false);
    expect(paths().at(-1)).toBe('/apps/available');
    wrapper.unmount();
  });

  it('leaves the desktop shortcut to the backend that installs the application', async () => {
    setActivePinia(createPinia());
    answer([offered]);
    const wrapper = mount(AppManager, { attachTo: document.body });
    await flushPromises();
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Disponibili')!
      .trigger('click');
    await flushPromises();
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Media')!
      .trigger('click');
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Installa')!
      .trigger('click');
    document.querySelector<HTMLButtonElement>('.common_modal_footer button:last-child')!.click();
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/apps/mediaServer/install', {
      method: 'POST',
      body: undefined,
    });
    expect(
      vi.mocked(request).mock.calls.filter(([path]) => path.startsWith('/desktop/shortcuts/')),
    ).toHaveLength(0);
    wrapper.unmount();
  });

  it('sorts the list by the column that was clicked', async () => {
    setActivePinia(createPinia());
    answer([
      { id: 'zeta', name: 'Zeta', info: 'ultima', system: false, version: '2.0.0' },
      { id: 'alfa', name: 'Alfa', info: 'prima', system: false, version: '1.0.0' },
    ]);
    const wrapper = mount(AppManager);
    await flushPromises();
    await wrapper.get('[data-testid="app_manager_view_list"]').trigger('click');

    const names = () => wrapper.findAll('tbody tr').map((row) => row.text());
    expect(names()[0]).toContain('Alfa');

    const header = wrapper.get('[data-testid="app_manager_sort_name"]');
    await header.trigger('click');
    expect(names()[0]).toContain('Zeta');
    expect(header.element.closest('th')?.getAttribute('aria-sort')).toBe('descending');

    await wrapper.get('[data-testid="app_manager_sort_info"]').trigger('click');
    expect(names()[0]).toContain('Alfa');
    expect(wrapper.get('[data-testid="app_manager_version_alfa"]').text()).toContain('1.0.0');
    wrapper.unmount();
  });

  it('reaches the repositories only when the check is pressed', async () => {
    setActivePinia(createPinia());
    answer([optionalApp]);
    const wrapper = mount(AppManager);
    await flushPromises();
    expect(paths()).toEqual(['/apps/installed']);

    vi.mocked(request).mockImplementation((path: string) =>
      Promise.resolve(
        (path === '/apps/updates'
          ? [
              {
                ...offered,
                installedVersion: '1.0.0',
                latestVersion: '1.4.0',
                updateAvailable: true,
              },
            ]
          : path === '/apps/credentials'
            ? { configured: true, packagesEnabled: true }
            : [optionalApp]) as never,
      ),
    );
    await wrapper.get('[data-testid="app_manager_check"]').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/apps/updates', { method: 'POST', body: '{}' });
    expect(wrapper.find('[data-testid="app_manager_updates"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="app_manager_update_available"]').text()).toContain(
      'Aggiorna tutte',
    );
    expect(wrapper.get('[data-testid="app_manager_badge_mediaServer"]').text()).toContain('1.4.0');

    await wrapper.get('[data-testid="app_manager_app_mediaServer"]').trigger('click');
    expect(wrapper.get('[data-testid="app_manager_update_available"]').text()).toContain(
      'Aggiorna selezionata',
    );
    wrapper.unmount();
  });

  it('says so where this installation fetches nothing', async () => {
    setActivePinia(createPinia());
    vi.mocked(request)
      .mockReset()
      .mockImplementation((path: string) =>
        Promise.resolve(
          (path === '/apps/credentials'
            ? { configured: true, packagesEnabled: false }
            : [optionalApp]) as never,
        ),
      );
    const wrapper = mount(AppManager);
    await flushPromises();

    expect(wrapper.get('[data-testid="app_manager_check"]').attributes('disabled')).toBeDefined();
    expect(wrapper.get('[data-testid="app_manager_offline"]').text()).toContain(
      'Download dei pacchetti non attivo',
    );
    expect(wrapper.find('[data-testid="app_manager_token_status"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('says the token is missing and where it is set, with no field on the page', async () => {
    setActivePinia(createPinia());
    answer([optionalApp], false);
    const wrapper = mount(AppManager);
    await flushPromises();
    const status = wrapper.get('[data-testid="app_manager_token_status"]');
    expect(status.attributes('role')).toBe('status');
    expect(status.text()).toContain('impostazioni del gestore');
    expect(wrapper.find('input[type="password"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('says where to follow an installation it has just started', async () => {
    setActivePinia(createPinia());
    vi.mocked(request)
      .mockReset()
      .mockImplementation((path: string) =>
        Promise.resolve(
          (path === '/apps/credentials'
            ? { configured: true, packagesEnabled: true }
            : path === '/apps/available'
              ? [offered]
              : []) as never,
        ),
      );
    const wrapper = mount(AppManager, { attachTo: document.body });
    await flushPromises();
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Disponibili')!
      .trigger('click');
    await flushPromises();
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Media')!
      .trigger('click');
    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Installa')!
      .trigger('click');
    document.querySelector<HTMLButtonElement>('.common_modal_footer button:last-child')!.click();
    await flushPromises();

    expect(wrapper.find('[data-testid="app_manager_started"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it.each(['list', 'preview'] as const)(
    'keeps process controls and status out of the %s view',
    async (view) => {
      setActivePinia(createPinia());
      answer([systemApp, { ...optionalApp, running: true }]);
      const wrapper = mount(AppManager);
      await flushPromises();
      if (view === 'list') {
        await wrapper.get('[data-testid="app_manager_view_list"]').trigger('click');
      }
      await wrapper.get('[data-testid="app_manager_app_mediaServer"]').trigger('click');

      expect(wrapper.get('[data-testid="app_manager_actions"]').text()).toContain('Disinstalla');
      expect(wrapper.find('[data-testid="app_manager_power"]').exists()).toBe(false);
      expect(wrapper.find('[data-testid="app_manager_state_mediaServer"]').exists()).toBe(false);
      expect(
        wrapper.get('[data-testid="app_manager_app_nas-system-settings"]').text(),
      ).not.toContain('Di sistema');
      wrapper.unmount();
    },
  );

  it('says why the board is empty, inside the board', async () => {
    setActivePinia(createPinia());
    answer([]);
    const wrapper = mount(AppManager);
    await flushPromises();

    const board = wrapper.get('[data-testid="app_manager_catalog"]');
    const empty = wrapper.get('[data-testid="app_manager_empty"]');
    expect(board.element.contains(empty.element)).toBe(true);
    expect(empty.text()).toContain('Nessuna applicazione installata');
    expect(board.classes()).toContain('manager_preview_hollow');

    await wrapper
      .findAll('button')
      .find((button) => button.text() === 'Disponibili')!
      .trigger('click');
    await flushPromises();
    expect(wrapper.get('[data-testid="app_manager_empty"]').text()).toContain(
      'Nessuna applicazione da installare',
    );

    for (const control of ['refresh', 'view_list', 'view_preview']) {
      const button = wrapper.get(`[data-testid="app_manager_${control}"]`);
      expect((button.element as HTMLButtonElement).disabled).toBe(true);
    }
    expect(paths()).toEqual(['/apps/installed', '/apps/available']);
    await wrapper.get('[data-testid="app_manager_refresh"]').trigger('click');
    await wrapper.get('[data-testid="app_manager_view_list"]').trigger('click');
    expect(paths()).toEqual(['/apps/installed', '/apps/available']);
    expect(wrapper.get('[data-testid="app_manager_view_preview"]').attributes('aria-pressed')).toBe(
      'true',
    );
    expect(wrapper.find('table').exists()).toBe(false);
    const list = wrapper.get('[data-testid="app_manager_catalog"]');
    expect(list.element.contains(wrapper.get('[data-testid="app_manager_empty"]').element)).toBe(
      true,
    );
    wrapper.unmount();
  });

  it('opens its settings from the gear in the toolbar', async () => {
    setActivePinia(createPinia());
    answer([]);
    const wrapper = mount(AppManager, { attachTo: document.body });
    await flushPromises();

    const gear = wrapper.get('[data-testid="app_manager_settings_open"]');
    expect(gear.attributes('aria-label')).toBe('Impostazioni del gestore applicazioni');
    await gear.trigger('click');
    await flushPromises();

    expect(document.querySelector('[data-testid="app_manager_settings"]')).not.toBeNull();
    wrapper.unmount();
  });
});
