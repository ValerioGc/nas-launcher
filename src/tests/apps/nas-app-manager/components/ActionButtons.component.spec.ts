import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, expect, it, vi } from 'vitest';
import ActionButtons from '@/apps/nas-app-manager/components/ActionButtons.vue';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import { answerManager, credentials, installed } from '@/tests/apps/nas-app-manager/fixtures';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
beforeEach(async () => {
  setActivePinia(createPinia());
  setLocale('it');
  answerManager();
  await useAppManagerStore().initialize();
  useAppManagerStore().select(installed.id);
});

it('requires confirmation, keeps failures visible, and lets the user retry', async () => {
  const wrapper = mount(ActionButtons, { global: { stubs: { teleport: true } } });
  await wrapper
    .findAll('button')
    .find((button) => button.text() === 'Disinstalla')!
    .trigger('click');
  const confirm = () => wrapper.findAll('button').find((button) => button.text() === 'Conferma')!;
  expect(request).not.toHaveBeenCalledWith('/apps/example/uninstall', expect.anything());
  vi.mocked(request).mockRejectedValueOnce(new Error('Operation failed'));
  await confirm().trigger('click');
  await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toBe('Operation failed');
  await confirm().trigger('click');
  await flushPromises();
  expect(wrapper.find('dialog').exists()).toBe(false);
  expect(wrapper.find('[data-testid="app_manager_actions"]').exists()).toBe(false);
  wrapper.unmount();
});

it('cancels a pending confirmation when the catalogue changes', async () => {
  const wrapper = mount(ActionButtons, { global: { stubs: { teleport: true } } });
  await wrapper
    .findAll('button')
    .find((button) => button.text() === 'Disinstalla')!
    .trigger('click');
  expect(wrapper.find('dialog').exists()).toBe(true);
  await useAppManagerStore().setEndpoint('available');
  await flushPromises();
  expect(wrapper.find('dialog').exists()).toBe(false);
  expect(request).not.toHaveBeenCalledWith('/apps/example/uninstall', expect.anything());
  wrapper.unmount();
});

it('offers no installation until the token is configured', async () => {
  const manager = useAppManagerStore();
  const wrapper = mount(ActionButtons, { global: { stubs: { teleport: true } } });
  const button = (label: string) =>
    wrapper.findAll('button').find((item) => item.text().startsWith(label))!;
  expect(button('Disinstalla').attributes('disabled')).toBeUndefined();

  vi.mocked(request).mockResolvedValueOnce({ ...credentials, configured: true });
  await manager.loadCredentials();

  await manager.setEndpoint('available');
  manager.select(installed.id);
  await flushPromises();
  expect(button('Installa').attributes('disabled')).toBeUndefined();
  manager.installing = [installed.id];
  await flushPromises();
  expect(button('Installa').attributes('disabled')).toBeDefined();
  wrapper.unmount();
});

it('leaves update commands to the manager toolbar', async () => {
  const manager = useAppManagerStore();
  await manager.check();
  const wrapper = mount(ActionButtons, { global: { stubs: { teleport: true } } });

  expect(wrapper.find('[data-testid="app_manager_update"]').exists()).toBe(false);
  expect(wrapper.text()).toContain('Disinstalla');
  expect(wrapper.text()).toContain('Dettagli');
  wrapper.unmount();
});
