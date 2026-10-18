import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import AppMenu from '@/components/header/AppMenu.vue';
import { useAppsStore } from '@/stores/appsStore';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset();
});

describe('application context menu', () => {
  it('creates a desktop shortcut on the backend from the right-click action', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce([
        { id: 'nas-app-manager', name: 'Apps', info: '', system: true },
        { id: 'mediaServer', name: 'Media', info: '', system: false },
      ])
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([{ kind: 'app', target: 'mediaServer', name: null, position: 0 }]);

    const store = useAppsStore();
    await store.fetchInstalled();
    const wrapper = mount(AppMenu, { props: { modelValue: true }, attachTo: document.body });
    await wrapper
      .get('[data-testid="app_menu_app_mediaServer"]')
      .trigger('contextmenu', { clientX: 100, clientY: 100 });
    await flushPromises();
    document.querySelector<HTMLButtonElement>('[role="menuitem"]')?.click();
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/desktop/shortcuts/mediaServer', { method: 'POST' });
    expect(store.shortcuts).toHaveLength(1);
    wrapper.unmount();
  });

  it('separates the applications of the system from the installed ones', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce([
        { id: 'nas-app-manager', name: 'Apps', info: '', system: true },
        { id: 'mediaServer', name: 'Media', info: '', system: false },
      ])
      .mockResolvedValueOnce([]);
    const store = useAppsStore();
    await store.fetchInstalled();
    const wrapper = mount(AppMenu, { props: { modelValue: true }, attachTo: document.body });
    const listed = () =>
      wrapper.findAll('[data-testid^="app_menu_app_"]').map((row) => row.attributes('data-testid'));
    expect(listed()).toEqual(['app_menu_app_nas-app-manager', 'app_menu_app_mediaServer']);
    await wrapper.get('[data-testid="app_menu_folder_system"]').trigger('click');
    expect(listed()).toEqual(['app_menu_app_nas-app-manager']);
    await wrapper.get('[data-testid="app_menu_folder_all"]').trigger('click');
    expect(listed()).toEqual(['app_menu_app_nas-app-manager', 'app_menu_app_mediaServer']);
    wrapper.unmount();
  });

  it('gives every category of the installed applications a folder of its own', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce([
        { id: 'nas-app-manager', name: 'Apps', info: '', system: true },
        { id: 'pictureManager', name: 'Pictures', info: '', system: false, category: 'media' },
        { id: 'nas-disk-manager', name: 'Disks', info: '', system: false, category: 'storage' },
        { id: 'notes', name: 'Notes', info: '', system: false, category: null },
        { id: 'securityCenter', name: 'Security', info: '', system: false, category: 'system' },
        { id: 'nas-file-manager', name: 'Explorer', info: '', system: false, category: 'storage' },
      ])
      .mockResolvedValueOnce([]);
    await useAppsStore().fetchInstalled();
    const wrapper = mount(AppMenu, { props: { modelValue: true }, attachTo: document.body });
    const listed = () =>
      wrapper.findAll('[data-testid^="app_menu_app_"]').map((row) => row.attributes('data-testid'));

    expect(
      wrapper.findAll('[data-testid^="app_menu_folder_"]').map((folder) => folder.text()),
    ).toEqual(['Tutte le app', 'Di sistema', 'Archiviazione', 'Media']);
    await wrapper.get('[data-testid="app_menu_folder_system"]').trigger('click');
    expect(listed()).toEqual(['app_menu_app_nas-app-manager', 'app_menu_app_securityCenter']);
    await wrapper.get('[data-testid="app_menu_folder_storage"]').trigger('click');
    expect(listed()).toEqual(['app_menu_app_nas-disk-manager', 'app_menu_app_nas-file-manager']);
    await wrapper.get('[data-testid="app_menu_folder_media"]').trigger('click');
    expect(listed()).toEqual(['app_menu_app_pictureManager']);
    await wrapper.get('[data-testid="app_menu_folder_all"]').trigger('click');
    expect(listed()).toContain('app_menu_app_notes');
    wrapper.unmount();
  });

  async function ready() {
    vi.mocked(request)
      .mockResolvedValueOnce([
        { id: 'nas-app-manager', name: 'Apps', info: '', system: true },
        { id: 'mediaServer', name: 'Media', info: '', system: false },
      ])
      .mockResolvedValue([]);
    const store = useAppsStore();
    await store.fetchInstalled();
    return mount(AppMenu, { props: { modelValue: true }, attachTo: document.body });
  }

  it('keeps the settings out of the list and offers them at the foot of the menu', async () => {
    const wrapper = await ready();

    expect(wrapper.find('[data-testid="app_menu_app_nas-system-settings"]').exists()).toBe(false);
    await wrapper.get('[data-testid="app_menu_settings"]').trigger('click');

    expect(wrapper.emitted('openSettings')).toHaveLength(1);
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
    wrapper.unmount();
  });

  it('asks the shell to open the application that was chosen, and closes', async () => {
    const wrapper = await ready();
    await wrapper.get('[data-testid="app_menu_app_mediaServer"]').trigger('click');

    expect(wrapper.emitted('select')?.[0]?.[0]).toMatchObject({ id: 'mediaServer' });
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
    wrapper.unmount();
  });

  it.each([
    ['backdrop', '[data-testid="app_menu_backdrop"]', 'click'],
    ['Escape', '.app_menu_panel', 'keydown.esc'],
  ])('closes from the %s', async (_name, selector, event) => {
    const wrapper = await ready();
    await wrapper.get(selector).trigger(event);

    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
    wrapper.unmount();
  });

  it('tells an unreadable list apart from an empty one', async () => {
    vi.mocked(request).mockResolvedValue([]);
    const store = useAppsStore();
    await store.fetchInstalled();
    const empty = mount(AppMenu, { props: { modelValue: true }, attachTo: document.body });
    expect(empty.get('.app_menu_empty').text()).toBe('Nessuna applicazione installata.');
    expect(empty.find('[role="alert"]').exists()).toBe(false);
    empty.unmount();

    store.errorMessage = 'Richiesta non riuscita.';
    const broken = mount(AppMenu, { props: { modelValue: true }, attachTo: document.body });
    expect(broken.get('[role="alert"]').text()).toBe('Richiesta non riuscita.');
    broken.unmount();
  });
});
