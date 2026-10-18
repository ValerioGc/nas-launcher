import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, expect, it, vi } from 'vitest';
import AccountEmail from '@/apps/nas-system-settings/components/AccountEmail.vue';
import { MailNotSent, removeEmail, requestEmail, setRecovery, verifyEmail } from '@/api/account';
import { confirmPassword } from '@/api/gateway';
import { setLocale, t } from '@/i18n';
import { useShellStore } from '@/stores/shellStore';

vi.mock('@/api/account', () => ({
  requestEmail: vi.fn(),
  verifyEmail: vi.fn(),
  removeEmail: vi.fn(),
  setRecovery: vi.fn(),
  CodeRefused: class extends Error {},
  MailNotSent: class extends Error {},
}));
vi.mock('@/api/gateway', () => ({
  confirmPassword: vi.fn(),
  PasswordRefused: class extends Error {},
  LoginThrottled: class extends Error {},
}));

const account = { username: 'admin', email: null, recovery: true };
const ready = { installed: true, sender: true };

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('en');
  vi.clearAllMocks();
  vi.mocked(confirmPassword).mockResolvedValue('code');
});

it('sends a code to the new address and makes it the recovery one once verified', async () => {
  vi.mocked(verifyEmail).mockResolvedValue({
    username: 'admin',
    email: 'me@example.com',
    recovery: true,
  });
  const wrapper = mount(AccountEmail, { props: { account, mail: ready } });
  expect(wrapper.get('[data-testid="account_email_current"]').text()).toBe(t('accountEmailNone'));
  expect(wrapper.find('[data-testid="account_remove_email"]').exists()).toBe(false);

  await wrapper.get('[data-testid="account_new_email"]').setValue(' me@example.com ');
  await wrapper.get('[data-testid="account_email_password"]').setValue('long enough');
  await wrapper.get('[data-testid="account_send_code"]').trigger('submit');
  await flushPromises();
  expect(requestEmail).toHaveBeenCalledWith('me@example.com', 'code');
  expect(wrapper.text()).toContain(t('accountCodeSent', { email: 'me@example.com' }));

  await wrapper.get('[data-testid="account_email_code"]').setValue('123456');
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(verifyEmail).toHaveBeenCalledWith('123456');
  expect(wrapper.emitted('changed')?.[0]).toEqual([
    { username: 'admin', email: 'me@example.com', recovery: true },
  ]);
  expect(wrapper.text()).toContain(t('accountEmailVerified'));
});

it('removes the verified address with the password', async () => {
  const wrapper = mount(AccountEmail, {
    props: { account: { username: 'admin', email: 'me@example.com', recovery: true }, mail: ready },
  });
  expect(wrapper.get('[data-testid="account_email_current"]').text()).toBe(
    t('accountEmailCurrent', { email: 'me@example.com' }),
  );
  await wrapper.get('[data-testid="account_email_password"]').setValue('long enough');
  await wrapper.get('[data-testid="account_remove_email"]').trigger('click');
  await flushPromises();
  expect(removeEmail).toHaveBeenCalledWith('code');
  expect(wrapper.emitted('changed')?.[0]).toEqual([
    { username: 'admin', email: null, recovery: true },
  ]);
});

it('says when the mail with the code did not leave', async () => {
  vi.mocked(requestEmail).mockRejectedValue(new MailNotSent('down'));
  const wrapper = mount(AccountEmail, { props: { account, mail: ready } });
  await wrapper.get('[data-testid="account_new_email"]').setValue('me@example.com');
  await wrapper.get('[data-testid="account_email_password"]').setValue('long enough');
  await wrapper.get('form').trigger('submit');
  await flushPromises();
  expect(wrapper.text()).toContain(t('accountMailNotSent'));
  expect(wrapper.find('[data-testid="account_email_code"]').exists()).toBe(false);
});

it('is off until the Email app can send, and leads to it or to the App Manager', async () => {
  const shell = useShellStore();
  const open = vi.spyOn(shell, 'open').mockImplementation(() => {});
  const wrapper = mount(AccountEmail, {
    props: { account, mail: { installed: true, sender: false } },
  });
  expect(wrapper.get('[data-testid="account_email_unavailable"]').text()).toContain(
    t('accountEmailUnavailable'),
  );
  expect(wrapper.get('[data-testid="account_new_email"]').attributes('disabled')).toBeDefined();
  expect(wrapper.get('[data-testid="account_send_code"]').attributes('disabled')).toBeDefined();
  await wrapper.get('[data-testid="account_configure_mail"]').trigger('click');
  expect(open).toHaveBeenLastCalledWith('nas-mail-client', { location: '?section=smtp' });

  await wrapper.setProps({ mail: { installed: false, sender: false } });
  expect(wrapper.text()).toContain(t('accountEmailNotInstalled'));
  await wrapper.get('[data-testid="account_configure_mail"]').trigger('click');
  expect(open).toHaveBeenLastCalledWith('nas-app-manager');
});

it('turns the recovery by mail on with the password, and shows the address only then', async () => {
  vi.mocked(setRecovery).mockResolvedValue({ username: 'admin', email: null, recovery: true });
  const off = { username: 'admin', email: null, recovery: false };
  const wrapper = mount(AccountEmail, { props: { account: off, mail: ready } });
  expect(wrapper.find('[data-testid="account_new_email"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="account_recovery_form"]').exists()).toBe(false);

  await wrapper.get('[data-testid="account_recovery_input"]').setValue(true);
  await wrapper.get('[data-testid="account_recovery_password"]').setValue('long enough');
  await wrapper.get('[data-testid="account_recovery_form"]').trigger('submit');
  await flushPromises();
  expect(setRecovery).toHaveBeenCalledWith(true, 'code');
  expect(wrapper.emitted('changed')?.[0]).toEqual([
    { username: 'admin', email: null, recovery: true },
  ]);
  expect(wrapper.text()).toContain(t('accountRecoveryOn'));

  await wrapper.setProps({ account: { username: 'admin', email: null, recovery: true } });
  expect(wrapper.find('[data-testid="account_recovery_form"]').exists()).toBe(false);
  expect(wrapper.find('[data-testid="account_new_email"]').exists()).toBe(true);
});

it('cannot turn the recovery on while the Email app cannot send, and can always cancel', async () => {
  const off = { username: 'admin', email: null, recovery: false };
  const wrapper = mount(AccountEmail, {
    props: { account: off, mail: { installed: true, sender: false } },
  });
  expect(
    wrapper.get('[data-testid="account_recovery_input"]').attributes('disabled'),
  ).toBeDefined();

  await wrapper.setProps({ mail: ready });
  await wrapper.get('[data-testid="account_recovery_input"]').setValue(true);
  await wrapper.get('[data-testid="account_recovery_form"] .app_toolbar button').trigger('click');
  expect(wrapper.find('[data-testid="account_recovery_form"]').exists()).toBe(false);
  expect(setRecovery).not.toHaveBeenCalled();
});
