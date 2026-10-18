import { beforeEach, describe, expect, it, vi } from 'vitest';
import { diskCall } from '@/api/disks';
import { request } from '@/api/client';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  vi.mocked(request).mockReset().mockResolvedValue(['ab12-cd34']);
});

describe('what the disk manager may ask about the disks', () => {
  it('lists the local or the external disks', async () => {
    await diskCall('disk-inventory', { kind: 'external' })?.();

    expect(request).toHaveBeenCalledWith('/storage/disks?kind=external');
  });

  it('ejects a disk by its device name, waiting for the root unit', async () => {
    await diskCall('eject-disk', { device: 'sdb1' })?.();

    expect(request).toHaveBeenCalledWith('/storage/disks/eject', {
      method: 'POST',
      body: JSON.stringify({ device: 'sdb1' }),
      signal: expect.any(AbortSignal),
    });
  });

  it('reads and changes the marks of the disks not to be ejected', async () => {
    await expect(diskCall('disk-locks', {})?.()).resolves.toEqual(['ab12-cd34']);
    await diskCall('lock-disk', { uuid: 'AB12-CD34', locked: true })?.();

    expect(request).toHaveBeenCalledWith('/storage/locks');
    expect(request).toHaveBeenCalledWith('/storage/locks', {
      method: 'POST',
      body: JSON.stringify({ uuid: 'AB12-CD34', locked: true }),
    });
  });

  it('asks nothing for what is not valid', () => {
    expect(diskCall('eject-disk', { device: '../sda' })).toBeNull();
    expect(diskCall('eject-disk', {})).toBeNull();
    expect(diskCall('disk-inventory', { kind: 'network' })).toBeNull();
    expect(diskCall('disk-inventory', {})).toBeNull();
    expect(diskCall('lock-disk', { uuid: 'a b', locked: true })).toBeNull();
    expect(diskCall('lock-disk', { uuid: 'ab12', locked: 'yes' })).toBeNull();
    expect(diskCall('format-disk' as 'eject-disk', { device: 'sdb' })).toBeNull();
    expect(request).not.toHaveBeenCalled();
  });
});

describe('what the disk manager may ask about encryption', () => {
  it('says whether encryption is on', async () => {
    await diskCall('disk-encryption', {})?.();

    expect(request).toHaveBeenCalledWith('/storage/encryption');
  });

  it('says whether the system is encrypted and has a TPM', async () => {
    await diskCall('disk-security', {})?.();

    expect(request).toHaveBeenCalledWith('/storage/security');
  });

  it('sends a request of encryption and follows a format to its end', async () => {
    vi.mocked(request)
      .mockReset()
      .mockResolvedValueOnce({ taskId: 'format-storage' })
      .mockResolvedValueOnce({ state: 'done', detail: '' });
    const format = {
      op: 'format',
      device: 'sdb',
      identity: 'wwn/S1/1024',
      label: 'Data',
      encrypt: true,
      automatic: true,
    };

    await expect(diskCall('disk-crypt', { request: format })?.()).resolves.toEqual({
      taskId: 'format-storage',
    });
    expect(request).toHaveBeenCalledWith('/storage/encryption', {
      method: 'POST',
      body: JSON.stringify(format),
      signal: expect.any(AbortSignal),
    });
    expect(request).toHaveBeenCalledWith('/tasks/format-storage');
  });

  it('says why a format failed', async () => {
    vi.mocked(request)
      .mockReset()
      .mockResolvedValueOnce({ taskId: 'format-storage' })
      .mockResolvedValueOnce({ state: 'failed', detail: 'mkfs failed' });

    await expect(
      diskCall('disk-crypt', { request: { op: 'lock', uuid: 'ab' } })?.(),
    ).rejects.toThrow('mkfs failed');
  });

  it('keeps a password off plain HTTP, but not off the machine itself', async () => {
    vi.stubGlobal('location', { protocol: 'http:', hostname: 'nas.local' });
    const unlock = { op: 'unlock', uuid: 'ab', password: 'long enough' };
    await expect(diskCall('disk-crypt', { request: unlock })?.()).rejects.toThrow('HTTPS');
    expect(request).not.toHaveBeenCalled();

    vi.stubGlobal('location', { protocol: 'http:', hostname: '127.0.0.1' });
    await diskCall('disk-crypt', { request: unlock })?.();
    expect(request).toHaveBeenCalledTimes(1);
    vi.unstubAllGlobals();
  });

  it('asks nothing for a request it does not know or that is not flat', () => {
    expect(diskCall('disk-crypt', {})).toBeNull();
    expect(diskCall('disk-crypt', { request: [] })).toBeNull();
    expect(diskCall('disk-crypt', { request: { op: 'wipe', uuid: 'ab' } })).toBeNull();
    expect(diskCall('disk-crypt', { request: { op: 'lock', uuid: { nested: 1 } } })).toBeNull();
    expect(request).not.toHaveBeenCalled();
  });
});

describe('what the disk manager may ask about the network disks', () => {
  it('lists them and hands a request over, its password only on a secure page', async () => {
    await diskCall('smb-disks', {})?.();
    expect(request).toHaveBeenCalledWith('/storage/network');

    const connect = { op: 'networkConnect', id: 'n-0a1b2c3d' };
    await diskCall('smb-disk', { request: connect })?.();
    expect(request).toHaveBeenCalledWith('/storage/network', {
      method: 'POST',
      body: JSON.stringify(connect),
      signal: expect.any(AbortSignal),
    });

    vi.stubGlobal('location', { protocol: 'http:', hostname: 'nas.local' });
    const save = { op: 'networkSave', name: 'Office', password: 'secret' };
    await expect(diskCall('smb-disk', { request: save })?.()).rejects.toThrow('HTTPS');
    vi.unstubAllGlobals();
    expect(diskCall('smb-disk', { request: { op: 'lock', uuid: 'ab' } })).toBeNull();
    expect(diskCall('disk-crypt', { request: connect })).toBeNull();
  });
});
