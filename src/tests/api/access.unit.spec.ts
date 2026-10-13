import { beforeEach, describe, expect, it, vi } from 'vitest';
import { accessCall } from '@/api/access';
import { request } from '@/api/client';
import { saveSignInPolicy, signInPolicy } from '@/api/gateway';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
vi.mock('@/api/gateway', () => ({ signInPolicy: vi.fn(), saveSignInPolicy: vi.fn() }));

beforeEach(() => {
  vi.mocked(request).mockReset().mockResolvedValue(undefined);
  vi.mocked(signInPolicy)
    .mockReset()
    .mockResolvedValue({ enabled: true, attempts: 5, lockMinutes: 1 });
  vi.mocked(saveSignInPolicy)
    .mockReset()
    .mockImplementation((policy) => Promise.resolve(policy));
});

describe('what the security center may ask', () => {
  it('reads the settings, the rules and the lock where the console reads them', async () => {
    await accessCall('access-settings', undefined)?.();
    await accessCall('access-rules', undefined)?.();
    await accessCall('discovery-settings', undefined)?.();
    await expect(accessCall('sign-in-policy', undefined)?.()).resolves.toMatchObject({
      attempts: 5,
    });

    expect(request).toHaveBeenCalledWith('/settings/access');
    expect(request).toHaveBeenCalledWith('/access/rules');
    expect(request).toHaveBeenCalledWith('/settings/discovery');
  });

  it('saves only the fields the launcher takes', async () => {
    await accessCall('save-access-settings', { notifyFailures: true, extra: 'x' })?.();
    await accessCall('add-access-rule', {
      kind: 'range',
      value: '192.168.1.0/24',
      label: 'Casa',
      createdAt: 1,
    })?.();
    await accessCall('remove-access-rule', { id: '12' })?.();
    await accessCall('save-discovery-settings', { enabled: true, port: 22 })?.();
    await accessCall('save-sign-in-policy', { enabled: false, attempts: 3, lockMinutes: 10 })?.();

    expect(request).toHaveBeenCalledWith('/settings/access', {
      method: 'PUT',
      body: JSON.stringify({ notifyFailures: true }),
    });
    expect(request).toHaveBeenCalledWith('/access/rules', {
      method: 'POST',
      body: JSON.stringify({ kind: 'range', value: '192.168.1.0/24', label: 'Casa' }),
    });
    expect(request).toHaveBeenCalledWith('/access/rules/12', { method: 'DELETE' });
    expect(request).toHaveBeenCalledWith('/settings/discovery', {
      method: 'PUT',
      body: JSON.stringify({ enabled: true }),
    });
    expect(saveSignInPolicy).toHaveBeenCalledWith({ enabled: false, attempts: 3, lockMinutes: 10 });
  });

  it('refuses what is not a request the launcher takes', () => {
    for (const [type, payload] of [
      ['save-access-settings', { notifyFailures: 'yes' }],
      ['save-access-settings', null],
      ['save-discovery-settings', { enabled: 'on' }],
      ['save-discovery-settings', [true]],
      ['add-access-rule', { kind: 'host', value: '10.0.0.1', label: '' }],
      ['add-access-rule', { kind: 'ip', value: 'x'.repeat(65), label: '' }],
      ['add-access-rule', { kind: 'ip', value: '10.0.0.1' }],
      ['remove-access-rule', { id: '../settings' }],
      ['remove-access-rule', { id: 7 }],
      ['save-sign-in-policy', { enabled: true, attempts: 2.5, lockMinutes: 1 }],
      ['save-sign-in-policy', { enabled: true, attempts: -1, lockMinutes: 1 }],
      ['save-sign-in-policy', [true, 1, 1]],
    ] as const) {
      expect(accessCall(type, payload), `${type} ${JSON.stringify(payload)}`).toBeNull();
    }
  });

  it('has no call for a request it does not know', () => {
    expect(accessCall('toString' as never, undefined)).toBeNull();
    expect(accessCall('drop-everything' as never, {})).toBeNull();
    expect(request).not.toHaveBeenCalled();
  });

  it('lists the refused sign-ins out of the notifications, and nothing else', async () => {
    vi.mocked(request).mockResolvedValue([
      {
        id: '4',
        key: 'signInRefused',
        at: 1_700_000_000_000,
        params: { from: '192.168.1.9', field: 'password' },
      },
      { id: '5', key: 'settingsSaved', at: 1_700_000_100_000, params: { section: 'general' } },
      { id: '6', key: 'signInRefused', at: 1_700_000_200_000 },
    ]);

    await expect(accessCall('refused-sign-ins', undefined)?.()).resolves.toEqual([
      { id: '4', at: 1_700_000_000_000, from: '192.168.1.9', field: 'password' },
      { id: '6', at: 1_700_000_200_000, from: '', field: '' },
    ]);
    expect(request).toHaveBeenCalledWith('/notifications?limit=500');
  });
});
