import { beforeEach, describe, expect, it, vi } from 'vitest';
import { diskCall } from '@/api/disks';
import { request } from '@/api/client';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  vi.mocked(request).mockReset().mockResolvedValue(['ab12-cd34']);
});

describe('what the disk manager may ask about the external disks', () => {
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
    expect(diskCall('lock-disk', { uuid: 'a b', locked: true })).toBeNull();
    expect(diskCall('lock-disk', { uuid: 'ab12', locked: 'yes' })).toBeNull();
    expect(diskCall('format-disk' as 'eject-disk', { device: 'sdb' })).toBeNull();
    expect(request).not.toHaveBeenCalled();
  });
});
