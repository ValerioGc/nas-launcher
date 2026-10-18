import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GlobalSettings from '@/apps/nas-system-settings/GlobalSettings.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import { useShellStore } from '@/stores/shellStore';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset().mockRejectedValue(new Error('down'));
});

describe('settings application', () => {
  it('offers its six sections, with the account second', async () => {
    const wrapper = mount(GlobalSettings);
    await flushPromises();

    expect(wrapper.findAll('.app_sections_nav button').map((button) => button.text())).toEqual([
      'Generali',
      'Account',
      'Rete',
      'Alimentazione',
      'Aggiornamenti',
      'Ripristino e reset',
    ]);
    for (const button of wrapper.findAll('.app_sections_nav button')) {
      expect(button.find('[data-testid="common_icon"]').exists()).toBe(true);
    }
    wrapper.unmount();
  });

  it('starts on the general section and moves to the one that was chosen', async () => {
    const wrapper = mount(GlobalSettings);
    await flushPromises();

    const tabs = wrapper.findAll('.app_sections_nav button');
    expect(tabs[0]?.attributes('aria-pressed')).toBe('true');
    expect(request).toHaveBeenCalledWith('/settings/general');

    await tabs[3]?.trigger('click');
    await flushPromises();
    expect(tabs[3]?.attributes('aria-pressed')).toBe('true');
    expect(request).toHaveBeenCalledWith('/settings/power');
    wrapper.unmount();
  });

  it('opens on the section the search asked for, even when it is already open', async () => {
    const shell = useShellStore();
    shell.launchQuery = 'power';
    const wrapper = mount(GlobalSettings);
    await flushPromises();
    const tabs = wrapper.findAll('.app_sections_nav button');
    expect(tabs[3]?.attributes('aria-pressed')).toBe('true');

    await tabs[0]?.trigger('click');
    shell.launchQuery = '';
    shell.launchQuery = 'power';
    await flushPromises();
    expect(tabs[3]?.attributes('aria-pressed')).toBe('true');
    wrapper.unmount();
  });

  it('names the window and its navigation after the application', async () => {
    const wrapper = mount(GlobalSettings);
    await flushPromises();

    expect(wrapper.get('.app_view').attributes('aria-label')).toBe('Impostazioni');
    expect(wrapper.get('nav').attributes('aria-label')).toBe('Impostazioni');
    wrapper.unmount();
  });
});
