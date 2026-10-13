import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import DesktopGrid from '@/components/DesktopGrid.vue';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import type { GeneralSettings } from '@/apps/nas-system-settings/types/general';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const manager = { id: 'nas-app-manager', name: 'Gestore applicazioni', info: '', system: true };
const search = { id: 'nas-global-search', name: 'Ricerca', info: '', system: true };
const three = [
  manager,
  search,
  { id: 'nas-system-settings', name: 'Impostazioni', info: '', system: true },
];

async function fillThree() {
  vi.mocked(request)
    .mockResolvedValueOnce(three)
    .mockResolvedValueOnce(
      three.map((app, position) => ({ kind: 'app', target: app.id, name: null, position })),
    )
    .mockImplementation(() => new Promise(() => {}));
  const apps = useAppsStore();
  await apps.fetchInstalled();
  return apps;
}

const order = (apps: ReturnType<typeof useAppsStore>) => apps.shortcuts.map((app) => app.id);

function pointer(type: string, x: number, y: number) {
  return new PointerEvent(type, { bubbles: true, button: 0, clientX: x, clientY: y });
}

async function fill() {
  vi.mocked(request)
    .mockResolvedValueOnce([manager])
    .mockResolvedValueOnce([{ kind: 'app', target: 'nas-app-manager', name: null, position: 0 }])
    .mockResolvedValue([]);
  const apps = useAppsStore();
  await apps.fetchInstalled();
  return apps;
}

beforeEach(() => {
  sessionStorage.clear();
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset().mockResolvedValue([]);
});

