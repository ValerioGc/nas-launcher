import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, expect, it, vi } from 'vitest';
import AppForge from '@/apps/nas-app-manager/components/AppForge.vue';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import { answerManager, credentials } from '@/tests/apps/nas-app-manager/fixtures';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
beforeEach(async () => {
  setActivePinia(createPinia());
  setLocale('it');
  answerManager();
  await useAppManagerStore().initialize();
});

it('asks for the token only while it is missing, and leaves setting it to the settings', async () => {
  const wrapper = mount(AppForge);
  expect(wrapper.get('[data-testid="app_manager_token_status"]').text()).toContain(
    'si imposta nelle impostazioni del gestore',
  );
  expect(wrapper.find('input').exists()).toBe(false);

  vi.mocked(request).mockResolvedValueOnce({ ...credentials, configured: true });
  await useAppManagerStore().loadCredentials();
  await wrapper.vm.$nextTick();
  expect(wrapper.find('[data-testid="app_manager_token_status"]').exists()).toBe(false);
  wrapper.unmount();
});

it('shows a failed update check as an error alert', async () => {
  const manager = useAppManagerStore();
  vi.mocked(request).mockResolvedValueOnce(credentials);
  await manager.loadCredentials();
  vi.mocked(request).mockRejectedValueOnce(new Error('the forge does not answer'));
  const wrapper = mount(AppForge);

  await manager.check();
  await wrapper.vm.$nextTick();

  const alert = wrapper.get('[data-testid="app_manager_check_error"]');
  expect(alert.attributes('role')).toBe('alert');
  expect(alert.text()).toContain('the forge does not answer');
  wrapper.unmount();
});

it('offers the update check on the installed tab only', async () => {
  const manager = useAppManagerStore();
  vi.mocked(request).mockResolvedValueOnce({ ...credentials, configured: true });
  await manager.loadCredentials();
  const wrapper = mount(AppForge);
  expect(wrapper.find('[data-testid="app_manager_check"]').exists()).toBe(true);

  await manager.setEndpoint('available');
  await wrapper.vm.$nextTick();
  expect(wrapper.find('section').exists()).toBe(false);

  vi.mocked(request).mockResolvedValueOnce(credentials);
  await manager.loadCredentials();
  await wrapper.vm.$nextTick();
  expect(wrapper.find('[data-testid="app_manager_check"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="app_manager_token_status"]').exists()).toBe(true);
  wrapper.unmount();
});

it('replaces the check with all or selected updates without an inline message', async () => {
  const manager = useAppManagerStore();
  vi.mocked(request).mockResolvedValueOnce({ ...credentials, configured: true });
  await manager.loadCredentials();
  const wrapper = mount(AppForge);

  await manager.check();
  await flushPromises();

  expect(wrapper.find('[data-testid="app_manager_check"]').exists()).toBe(false);
  expect(wrapper.get('[data-testid="app_manager_update_available"]').text()).toContain(
    'Aggiorna tutte',
  );
  expect(wrapper.find('[data-testid="app_manager_updates"]').exists()).toBe(false);

  manager.select('example');
  await flushPromises();
  expect(wrapper.get('[data-testid="app_manager_update_available"]').text()).toContain(
    'Aggiorna selezionata',
  );
  wrapper.unmount();
});
