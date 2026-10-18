import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import {
  confirmPassword,
  hasSession,
  login,
  LoginRefused,
  LoginThrottled,
  logout,
  logoutUrl,
  PasswordRefused,
  saveSignInPolicy,
  signInPolicy,
} from '@/api/gateway';

const assign = vi.fn();
beforeEach(() => {
  vi.stubGlobal('fetch', vi.fn());
  vi.stubGlobal('location', { assign });
  assign.mockReset();
});
afterEach(() => vi.unstubAllGlobals());

it('sends the credentials to the gateway, which names the device from the handshake', async () => {
  vi.mocked(fetch).mockResolvedValue(
    new Response(JSON.stringify({ username: 'mock-user', device: 'Notebook' })),
  );

  await expect(login('admin', 'secret')).resolves.toEqual({
    username: 'mock-user',
    device: 'Notebook',
  });
  expect(fetch).toHaveBeenCalledWith(
    '/auth/login',
    expect.objectContaining({
      method: 'POST',
      credentials: 'include',
      body: JSON.stringify({ username: 'admin', password: 'secret' }),
      headers: { 'Content-Type': 'application/json' },
    }),
  );
});

it('accepts only a positive session answer and treats network errors as unauthenticated', async () => {
  vi.mocked(fetch).mockResolvedValueOnce(new Response(JSON.stringify({ authenticated: true })));
  expect(await hasSession()).toBe(true);
  vi.mocked(fetch).mockResolvedValueOnce(new Response('', { status: 401 }));
  expect(await hasSession()).toBe(false);
  vi.mocked(fetch).mockRejectedValueOnce(new Error('offline'));
  expect(await hasSession()).toBe(false);
});

it('tells wrong credentials apart from a gateway that does not answer', async () => {
  vi.mocked(fetch).mockResolvedValueOnce(new Response('', { status: 401 }));
  await expect(login('admin', 'secret')).rejects.toBeInstanceOf(LoginRefused);
  vi.mocked(fetch).mockResolvedValueOnce(new Response('', { status: 429 }));
  await expect(login('admin', 'secret')).rejects.toBeInstanceOf(LoginThrottled);
  vi.mocked(fetch).mockResolvedValueOnce(new Response('', { status: 502 }));
  await expect(login('admin', 'secret')).rejects.not.toBeInstanceOf(LoginRefused);
});

it('rejects a malformed success response', async () => {
  vi.mocked(fetch).mockResolvedValue(new Response(JSON.stringify({ success: true })));
  await expect(login('admin', 'secret')).rejects.toThrow('invalid login response');
});

it('signs in without a device on a connection that showed no certificate', async () => {
  vi.mocked(fetch).mockResolvedValue(new Response(JSON.stringify({ username: 'mock-user' })));
  await expect(login('admin', 'secret')).resolves.toEqual({ username: 'mock-user', device: null });
});

it('closes the session on the gateway route and then leaves the launcher', async () => {
  vi.mocked(fetch).mockResolvedValue(new Response(null, { status: 204 }));
  await logout();
  expect(logoutUrl).toBe('/auth/logout');
  expect(fetch).toHaveBeenCalledWith(
    logoutUrl,
    expect.objectContaining({ method: 'POST', credentials: 'include' }),
  );
  expect(assign).toHaveBeenCalledWith('/login');
});

it('keeps the user where they are when the gateway refuses logout', async () => {
  vi.mocked(fetch).mockResolvedValue(new Response('', { status: 502 }));
  await expect(logout()).rejects.toThrow('502');
  expect(assign).not.toHaveBeenCalled();
});

it('reads and saves the lock after failed sign-ins on the gateway', async () => {
  const lock = { enabled: true, attempts: 3, lockMinutes: 10 };
  vi.mocked(fetch).mockImplementation(() => Promise.resolve(new Response(JSON.stringify(lock))));
  await expect(signInPolicy()).resolves.toEqual(lock);
  await expect(saveSignInPolicy(lock)).resolves.toEqual(lock);
  expect(fetch).toHaveBeenLastCalledWith(
    '/auth/policy',
    expect.objectContaining({ method: 'PUT', body: JSON.stringify(lock) }),
  );
  vi.mocked(fetch).mockResolvedValue(new Response('', { status: 401 }));
  await expect(signInPolicy()).rejects.toThrow('401');
});

it('checks the password again and hands back the code for one request', async () => {
  vi.mocked(fetch).mockResolvedValue(new Response(JSON.stringify({ code: 'c0de' })));

  await expect(confirmPassword('long enough')).resolves.toBe('c0de');
  expect(fetch).toHaveBeenCalledWith(
    '/auth/confirm',
    expect.objectContaining({
      method: 'POST',
      credentials: 'include',
      body: JSON.stringify({ password: 'long enough' }),
    }),
  );
});

it('tells a wrong password from an address that has to wait', async () => {
  vi.mocked(fetch).mockResolvedValueOnce(new Response('{}', { status: 403 }));
  await expect(confirmPassword('wrong')).rejects.toBeInstanceOf(PasswordRefused);

  vi.mocked(fetch).mockResolvedValueOnce(new Response('{}', { status: 429 }));
  await expect(confirmPassword('wrong')).rejects.toBeInstanceOf(LoginThrottled);

  vi.mocked(fetch).mockResolvedValueOnce(new Response(JSON.stringify({ token: 'other' })));
  await expect(confirmPassword('right')).rejects.toThrow('invalid confirmation response');
});
