import { mount, flushPromises } from '@vue/test-utils';
import { nextTick } from 'vue';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, expect, it, vi } from 'vitest';
import AppList from '@/apps/nas-app-manager/components/AppList.vue';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import { answerManager, installed } from '@/tests/apps/nas-app-manager/fixtures';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
beforeEach(async () => {
  setActivePinia(createPinia());
  setLocale('it');
  answerManager();
  await useAppManagerStore().initialize();
});

it('keeps sorting and selection across view changes without requesting data', async () => {
  const manager = useAppManagerStore();
  manager.setView('list');
  const wrapper = mount(AppList);
  vi.mocked(request).mockClear();
  await wrapper.get('[data-testid="app_manager_sort_name"]').trigger('click');
  expect(wrapper.findAll('tbody tr')[0]!.text()).toContain('Terminale');
  await wrapper.get('[data-testid="app_manager_app_example"]').trigger('click');
  manager.setView('preview');
  manager.setShowSystem(false);
  await nextTick();
  expect(wrapper.find('[data-testid="app_manager_app_nas-system-terminal"]').exists()).toBe(false);
  expect(wrapper.get('[data-testid="app_manager_app_example"]').attributes('aria-pressed')).toBe(
    'true',
  );
  manager.setView('list');
  manager.setShowSystem(true);
  await nextTick();
  expect(wrapper.findAll('tbody tr')[0]!.text()).toContain('Terminale');
  expect(request).not.toHaveBeenCalled();
  wrapper.unmount();
});

it.each(['list', 'preview'] as const)(
  'offers keyboard shortcuts only for installed apps in %s view',
  async (view) => {
    useAppManagerStore().setView(view);
    const wrapper = mount(AppList, {
      global: { stubs: { teleport: true } },
    });
    await wrapper
      .get('[data-testid="app_manager_app_example"]')
      .trigger('keydown', { key: 'F10', shiftKey: true });
    expect(wrapper.find('[role="menuitem"]').exists()).toBe(true);
    await useAppManagerStore().setEndpoint('available');
    await flushPromises();
    expect(wrapper.find('[role="menuitem"]').exists()).toBe(false);
    await wrapper.get(`[data-testid="app_manager_app_${installed.id}"]`).trigger('contextmenu');
    expect(wrapper.find('[role="menuitem"]').exists()).toBe(false);
    wrapper.unmount();
  },
);
