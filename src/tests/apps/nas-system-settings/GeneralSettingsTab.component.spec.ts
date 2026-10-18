import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GeneralSettingsTab from '@/apps/nas-system-settings/components/tabs/GeneralSettingsTab.vue';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { GeneralSettings } from '@/apps/nas-system-settings/types/general';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const settings: GeneralSettings = {
  nasName: 'nas',
  theme: 'system',
  themes: { light: 'default', dark: 'default' },
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

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset();
});

const puts = () =>
  vi.mocked(request).mock.calls.filter(([, options]) => options?.method === 'PUT').length;

describe('general preferences', () => {
  it('saves a moment after the last change, once for changes in a row, and switches locale', async () => {
    vi.useFakeTimers();
    vi.mocked(request)
      .mockResolvedValueOnce(settings)
      .mockImplementation(async (_path, options) => JSON.parse(String(options?.body)));
    const store = useSettingsStore();
    await store.load();
    const wrapper = mount(GeneralSettingsTab);
    expect(wrapper.find('[data-testid="settings_save"]').exists()).toBe(false);
    await wrapper.get('[data-testid="settings_nas_name"]').setValue('home-nas');
    await wrapper.get('[data-testid="settings_theme"]').setValue('light');
    await wrapper.get('[data-testid="settings_language"]').setValue('en');
    const sizes = wrapper.get('[data-testid="settings_desktop_icon_size"]');
    expect(sizes.findAll('label').map((item) => item.text())).toEqual([
      'Piccole',
      'Medie',
      'Grandi',
      'Molto grandi',
    ]);
    await wrapper.get('[data-testid="settings_desktop_icon_size_xlarge"]').setValue(true);
    await wrapper.get('[data-testid="settings_background_direction_90"]').setValue(true);
    expect(puts()).toBe(0);

    await vi.advanceTimersByTimeAsync(700);
    await flushPromises();
    expect(puts()).toBe(1);
    expect(store.general?.theme).toBe('light');
    expect(store.general?.nasName).toBe('home-nas');
    expect(store.general?.desktopIconSize).toBe('xlarge');
    expect(store.general?.background.direction).toBe(90);

    await wrapper.get('[data-testid="settings_nas_name"]').setValue('-bad');
    await vi.advanceTimersByTimeAsync(700);
    await flushPromises();
    expect(puts()).toBe(1);
    wrapper.unmount();
    vi.useRealTimers();
    setLocale('it');
  });

  it('offers the themes of the type chosen and keeps one for each type', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce(settings)
      .mockImplementation(async (_path, options) => JSON.parse(String(options?.body)));
    const store = useSettingsStore();
    await store.load();
    const wrapper = mount(GeneralSettingsTab);
    const options = () =>
      wrapper
        .get('[data-testid="settings_theme_name"]')
        .findAll('option')
        .map((item) => item.text());

    expect(options()).toEqual(['Predefinito', 'Sabbia', 'Lavanda', 'Ghiaccio']);
    await wrapper.get('[data-testid="settings_theme_name"]').setValue('sand');
    await wrapper.get('[data-testid="settings_theme"]').setValue('dark');
    expect(options()).toEqual(['Predefinito', 'Grafite', 'Notte', 'Bosco']);
    expect(
      (wrapper.get('[data-testid="settings_theme_name"]').element as HTMLSelectElement).value,
    ).toBe('default');
    await wrapper.get('[data-testid="settings_theme_name"]').setValue('forest');

    await wrapper.get('form').trigger('submit');
    await flushPromises();

    expect(store.general?.themes).toEqual({ light: 'sand', dark: 'forest' });
    wrapper.unmount();
  });
});
