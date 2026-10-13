import { beforeEach, expect, it, vi } from 'vitest';
import { accessCall } from '@/api/access';
import { ANTIVIRUS_CALLS } from '@/api/antivirus';
import { request } from '@/api/client';
vi.mock('@/api/client', () => ({ request: vi.fn() }));
beforeEach(() => vi.mocked(request).mockReset());
it('uses the existing authenticated client for antivirus operations', async () => {
  await accessCall('antivirus-status', undefined)?.();
  await accessCall('antivirus-enable', { enabled: true })?.();
  await accessCall('antivirus-scan', { kind: 'file', paths: ['/srv/storage/a'] })?.();
  for (const action of ['quarantine', 'restore', 'delete'] as const) {
    await ANTIVIRUS_CALLS[`antivirus-${action}`]({ id: '1-2' })?.();
  }
  expect(request).toHaveBeenCalledWith('/antivirus');
  expect(request).toHaveBeenCalledWith('/antivirus/enabled', {
    method: 'POST',
    body: '{"enabled":true}',
  });
  expect(request).toHaveBeenCalledWith('/antivirus/scan', {
    method: 'POST',
    body: '{"kind":"file","paths":["/srv/storage/a"]}',
  });
});
it('passes the choice to skip unchanged files and forgets the files already scanned', async () => {
  await accessCall('antivirus-scan', { kind: 'custom', paths: ['/srv'], skipUnchanged: true })?.();
  await accessCall('antivirus-forget', undefined)?.();
  expect(request).toHaveBeenCalledWith('/antivirus/scan', {
    method: 'POST',
    body: '{"kind":"custom","paths":["/srv"],"skipUnchanged":true}',
  });
  expect(request).toHaveBeenCalledWith('/antivirus/scanned/forget', { method: 'POST' });
  expect(
    ANTIVIRUS_CALLS['antivirus-scan']({ kind: 'custom', paths: ['/srv'], skipUnchanged: 'yes' }),
  ).toBeNull();
});
it('accepts Unicode filenames without treating supplementary characters as controls', async () => {
  const payload = { kind: 'file', paths: ['/srv/storage/🛡️-文档.txt'] };
  await ANTIVIRUS_CALLS['antivirus-scan'](payload)?.();
  expect(request).toHaveBeenCalledWith('/antivirus/scan', {
    method: 'POST',
    body: JSON.stringify(payload),
  });
});
it('refuses malformed payloads without calling the machine', () => {
  for (const payload of [
    null,
    {},
    { kind: 'shell', paths: ['/'] },
    { kind: 'file', paths: ['/a/../etc'] },
    { kind: 'file', paths: ['relative'] },
    { kind: 'file', paths: ['/a\n'] },
    { kind: 'file', paths: ['/a\u0000'] },
    { kind: 'file', paths: ['/a\u007f'] },
  ]) {
    expect(ANTIVIRUS_CALLS['antivirus-scan'](payload)).toBeNull();
  }
  expect(ANTIVIRUS_CALLS['antivirus-enable']({ enabled: 'yes' })).toBeNull();
  expect(ANTIVIRUS_CALLS['antivirus-delete']({ id: '../etc' })).toBeNull();
  expect(ANTIVIRUS_CALLS['antivirus-settings']({})).toBeNull();
  expect(request).not.toHaveBeenCalled();
});
it('validates all scan options and scheduled jobs', async () => {
  const options = { archives: true, detectPua: false, maxFileMb: 100, paths: [], exclusions: [] };
  const settings = {
    enabled: false,
    schedulerEnabled: true,
    bootScan: { enabled: false, kind: 'quick', paths: [] },
    shutdownScan: { enabled: false, kind: 'quick', paths: [] },
    autoQuarantine: false,
    full: options,
    quick: options,
    file: options,
    custom: options,
    schedules: [{ id: 'one', enabled: true, kind: 'quick', day: 'Mon', time: '03:00', paths: [] }],
  };
  await ANTIVIRUS_CALLS['antivirus-settings'](settings)?.();
  expect(request).toHaveBeenCalledWith('/antivirus/settings', {
    method: 'PUT',
    body: JSON.stringify(settings),
  });
  expect(
    ANTIVIRUS_CALLS['antivirus-settings']({ ...settings, custom: { ...options, maxFileMb: 0 } }),
  ).toBeNull();
  expect(
    ANTIVIRUS_CALLS['antivirus-settings']({
      ...settings,
      schedules: [{ ...settings.schedules[0], time: '25:00' }],
    }),
  ).toBeNull();
});

it('rejects invalid lifecycle scans and accepts configured scopes', async () => {
  const options = { archives: true, detectPua: false, maxFileMb: 100, paths: [], exclusions: [] };
  const settings = {
    enabled: false,
    schedulerEnabled: false,
    autoQuarantine: false,
    bootScan: { enabled: true, kind: 'full', paths: [] },
    shutdownScan: { enabled: true, kind: 'file', paths: ['/srv/storage/a'] },
    full: options,
    quick: options,
    file: options,
    custom: options,
    schedules: [],
  };
  expect(ANTIVIRUS_CALLS['antivirus-settings'](settings)).not.toBeNull();
  for (const selected of [
    undefined,
    {},
    { enabled: true, kind: 'file', paths: [] },
    { enabled: true, kind: 'file', paths: ['/a', '/b'] },
    { enabled: true, kind: 'custom', paths: [] },
  ]) {
    expect(ANTIVIRUS_CALLS['antivirus-settings']({ ...settings, bootScan: selected })).toBeNull();
  }
});
