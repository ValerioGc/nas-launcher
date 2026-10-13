import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ManagerSettings from '@/apps/nas-app-manager/components/modals/ManagerSettings.vue';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { CatalogueEntry } from '@/apps/nas-app-manager/types';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const general = {
  nasName: 'nas',
  theme: 'system',
  language: 'it',
  createShortcut: false,
  desktopIconSize: 'large',
  accent: '',
  background: { mode: 'gradient' },
};

const photos: CatalogueEntry = {
  id: 'photos',
  name: 'Photos',
  info: '',
  provider: 'github',
  repository: 'example/photos',
  asset: 'photos.tar.gz',
  preinstall: false,
  logo: '',
  category: null,
};

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request)
    .mockReset()
    .mockImplementation((path: string, options?: RequestInit) => {
      if (path.endsWith('/logo')) {
        return Promise.resolve(undefined as never);
      }
      if (options?.method === 'PUT') {
        const body = JSON.parse(String(options.body));
        return Promise.resolve((path === '/apps/catalogue' ? body.apps : body) as never);
      }
      return Promise.resolve(
        (path === '/settings/apps'
          ? { autoCheck: false, frequency: 'daily', checkOnOpen: false, updateOnOpen: false }
          : path === '/apps/catalogue'
            ? [photos]
            : path === '/settings/general'
              ? general
              : []) as never,
      );
    });
});

afterEach(() => {
  document.body.innerHTML = '';
});

const find = (testid: string) => document.querySelector<HTMLElement>(`[data-testid="${testid}"]`);

async function openSettings() {
  const wrapper = mount(ManagerSettings, { props: { modelValue: true }, attachTo: document.body });
  await flushPromises();
  return wrapper;
}

/** Opens the settings again after a save closed them, keeping what the window holds. */
async function reopen(wrapper: Awaited<ReturnType<typeof openSettings>>) {
  await wrapper.setProps({ modelValue: false });
  await wrapper.setProps({ modelValue: true });
  await flushPromises();
}

const puts = () =>
  vi
    .mocked(request)
    .mock.calls.filter(([, options]) => options?.method === 'PUT')
    .map(([path]) => path);

