import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import AppListItem from '@/apps/nas-app-manager/components/AppListItem.vue';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import { setLocale } from '@/i18n';
import type { ManagerRow } from '@/apps/nas-app-manager/types';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const row: ManagerRow = {
  id: 'example',
  name: 'Example',
  info: 'A package',
  system: false,
  version: '1.0.0',
  icon: '/example.svg',
  repository: 'example/app',
  updateAvailable: true,
  latestVersion: '2.0.0',
};

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('en');
});

describe('AppListItem', () => {
  it('shows the name and the update on offer without a process status', () => {
    const wrapper = mount(AppListItem, { props: { app: row } });

    expect(wrapper.text()).toContain('Example');
    expect(wrapper.get('[data-testid="app_manager_badge_example"]').text()).toContain('2.0.0');
    expect(wrapper.find('[data-testid="app_manager_state_example"]').exists()).toBe(false);
  });

  it('draws a spinner over the icon of an application being installed', async () => {
    const wrapper = mount(AppListItem, { props: { app: row } });
    expect(wrapper.find('[data-testid="app_manager_installing_example"]').exists()).toBe(false);

    useAppManagerStore().installing = ['example'];
    await wrapper.vm.$nextTick();
    expect(wrapper.get('[data-testid="app_manager_installing_example"]').text()).toBe('Installing');
  });

  it('keeps an integrated application protected without showing a badge', () => {
    const wrapper = mount(AppListItem, {
      props: { app: { ...row, id: 'nas-global-search', system: true, updateAvailable: false } },
    });

    expect(wrapper.attributes('aria-disabled')).toBe('true');
    expect(wrapper.find('.manager_system_badge').exists()).toBe(false);
    expect(wrapper.find('[data-testid="app_manager_state_example"]').exists()).toBe(false);
  });

  it('keeps an external system application selectable', () => {
    const wrapper = mount(AppListItem, { props: { app: { ...row, system: true } } });
    expect(wrapper.attributes('aria-disabled')).toBeUndefined();
    expect(wrapper.get('[data-testid="app_manager_badge_example"]').text()).toContain('2.0.0');
  });

  it('selects the application and asks for its context menu', async () => {
    const manager = useAppManagerStore();
    const select = vi.spyOn(manager, 'select').mockImplementation(() => undefined);
    const wrapper = mount(AppListItem, { props: { app: row } });

    await wrapper.trigger('click');
    expect(select).toHaveBeenCalledWith('example');

    await wrapper.trigger('contextmenu');
    await wrapper.trigger('keydown', { key: 'F10', shiftKey: true });
    expect(wrapper.emitted('context')).toHaveLength(2);
  });
});
