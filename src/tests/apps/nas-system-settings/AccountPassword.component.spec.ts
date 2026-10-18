import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import AccountPassword from '@/apps/nas-system-settings/components/AccountPassword.vue';
import { changePassword } from '@/api/account';
import { confirmPassword, LoginThrottled } from '@/api/gateway';
import { setLocale, t } from '@/i18n';

vi.mock('@/api/account', () => ({
  changePassword: vi.fn(),
  CodeRefused: class extends Error {},
  MailNotSent: class extends Error {},
}));
vi.mock('@/api/gateway', () => ({
  confirmPassword: vi.fn(),
  PasswordRefused: class extends Error {},
  LoginThrottled: class extends Error {},
}));

beforeEach(() => {
  setLocale('en');
  vi.clearAllMocks();
  vi.mocked(confirmPassword).mockResolvedValue('code');
});

async function fill(wrapper: ReturnType<typeof mount>, next: string, repeated: string) {
  await wrapper.get('[data-testid="account_current_password"]').setValue('long enough');
  await wrapper.get('[data-testid="account_new_password"]').setValue(next);
  await wrapper.get('[data-testid="account_repeat_password"]').setValue(repeated);
}

it('asks for the same new password twice before changing it', async () => {
  const wrapper = mount(AccountPassword);
  await fill(wrapper, 'a new password', 'a new passwor');
  expect(wrapper.text()).toContain(t('accountPasswordMismatch'));
  expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined();

  await fill(wrapper, 'a new password', 'a new password');
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(changePassword).toHaveBeenCalledWith('a new password', 'code');
  expect(wrapper.text()).toContain(t('accountPasswordChanged'));
});

it('says when the address has to wait', async () => {
  vi.mocked(confirmPassword).mockRejectedValue(new LoginThrottled('wait'));
  const wrapper = mount(AccountPassword);
  await fill(wrapper, 'a new password', 'a new password');
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(wrapper.text()).toContain(t('tooManyPasswordAttempts'));
  expect(changePassword).not.toHaveBeenCalled();
});

it('carries the name of the account in its form, for the password managers', () => {
  const wrapper = mount(AccountPassword, { props: { username: 'valerio' } });
  const account = wrapper.get('form input[autocomplete="username"]');
  expect((account.element as HTMLInputElement).value).toBe('valerio');
  expect(account.attributes('aria-hidden')).toBe('true');
});
