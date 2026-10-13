import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import AppearanceProbe from '@tests/fixtures/AppearanceProbe.vue';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import { request } from '@/api/client';
import { DARK_INK, LIGHT_INK } from '@/utils/contrast';
vi.mock('@/api/client', () => ({ request: vi.fn() }));

function stubLightSystem() {
  vi.stubGlobal('matchMedia', () => ({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
}

const background = {
  mode: 'gradient',
  image: '/wallpapers/landscape.svg',
  blur: 0,
  intensity: 100,
  color: '#ffffff',
  direction: 135,
  pattern: 'multiple',
  size: 60,
};

afterEach(() => {
  vi.unstubAllGlobals();
  document.documentElement.style.removeProperty('--accent');
  document.documentElement.style.removeProperty('--desktop-ink');
});

describe('system appearance', () => {
  it('draws the theme chosen for the type shown, the default for one it does not know', async () => {
    setActivePinia(createPinia());
    stubLightSystem();
    const wrapper = mount(AppearanceProbe);
    const root = document.documentElement;
    vi.mocked(request).mockResolvedValue({
      theme: 'light',
      themes: { light: 'sand', dark: 'forest' },
      language: 'it',
    });
    await useSettingsStore().load();
    await nextTick();
    expect(root.dataset.palette).toBe('sand');

    vi.mocked(request).mockResolvedValue({
      theme: 'dark',
      themes: { light: 'sand', dark: 'forest' },
      language: 'it',
    });
    await useSettingsStore().load();
    await nextTick();
    expect(root.dataset.theme).toBe('dark');
    expect(root.dataset.palette).toBe('forest');

    vi.mocked(request).mockResolvedValue({
      theme: 'dark',
      themes: { light: 'sand', dark: 'gone' },
      language: 'it',
    });
    await useSettingsStore().load();
    await nextTick();
    expect(root.dataset.palette).toBeUndefined();
    wrapper.unmount();
  });

  it('follows OS theme unless an explicit preference was saved', async () => {
    setActivePinia(createPinia());
    let update: () => void = () => {};
    const media = {
      matches: false,
      addEventListener: vi.fn((_event: string, fn: () => void) => {
        update = fn;
      }),
      removeEventListener: vi.fn(),
    };

    vi.stubGlobal('matchMedia', () => media);
    const wrapper = mount(AppearanceProbe);
    expect(document.documentElement.dataset.theme).toBe('light');
    media.matches = true;
    update();
    await nextTick();
    expect(document.documentElement.dataset.theme).toBe('dark');
    vi.mocked(request).mockResolvedValue({ theme: 'light', language: 'it' });
    await useSettingsStore().load();
    await nextTick();
    expect(document.documentElement.dataset.theme).toBe('light');
    wrapper.unmount();
    expect(media.removeEventListener).toHaveBeenCalled();
  });

  it('applies the chosen accent and gives it back when it is cleared', async () => {
    setActivePinia(createPinia());
    stubLightSystem();
    const wrapper = mount(AppearanceProbe);
    const settings = useSettingsStore();
    const style = document.documentElement.style;

    vi.mocked(request).mockResolvedValue({ theme: 'light', accent: '#ff8000', background });
    await settings.load();
    await nextTick();
    expect(style.getPropertyValue('--accent')).toBe('#ff8000');

    vi.mocked(request).mockResolvedValue({ theme: 'light', accent: '', background });
    await settings.load();
    await nextTick();
    expect(style.getPropertyValue('--accent')).toBe('');
    wrapper.unmount();
  });

  it.each([
    ['#ffffff', DARK_INK],
    ['#101010', LIGHT_INK],
  ])('writes the desktop labels in the ink that reads over %s', async (color, ink) => {
    setActivePinia(createPinia());
    stubLightSystem();
    const wrapper = mount(AppearanceProbe);

    vi.mocked(request).mockResolvedValue({
      theme: 'light',
      accent: '',
      background: { ...background, color },
    });
    await useSettingsStore().load();
    await nextTick();

    expect(document.documentElement.style.getPropertyValue('--desktop-ink')).toBe(ink);
    expect(document.documentElement.style.getPropertyValue('--desktop-ink-shadow')).not.toBe('');
    wrapper.unmount();
  });
});