describe('app manager settings', () => {
  it('holds the automatic updates, the token and the catalogue, with one save at the bottom', async () => {
    const wrapper = await openSettings();

    expect(document.querySelector('[aria-label="Aggiornamenti automatici"]')).not.toBeNull();
    expect(document.querySelector('[aria-label="Token per le release private"]')).not.toBeNull();
    expect(find('catalogue_row_photos')).not.toBeNull();
    expect(find('app_manager_settings_save')?.closest('.common_modal_footer')).not.toBeNull();
    wrapper.unmount();
  });

  it('edits an entry in place of the settings and keeps the change until the save', async () => {
    const wrapper = await openSettings();

    find('catalogue_edit_photos')?.click();
    await flushPromises();
    expect(find('app_manager_settings')).toBeNull();
    expect(find('app_manager_settings_save')).toBeNull();

    expect(find('catalogue_apply')?.closest('.common_modal_footer')).not.toBeNull();
    const name = find('catalogue_name') as HTMLInputElement;
    name.value = 'Foto';
    name.dispatchEvent(new Event('input'));
    find('catalogue_apply')?.click();
    await flushPromises();

    expect(find('catalogue_row_photos')?.textContent).toContain('Foto');
    expect(puts()).toEqual([]);

    find('catalogue_edit_photos')?.click();
    await flushPromises();
    find('catalogue_cancel')?.click();
    await flushPromises();
    expect(find('catalogue_row_photos')?.textContent).toContain('Foto');
    wrapper.unmount();
  });

  it('saves the settings always, the catalogue when it changed, and refreshes the manager', async () => {
    const load = vi.spyOn(useAppManagerStore(), 'load');
    const wrapper = await openSettings();

    find('app_manager_settings_save')?.click();
    await flushPromises();
    expect(puts()).toEqual(['/settings/apps']);
    expect(load).not.toHaveBeenCalled();
    // Saved, the window closes.
    expect(wrapper.emitted('update:modelValue')).toEqual([[false]]);
    expect(find('app_manager_settings')).toBeNull();
    await reopen(wrapper);

    find('catalogue_remove_photos')?.click();
    await flushPromises();
    expect(find('catalogue_row_photos')).not.toBeNull();
    find('app_manager_cancel')?.click();
    await flushPromises();
    expect(find('catalogue_row_photos')).not.toBeNull();
    expect(puts()).toEqual(['/settings/apps']);

    find('catalogue_remove_photos')?.click();
    await flushPromises();
    find('app_manager_confirm')?.click();
    await flushPromises();
    expect(find('catalogue_row_photos')).toBeNull();
    expect(puts()).toEqual(['/settings/apps']);
    find('app_manager_settings_save')?.click();
    await flushPromises();

    expect(puts()).toEqual(['/settings/apps', '/settings/apps', '/apps/catalogue']);
    expect(load).toHaveBeenCalledOnce();
    expect(find('app_manager_settings_saved')).toBeNull();
    wrapper.unmount();
  });

  it('hands a written token over last without showing a local success banner', async () => {
    const wrapper = await openSettings();

    const field = find('app_manager_token_input') as HTMLInputElement;
    field.value = 'ghp_new';
    field.dispatchEvent(new Event('input'));
    find('app_manager_settings_save')?.click();
    await flushPromises();

    expect(puts()).toEqual(['/settings/apps', '/apps/credentials']);
    await reopen(wrapper);
    expect((find('app_manager_token_input') as HTMLInputElement).value).toBe('');
    expect(find('app_manager_settings_saved')).toBeNull();
    wrapper.unmount();
  });

  it('says why the launcher refused what was saved, as an error alert', async () => {
    const wrapper = await openSettings();
    vi.mocked(request).mockRejectedValueOnce(new Error('invalid catalogue entry'));

    find('app_manager_settings_save')?.click();
    await flushPromises();

    const alert = find('app_manager_settings_error');
    expect(alert?.getAttribute('role')).toBe('alert');
    expect(alert?.textContent).toContain('invalid catalogue entry');
    expect(wrapper.emitted('update:modelValue')).toBeUndefined();
    wrapper.unmount();
  });

  it('keeps the token field in a form, whose submission saves', async () => {
    const wrapper = await openSettings();
    const field = find('app_manager_token_input') as HTMLInputElement;
    const form = field.closest('form');
    expect(form?.id).toBe('app_manager_settings');
    expect(find('app_manager_settings_save')?.getAttribute('form')).toBe('app_manager_settings');

    form?.dispatchEvent(new Event('submit', { cancelable: true }));
    await flushPromises();
    expect(puts()).toEqual(['/settings/apps']);
    wrapper.unmount();
  });

  it('uploads a chosen logo when the settings are saved, and reads the catalogue back', async () => {
    const load = vi.spyOn(useAppManagerStore(), 'load');
    const wrapper = await openSettings();
    find('catalogue_edit_photos')?.click();
    await flushPromises();

    const image = new File(['<svg/>'], 'photos.svg', { type: 'image/svg+xml' });
    const input = find('catalogue_logo') as HTMLInputElement;
    Object.defineProperty(input, 'files', { value: [image] });
    input.dispatchEvent(new Event('change'));
    await flushPromises();
    find('catalogue_apply')?.click();
    await flushPromises();
    vi.mocked(request).mockClear();
    find('app_manager_settings_save')?.click();
    await flushPromises();

    expect(vi.mocked(request).mock.calls.map(([path, options]) => [path, options?.method])).toEqual(
      [
        ['/settings/apps', 'PUT'],
        ['/apps/photos/logo', 'PUT'],
        ['/apps/catalogue', undefined],
        ['/apps/installed', undefined],
      ],
    );
    const [, upload] = vi.mocked(request).mock.calls[1]!;
    expect(upload?.body).toBe(image);
    expect(upload?.headers).toEqual({ 'Content-Type': 'image/svg+xml' });
    expect(load).toHaveBeenCalledOnce();
    wrapper.unmount();
  });

  it('creates the shortcuts of new applications from here, saved with the general settings', async () => {
    const wrapper = await openSettings();
    find('app_manager_settings_save')?.click();
    await flushPromises();
    expect(puts()).not.toContain('/settings/general');
    await reopen(wrapper);

    const toggle = document.querySelector<HTMLInputElement>(
      '[data-testid="app_manager_shortcuts_input"]',
    );
    expect(toggle?.checked).toBe(false);
    toggle?.click();
    await flushPromises();
    find('app_manager_settings_save')?.click();
    await flushPromises();

    const saved = vi
      .mocked(request)
      .mock.calls.find(
        ([path, options]) => path === '/settings/general' && options?.method === 'PUT',
      );
    expect(JSON.parse(String(saved?.[1]?.body))).toEqual({ ...general, createShortcut: true });
    wrapper.unmount();
  });
});
