import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import ConfirmationModal from '@/apps/nas-app-manager/components/modals/ConfirmationModal.vue';
import { setLocale } from '@/i18n';

it('shows the target and error, blocks duplicate confirmation while busy, and emits cancellation', async () => {
  setLocale('it');
  const wrapper = mount(ConfirmationModal, {
    props: {
      modelValue: true,
      appName: 'Example',
      action: 'uninstall',
      busy: true,
      error: 'Failed',
    },
    global: { stubs: { teleport: true } },
  });
  expect(wrapper.text()).toContain('Disinstallare Example?');
  expect(wrapper.text()).not.toContain('simulata');
  expect(wrapper.get('[role="alert"]').text()).toBe('Failed');
  const confirm = wrapper.findAll('button').find((button) => button.text() === 'Conferma')!;
  expect(confirm.attributes('disabled')).toBeDefined();
  await confirm.trigger('click');
  expect(wrapper.emitted('confirm')).toBeUndefined();
  await wrapper.setProps({ busy: false });
  await confirm.trigger('click');
  expect(wrapper.emitted('confirm')).toHaveLength(1);
  await wrapper
    .findAll('button')
    .find((button) => button.text() === 'Annulla')!
    .trigger('click');
  expect(wrapper.emitted('update:modelValue')).toEqual([[false]]);
  wrapper.unmount();
});

it('asks to delete the application data only on uninstall, selected by default', async () => {
  setLocale('en');
  const wrapper = mount(ConfirmationModal, {
    props: { modelValue: true, appName: 'Example', action: 'uninstall', busy: false, error: '' },
    global: { stubs: { teleport: true } },
  });
  const box = wrapper.get<HTMLInputElement>('[data-testid="app_manager_remove_data_input"]');
  expect(box.element.checked).toBe(true);
  await box.setValue(false);
  expect(wrapper.emitted('update:removeData')).toEqual([[false]]);

  await wrapper.setProps({ action: 'install' });
  expect(wrapper.find('[data-testid="app_manager_remove_data"]').exists()).toBe(false);
  wrapper.unmount();
});
