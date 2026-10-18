import { beforeEach, expect, it, vi } from 'vitest';
import { usePasswordRecovery } from '@/composables/usePasswordRecovery';
import { CodeRefused, requestPasswordReset, resetPassword } from '@/api/account';
import { LoginThrottled } from '@/api/gateway';
import { setLocale, t } from '@/i18n';

vi.mock('@/api/account', () => ({
  requestPasswordReset: vi.fn(),
  resetPassword: vi.fn(),
  CodeRefused: class extends Error {},
}));
vi.mock('@/api/gateway', () => ({ LoginThrottled: class extends Error {} }));

beforeEach(() => {
  setLocale('en');
  vi.clearAllMocks();
});

it('asks for the code, then sets the new password with it', async () => {
  const recovery = usePasswordRecovery();
  recovery.username.value = ' admin ';
  await recovery.send();
  expect(requestPasswordReset).toHaveBeenCalledWith('admin');
  expect(recovery.step.value).toBe('reset');

  recovery.code.value = '12345';
  recovery.password.value = 'a new password';
  recovery.repeated.value = 'a new passwor';
  expect(recovery.mismatch.value).toBe(true);
  expect(recovery.ready.value).toBe(false);
  recovery.code.value = ' 123456 ';
  recovery.repeated.value = 'a new password';
  expect(recovery.ready.value).toBe(true);

  await recovery.reset();
  expect(resetPassword).toHaveBeenCalledWith('admin', '123456', 'a new password');
  expect(recovery.step.value).toBe('done');
  expect(recovery.password.value).toBe('');

  recovery.restart();
  expect(recovery.step.value).toBe('request');
  expect(recovery.username.value).toBe(' admin ');
});

it('says why a step did not go on', async () => {
  const recovery = usePasswordRecovery();
  vi.mocked(requestPasswordReset).mockRejectedValueOnce(new LoginThrottled('wait'));
  await recovery.send();
  expect(recovery.error.value).toBe(t('passwordRecoveryThrottled'));
  expect(recovery.step.value).toBe('request');

  vi.mocked(resetPassword).mockRejectedValueOnce(new CodeRefused('wrong'));
  await recovery.reset();
  expect(recovery.error.value).toBe(t('passwordRecoveryRefused'));
  expect(recovery.busy.value).toBe(false);
});
