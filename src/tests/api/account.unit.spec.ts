import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import {
  changePassword,
  changeUsername,
  CodeRefused,
  mailStatus,
  MailNotSent,
  readAccount,
  recoveryOffered,
  removeEmail,
  requestEmail,
  requestPasswordReset,
  resetPassword,
  setRecovery,
  verifyEmail,
} from '@/api/account';
import { request } from '@/api/client';
import { LoginThrottled } from '@/api/gateway';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const account = { username: 'admin', email: 'me@example.com', recovery: true };

function answer(body: unknown, status = 200) {
  return new Response(body === undefined ? null : JSON.stringify(body), { status });
}

beforeEach(() => vi.stubGlobal('fetch', vi.fn()));
afterEach(() => vi.unstubAllGlobals());

it('reads and changes the account on the gateway, with the confirmation where it is needed', async () => {
  vi.mocked(fetch)
    .mockResolvedValueOnce(answer(account))
    .mockResolvedValueOnce(answer({ ...account, username: 'keeper' }))
    .mockResolvedValueOnce(answer(undefined, 204))
    .mockResolvedValueOnce(answer(undefined, 202))
    .mockResolvedValueOnce(answer(account))
    .mockResolvedValueOnce(answer(undefined, 204));

  await expect(readAccount()).resolves.toEqual(account);
  await expect(changeUsername('keeper', 'c1')).resolves.toMatchObject({ username: 'keeper' });
  await changePassword('a new password', 'c2');
  await requestEmail('me@example.com', 'c3');
  await expect(verifyEmail('123456')).resolves.toEqual(account);
  await removeEmail('c4');

  const calls = vi
    .mocked(fetch)
    .mock.calls.map(([url, init]) => [
      url,
      init?.method,
      (init?.headers as Record<string, string>)['X-NAS-Confirmation'],
      init?.body,
    ]);
  expect(calls).toEqual([
    ['/auth/account', 'GET', undefined, undefined],
    ['/auth/account/username', 'PUT', 'c1', '{"username":"keeper"}'],
    ['/auth/account/password', 'PUT', 'c2', '{"password":"a new password"}'],
    ['/auth/account/email', 'POST', 'c3', '{"email":"me@example.com"}'],
    ['/auth/account/email/verify', 'POST', undefined, '{"code":"123456"}'],
    ['/auth/account/email', 'DELETE', 'c4', undefined],
  ]);
});

it('asks for and uses the reset code without a session', async () => {
  vi.mocked(fetch)
    .mockResolvedValueOnce(answer(undefined, 202))
    .mockResolvedValueOnce(answer(undefined, 204));
  await requestPasswordReset('admin');
  await resetPassword('admin', '123456', 'a new password');
  expect(vi.mocked(fetch).mock.calls.map(([url, init]) => [url, init?.body])).toEqual([
    ['/auth/recovery', '{"username":"admin"}'],
    ['/auth/recovery/reset', '{"username":"admin","code":"123456","password":"a new password"}'],
  ]);
});

it('tells a refused code, an unsent mail and too many attempts apart', async () => {
  vi.mocked(fetch)
    .mockResolvedValueOnce(answer({ error: 'the code is wrong or expired' }, 400))
    .mockResolvedValueOnce(answer({ error: 'the mail with the code could not be sent' }, 502))
    .mockResolvedValueOnce(answer(undefined, 429))
    .mockResolvedValueOnce(answer({ error: 'the username is not valid' }, 400))
    .mockResolvedValueOnce(answer(undefined, 500))
    .mockResolvedValueOnce(answer({ username: 1 }));

  await expect(verifyEmail('000000')).rejects.toBeInstanceOf(CodeRefused);
  await expect(requestEmail('me@example.com', 'c')).rejects.toBeInstanceOf(MailNotSent);
  await expect(requestPasswordReset('admin')).rejects.toBeInstanceOf(LoginThrottled);
  await expect(changeUsername('x', 'c')).rejects.toThrow('the username is not valid');
  await expect(readAccount()).rejects.toThrow('(500)');
  await expect(readAccount()).rejects.toThrow('invalid account response');
});

it('asks the launcher whether the Email app can send', async () => {
  vi.mocked(request).mockResolvedValue({ installed: true, sender: false });
  await expect(mailStatus()).resolves.toEqual({ installed: true, sender: false });
  expect(request).toHaveBeenCalledWith('/mail/status');
});

it('switches the recovery by mail and asks whether the sign-in page offers it', async () => {
  vi.mocked(fetch)
    .mockResolvedValueOnce(answer({ ...account, recovery: false }))
    .mockResolvedValueOnce(answer({ enabled: true }))
    .mockResolvedValueOnce(answer({ enabled: 'yes' }))
    .mockRejectedValueOnce(new Error('offline'));
  await expect(setRecovery(false, 'c5')).resolves.toMatchObject({ recovery: false });
  expect(vi.mocked(fetch).mock.calls[0]?.[0]).toBe('/auth/account/recovery');
  await expect(recoveryOffered()).resolves.toBe(true);
  await expect(recoveryOffered()).resolves.toBe(false);
  await expect(recoveryOffered()).resolves.toBe(false);
});
