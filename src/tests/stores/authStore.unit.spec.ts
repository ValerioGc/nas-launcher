import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAuthStore } from '@/stores/authStore';
import { hasSession, login as gatewayLogin, LoginRefused, LoginThrottled } from '@/api/gateway';
import { request } from '@/api/client';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
vi.mock('@/api/gateway', () => ({
  login: vi.fn(),
  hasSession: vi.fn(),
  LoginRefused: class extends Error {},
  LoginThrottled: class extends Error {},
}));

describe('auth store', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.mocked(request).mockReset().mockResolvedValue(undefined);
    vi.mocked(gatewayLogin).mockReset().mockResolvedValue({
      username: 'mock-user',
      device: null,
    });
    vi.mocked(hasSession).mockReset().mockResolvedValue(true);
  });

  it.each([
    ['', 'long enough'],
    ['admin', ''],
    ['   ', 'long enough'],
    ['admin', '   '],
    ['ab', 'long enough'],
    ['admin', 'ab'],
    ['a'.repeat(26), 'long enough'],
    ['admin', 'a'.repeat(129)],
    ['admin', 'short'],
  ])(
    'rejects invalid credentials (%j, %j) before contacting the gateway',
    async (username, password) => {
      const store = useAuthStore();
      await expect(store.login(username, password)).resolves.toMatchObject({
        success: false,
        code: 'invalid_input',
      });
      expect(gatewayLogin).not.toHaveBeenCalled();
      expect(store.isLoading).toBe(false);
    },
  );

  it('uses the gateway result without keeping the password', async () => {
    const store = useAuthStore();
    await expect(store.login(' admin ', 'long enough')).resolves.toEqual({
      success: true,
      username: 'mock-user',
    });
    expect(gatewayLogin).toHaveBeenCalledWith('admin', 'long enough');
    expect(store.authenticated).toBe(true);
    expect(JSON.stringify(store.$state)).not.toContain('long enough');
  });

  it('reports a network failure and does not mark the user authenticated', async () => {
    vi.mocked(gatewayLogin).mockRejectedValueOnce(new Error('offline'));
    const store = useAuthStore();
    await expect(store.login('admin', 'long enough')).resolves.toMatchObject({
      code: 'unavailable',
    });
    expect(store.authenticated).toBe(false);
  });

  it('does not accept a login without a server session', async () => {
    vi.mocked(hasSession).mockResolvedValueOnce(false);
    const store = useAuthStore();
    await expect(store.login('admin', 'long enough')).resolves.toMatchObject({
      code: 'unavailable',
    });
    expect(store.authenticated).toBe(false);
  });

  it('rejects concurrent requests while one login is pending', async () => {
    let resolve!: (value: { username: string; device: string | null }) => void;
    vi.mocked(gatewayLogin).mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const store = useAuthStore();
    const pending = store.login('admin', 'long enough');
    await expect(store.login('guest', 'long enough')).resolves.toMatchObject({ code: 'busy' });
    resolve({ username: 'mock-user', device: null });
    await expect(pending).resolves.toMatchObject({ success: true });
    expect(store.isLoading).toBe(false);
  });

  it('checks the server session instead of trusting the previous result', async () => {
    const store = useAuthStore();
    await store.login('admin', 'long enough');
    expect(store.authenticated).toBe(true);
    vi.mocked(hasSession).mockResolvedValueOnce(false);
    expect(await store.checkSession()).toBe(false);
    expect(store.authenticated).toBe(false);
  });

  it('says the credentials are wrong when the gateway refuses them, and logs it', async () => {
    vi.mocked(gatewayLogin).mockRejectedValueOnce(new LoginRefused('refused'));
    const store = useAuthStore();
    await expect(store.login('admin', 'long enough')).resolves.toMatchObject({
      success: false,
      code: 'refused',
    });
    expect(store.authenticated).toBe(false);
    expect(request).toHaveBeenCalledWith('/access/failures', {
      method: 'POST',
      body: JSON.stringify({ field: 'password' }),
    });
  });

  it('says to wait when the gateway throttles the address', async () => {
    vi.mocked(gatewayLogin).mockRejectedValueOnce(new LoginThrottled('throttled'));
    const store = useAuthStore();
    await expect(store.login('admin', 'long enough')).resolves.toMatchObject({
      success: false,
      code: 'throttled',
    });
    expect(store.authenticated).toBe(false);
  });
});