describe('desktop', () => {
  it('opens a saved folder in Explorer and removes only its shortcut', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce([
        manager,
        { id: 'nas-file-manager', name: 'Explorer', info: '', system: false },
      ])
      .mockResolvedValueOnce([
        { kind: 'folder', target: '/Documents/Été & photos', name: 'Été', position: 0 },
      ])
      .mockResolvedValueOnce([]);
    const apps = useAppsStore();
    await apps.fetchInstalled();
    const shell = useShellStore();
    const wrapper = mount(DesktopGrid, { attachTo: document.body });
    const folder = wrapper.get('[data-shortcut="folder:/Documents/Été & photos"]');
    expect(folder.text()).toBe('Été');
    await folder.get('button').trigger('click');
    expect(shell.activeId).toBe('nas-file-manager');
    expect(shell.locations['nas-file-manager']).toBe(
      '?path=%2FDocuments%2F%C3%89t%C3%A9+%26+photos',
    );
    await folder.get('button').trigger('contextmenu');
    await flushPromises();
    document.querySelector<HTMLButtonElement>('[data-testid="desktop_shortcut_menu"]')?.click();
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/desktop/folders', {
      method: 'DELETE',
      body: JSON.stringify({ path: '/Documents/Été & photos' }),
    });
    wrapper.unmount();
  });
  it('shows one icon per shortcut and opens the application it was given', async () => {
    await fill();
    const shell = useShellStore();
    const wrapper = mount(DesktopGrid, { attachTo: document.body });

    const icon = wrapper.get('[data-testid="desktop_shortcut_nas-app-manager"]');
    expect(icon.text()).toContain('Gestore applicazioni');

    await icon.get('button').trigger('click');
    expect(shell.activeId).toBe('nas-app-manager');
    wrapper.unmount();
  });

  it('draws the icons at the size of the general settings, large until they are read', async () => {
    await fill();
    const wrapper = mount(DesktopGrid);
    const grid = () => wrapper.get('[aria-label="Desktop"]');
    expect(grid().attributes('data-icon-size')).toBe('large');

    useSettingsStore().general = { desktopIconSize: 'small' } as GeneralSettings;
    await flushPromises();
    expect(grid().attributes('data-icon-size')).toBe('small');
    expect(grid().classes()).toContain('desktop_grid_small');
    wrapper.unmount();
  });

  it('offers to take the shortcut off the desktop', async () => {
    await fill();
    const wrapper = mount(DesktopGrid, { attachTo: document.body });
    await wrapper
      .get('[data-testid="desktop_shortcut_nas-app-manager"] button')
      .trigger('contextmenu');
    await flushPromises();

    const item = document.querySelector<HTMLButtonElement>('[data-testid="desktop_shortcut_menu"]');
    expect(item?.textContent?.trim()).toBe('Rimuovi collegamento');

    item?.click();
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/desktop/shortcuts/nas-app-manager', {
      method: 'DELETE',
      body: undefined,
    });
    wrapper.unmount();
  });

  it('shows a spinner instead of an empty desktop while the shortcuts load', async () => {
    vi.mocked(request).mockReturnValue(new Promise(() => {}));
    const apps = useAppsStore();
    void apps.fetchInstalled();
    const wrapper = mount(DesktopGrid);
    await flushPromises();

    expect(wrapper.find('[data-testid="desktop_loading"]').exists()).toBe(true);
    expect(wrapper.find('ul').exists()).toBe(false);
    wrapper.unmount();
  });

  it('stays empty when the list could not be read', () => {
    const apps = useAppsStore();
    apps.errorMessage = 'Richiesta non riuscita.';
    const wrapper = mount(DesktopGrid);

    expect(wrapper.findAll('li')).toHaveLength(0);
    expect(wrapper.find('[data-testid="desktop_loading"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('moves an icon to the place of the one it is dropped on, and does not open it', async () => {
    const apps = await fillThree();
    const shell = useShellStore();
    const wrapper = mount(DesktopGrid, { attachTo: document.body });
    const target = wrapper.get('[data-testid="desktop_shortcut_nas-system-settings"]').element;
    document.elementFromPoint = () => target;

    const dragged = wrapper.get('[data-testid="desktop_shortcut_nas-app-manager"]');
    dragged.element.dispatchEvent(pointer('pointerdown', 0, 0));
    globalThis.dispatchEvent(pointer('pointermove', 40, 0));
    await flushPromises();
    expect(dragged.classes()).toContain('desktop_dragged');
    expect(wrapper.get('[data-testid="desktop_shortcut_nas-system-settings"]').classes()).toContain(
      'desktop_drop_target',
    );

    globalThis.dispatchEvent(pointer('pointerup', 40, 0));
    await dragged.get('button').trigger('click');
    await flushPromises();

    expect(order(apps)).toEqual(['nas-global-search', 'nas-system-settings', 'nas-app-manager']);
    expect(request).toHaveBeenLastCalledWith('/desktop/shortcuts/order', {
      method: 'PUT',
      body: JSON.stringify({
        order: [
          { kind: 'app', target: 'nas-global-search' },
          { kind: 'app', target: 'nas-system-settings' },
          { kind: 'app', target: 'nas-app-manager' },
        ],
      }),
    });
    expect(shell.activeId).not.toBe('nas-app-manager');
    wrapper.unmount();
  });

  it('keeps a short press a click', async () => {
    const apps = await fillThree();
    const shell = useShellStore();
    const wrapper = mount(DesktopGrid, { attachTo: document.body });

    const icon = wrapper.get('[data-testid="desktop_shortcut_nas-system-settings"]');
    icon.element.dispatchEvent(pointer('pointerdown', 0, 0));
    globalThis.dispatchEvent(pointer('pointermove', 2, 1));
    globalThis.dispatchEvent(pointer('pointerup', 2, 1));
    await icon.get('button').trigger('click');

    expect(shell.activeId).toBe('nas-system-settings');
    expect(order(apps)).toEqual(['nas-app-manager', 'nas-global-search', 'nas-system-settings']);
    wrapper.unmount();
  });

  it('moves the focused icon with Alt and the arrows, and not past the ends', async () => {
    const apps = await fillThree();
    const wrapper = mount(DesktopGrid, { attachTo: document.body });

    await wrapper
      .get('[data-testid="desktop_shortcut_nas-global-search"] button')
      .trigger('keydown', { key: 'ArrowLeft', altKey: true });
    expect(order(apps)).toEqual(['nas-global-search', 'nas-app-manager', 'nas-system-settings']);

    await wrapper
      .get('[data-testid="desktop_shortcut_nas-global-search"] button')
      .trigger('keydown', { key: 'ArrowLeft', altKey: true });
    expect(order(apps)).toEqual(['nas-global-search', 'nas-app-manager', 'nas-system-settings']);
    wrapper.unmount();
  });
});
