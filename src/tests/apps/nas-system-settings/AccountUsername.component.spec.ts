import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import AccountUsername from '@/apps/nas-system-settings/components/AccountUsername.vue';
import { changeUsername } from '@/api/account';
import { confirmPassword, PasswordRefused } from '@/api/gateway';
import { setLocale, t } from '@/i18n';

vi.mock('@/api/account', () => ({
  changeUsername: vi.fn(),
  CodeRefused: class extends Error {},
  MailNotSent: class extends Error {},
}));
vi.mock('@/api/gateway', () => ({
  confirmPassword: vi.fn(),
  PasswordRefused: class extends Error {},
  LoginThrottled: class extends Error {},
}));

const account = { username: 'admin', email: null, recovery: false };

beforeEach(() => {
  setLocale('en');
  vi.clearAllMocks();
  vi.mocked(confirmPassword).mockResolvedValue('code');
});

it('changes the name once the password is confirmed, and only to a valid new one', async () => {
  vi.mocked(changeUsername).mockResolvedValue({ username: 'keeper', email: null, recovery: false });
  const wrapper = mount(AccountUsername, { props: { account } });
  const submit = wrapper.get('button[type="submit"]');
  await wrapper.get('[data-testid="account_username_password"]').setValue('long enough');

  for (const invalid of ['admin', 'ab', '1admin', 'with space']) {
    await wrapper.get('[data-testid="account_new_username"]').setValue(invalid);
    expect(submit.attributes('disabled'), invalid).toBeDefined();
  }
  await wrapper.get('[data-testid="account_new_username"]').setValue(' keeper ');
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(confirmPassword).toHaveBeenCalledWith('long enough');
  expect(changeUsername).toHaveBeenCalledWith('keeper', 'code');
  expect(wrapper.emitted('changed')?.[0]).toEqual([
    { username: 'keeper', email: null, recovery: false },
  ]);
  expect(wrapper.text()).toContain(t('accountUsernameChanged'));
});

it('says a wrong password and clears it', async () => {
  vi.mocked(confirmPassword).mockRejectedValue(new PasswordRefused('wrong'));
  const wrapper = mount(AccountUsername, { props: { account } });
  await wrapper.get('[data-testid="account_new_username"]').setValue('keeper');
  await wrapper.get('[data-testid="account_username_password"]').setValue('wrong one');
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(wrapper.text()).toContain(t('wrongAccountPassword'));
  expect(
    (wrapper.get('[data-testid="account_username_password"]').element as HTMLInputElement).value,
  ).toBe('');
  expect(wrapper.emitted('changed')).toBeUndefined();
});

it('says under the field why a name cannot be taken, and keeps the save off', async () => {
  const wrapper = mount(AccountUsername, { props: { account } });
  const field = wrapper.get('[data-testid="account_new_username"]');
  expect(wrapper.text()).toContain('From 3 to 25 characters.');

  await field.setValue('9 bad name');
  await wrapper.get('[data-testid="account_username_password"]').setValue('secret');
  expect(field.attributes('aria-invalid')).toBe('true');
  expect(wrapper.get('[role="alert"]').text()).toBe(t('accountUsernameInvalid'));
  expect(wrapper.get('[data-testid="account_username_save"]').attributes('disabled')).toBeDefined();

  await field.setValue('keeper');
  expect(field.attributes('aria-invalid')).toBe('false');
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  expect(wrapper.get('[data-testid="account_username_save"]').text()).toBe(t('save'));
  expect(
    wrapper.get('[data-testid="account_username_save"]').attributes('disabled'),
  ).toBeUndefined();
});
