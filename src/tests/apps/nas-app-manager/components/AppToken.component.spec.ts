import { mount } from '@vue/test-utils';
import { beforeEach, expect, it } from 'vitest';
import AppToken from '@/apps/nas-app-manager/components/AppToken.vue';
import successIcon from '@/assets/icons/status/success.svg';
import errorIcon from '@/assets/icons/status/error.svg';
import { setLocale } from '@/i18n';

beforeEach(() => setLocale('it'));

it('shows a configured status and opens an empty password field only when editing', async () => {
  const wrapper = mount(AppToken, { props: { modelValue: '', configured: true } });
  expect(wrapper.find('[data-testid="app_manager_token_input"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="app_manager_token_note"]').exists()).toBe(false);
  const status = wrapper.get('[data-testid="app_manager_token_status"]');
  expect(status.attributes('aria-label')).toBe('Token configurato');
  expect(status.attributes('style')).toContain(successIcon);
  await wrapper.get('[data-testid="app_manager_token_edit"]').trigger('click');
  const field = wrapper.get<HTMLInputElement>('[data-testid="app_manager_token_input"]');

  expect(field.element.type).toBe('password');
  expect(field.element.value).toBe('');
  expect(wrapper.get('label[for="app_manager_token"]').text()).toBe('Token');
  expect(wrapper.get('[data-testid="app_manager_token_note"]').text()).toContain('lo sostituisce');

  await field.setValue('ghp_new');
  expect(wrapper.emitted('update:modelValue')).toEqual([['ghp_new']]);
});

it('shows the missing status and the input by default when no token is configured', () => {
  const wrapper = mount(AppToken, { props: { modelValue: '', configured: false } });

  const status = wrapper.get('[data-testid="app_manager_token_status"]');
  expect(status.attributes('aria-label')).toBe('Token non configurato');
  expect(status.attributes('style')).toContain(errorIcon);
  expect(
    wrapper.get<HTMLInputElement>('[data-testid="app_manager_token_input"]').element.type,
  ).toBe('password');
  expect(wrapper.find('[data-testid="app_manager_token_edit"]').exists()).toBe(false);
  expect(wrapper.get('[data-testid="app_manager_token_note"]').text()).toContain(
    'non è visualizzabile',
  );
});
