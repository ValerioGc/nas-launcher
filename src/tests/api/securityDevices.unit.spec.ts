import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEVICE_CALLS } from '@/api/securityDevices';
import { accessCall } from '@/api/access';
import { confirmPassword } from '@/api/gateway';

vi.mock('@/api/gateway', async (original) => ({
  ...(await original<typeof import('@/api/gateway')>()),
  confirmPassword: vi.fn(),
}));

function answer(body: unknown, status = 200) {
  return new Response(status === 204 ? null : JSON.stringify(body), { status });
}

beforeEach(() => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(answer([])));
  vi.mocked(confirmPassword).mockReset().mockResolvedValue('c0de');
});

afterEach(() => vi.unstubAllGlobals());

const sent = () =>
  vi.mocked(fetch).mock.calls.map(([path, init]) => ({
    path,
    method: (init as RequestInit).method ?? 'GET',
    body: (init as RequestInit).body,
    headers: (init as RequestInit).headers,
  }));

describe('what the security center may ask about the devices', () => {
  it('reads the switch and the devices from the gateway', async () => {
    await accessCall('mtls-state', undefined)?.();
    await accessCall('devices', undefined)?.();

    expect(sent().map(({ path }) => path)).toEqual(['/auth/mtls', '/auth/devices']);
    expect(confirmPassword).not.toHaveBeenCalled();
  });

  it('confirms the password before issuing a certificate or turning mTLS on', async () => {
    vi.mocked(fetch).mockResolvedValue(answer({ device: { id: 3 }, credential: {} }, 201));
    await DEVICE_CALLS['add-device']({
      name: 'Notebook',
      icon: 'laptop',
      password: 'long enough',
    })?.();
    await DEVICE_CALLS['renew-device']({ id: 3, password: 'long enough' })?.();
    await DEVICE_CALLS['save-mtls']({ enabled: true, password: 'long enough' })?.();

    expect(confirmPassword).toHaveBeenCalledTimes(3);
    expect(sent()).toEqual([
      {
        path: '/auth/devices',
        method: 'POST',
        body: JSON.stringify({ name: 'Notebook', icon: 'laptop' }),
        headers: { 'Content-Type': 'application/json', 'X-NAS-Confirmation': 'c0de' },
      },
      {
        path: '/auth/devices/3/renew',
        method: 'POST',
        body: undefined,
        headers: { 'Content-Type': 'application/json', 'X-NAS-Confirmation': 'c0de' },
      },
      {
        path: '/auth/mtls',
        method: 'PUT',
        body: JSON.stringify({ enabled: true }),
        headers: { 'Content-Type': 'application/json', 'X-NAS-Confirmation': 'c0de' },
      },
    ]);
  });

  it('renames, revokes and removes a device with the session alone', async () => {
    vi.mocked(fetch)
      .mockResolvedValueOnce(answer({ id: 3 }))
      .mockResolvedValueOnce(answer({ id: 3 }))
      .mockResolvedValueOnce(answer(null, 204));
    await DEVICE_CALLS['update-device']({ id: 3, name: 'Tv sala', icon: 'tv' })?.();
    await DEVICE_CALLS['revoke-device']({ id: 3 })?.();
    await expect(DEVICE_CALLS['remove-device']({ id: 3 })?.()).resolves.toBeNull();

    expect(sent().map(({ path, method }) => `${method} ${path}`)).toEqual([
      'PUT /auth/devices/3',
      'POST /auth/devices/3/revoke',
      'DELETE /auth/devices/3',
    ]);
  });

  it('says why the gateway refused', async () => {
    vi.mocked(fetch).mockResolvedValue(
      answer({ error: 'this is the device in use: act on it from another one' }, 409),
    );
    await expect(DEVICE_CALLS['revoke-device']({ id: 3 })?.()).rejects.toThrow('device in use');
    vi.mocked(fetch).mockResolvedValue(new Response('down', { status: 502 }));
    await expect(DEVICE_CALLS.devices(undefined)?.()).rejects.toThrow('502');
  });

  it('asks nothing for what is not a device or carries no password', () => {
    for (const [type, payload] of [
      ['add-device', { name: '', icon: 'laptop', password: 'x' }],
      ['add-device', { name: 'Tv', icon: 'toaster', password: 'x' }],
      ['add-device', { name: 'Tv', icon: 'tv' }],
      ['add-device', { name: 'x'.repeat(61), icon: 'tv', password: 'x' }],
      ['renew-device', { id: 3 }],
      ['renew-device', { id: '3', password: 'x' }],
      ['revoke-device', { id: -1 }],
      ['update-device', { id: 3, name: 'Tv' }],
      ['save-mtls', { enabled: 'yes', password: 'x' }],
      ['save-mtls', { enabled: true }],
    ] as const) {
      expect(DEVICE_CALLS[type](payload), type).toBeNull();
    }
    expect(fetch).not.toHaveBeenCalled();
  });
});
