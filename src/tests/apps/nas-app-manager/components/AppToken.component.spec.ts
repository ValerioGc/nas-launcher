import { mount } from '@vue/test-utils';
import { beforeEach, expect, it } from 'vitest';
import AppToken from '@/apps/nas-app-manager/components/AppToken.vue';
import { setLocale } from '@/i18n';

beforeEach(() => setLocale('it'));

it('is a hidden field that hands the token back and never shows one already saved', async () => {
  const wrapper = mount(AppToken, { props: { modelValue: '', configured: true } });
  const field = wrapper.get<HTMLInputElement>('[data-testid="app_manager_token_input"]');

  expect(field.element.type).toBe('password');
  expect(field.element.value).toBe('');
  expect(wrapper.get('label[for="app_manager_token"]').text()).toBe('Token');
  expect(wrapper.get('[data-testid="app_manager_token_note"]').text()).toContain('lo sostituisce');

  await field.setValue('ghp_new');
  expect(wrapper.emitted('update:modelValue')).toEqual([['ghp_new']]);
});

it('says the token is write-only when none is saved yet', () => {
  const wrapper = mount(AppToken, { props: { modelValue: '', configured: false } });

  expect(wrapper.get('[data-testid="app_manager_token_note"]').text()).toContain(
    'non è visualizzabile',
  );
});
