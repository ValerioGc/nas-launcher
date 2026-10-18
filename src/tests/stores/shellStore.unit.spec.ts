import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { useShellStore } from '@/stores/shellStore';
import { useAppsStore } from '@/stores/appsStore';
import { request } from '@/api/client';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const list = ['nas-app-manager', 'nas-disk-manager', 'nas-global-search'].map((id) => ({
  id,
  name: id,
  info: '',
}));

beforeEach(() => {
  sessionStorage.clear();
  setActivePinia(createPinia());
  vi.mocked(request).mockReset();
  vi.mocked(request).mockResolvedValue(undefined as never);
  vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('not running')));
});

afterEach(() => {
  sessionStorage.clear();
  vi.unstubAllGlobals();
});

async function ready(installed = list) {
  vi.mocked(request).mockResolvedValueOnce(installed).mockResolvedValueOnce([]);
  const apps = useAppsStore();
  const shell = useShellStore();
  await apps.fetchInstalled();
  return { apps, shell };
}

describe('open applications', () => {
  it('restores open tabs and the active app after a page reload', async () => {
    const first = await ready();
    first.shell.open('nas-app-manager');
    first.shell.open('nas-disk-manager', { location: '?disk=sda' });
    first.shell.open('nas-global-search', { query: 'photo' });
    await vi.waitFor(() => expect(first.shell.starting).toEqual([]));

    setActivePinia(createPinia());
    vi.mocked(request).mockClear().mockResolvedValueOnce(list).mockResolvedValueOnce([]);
    const apps = useAppsStore();
    const shell = useShellStore();
    await apps.fetchInstalled();
    await nextTick();

    expect(shell.openApps.map((app) => app.id)).toEqual(['nas-app-manager', 'nas-disk-manager']);
    expect(shell.activeId).toBe('nas-disk-manager');
    expect(shell.modalId).toBeNull();
    expect(shell.launchQuery).toBe('');
    expect(shell.locations).toEqual({});
    await vi.waitFor(() => expect(shell.starting).toEqual([]));

    expect(request).toHaveBeenCalledWith('/apps/nas-disk-manager/start', expect.anything());
    expect(fetch).not.toHaveBeenCalled();
  });

  it('starts a restored external app when its process is no longer answering', async () => {
    const first = await ready();
    first.shell.open('nas-disk-manager');
    await vi.waitFor(() => expect(first.shell.starting).toEqual([]));

    setActivePinia(createPinia());
    vi.mocked(request).mockClear().mockResolvedValueOnce(list).mockResolvedValueOnce([]);
    const apps = useAppsStore();
    const shell = useShellStore();
    await apps.fetchInstalled();
    await vi.waitFor(() => expect(shell.starting).toEqual([]));

    expect(request).toHaveBeenCalledWith(
      '/apps/nas-disk-manager/start',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(shell.openApps.map((app) => app.id)).toEqual(['nas-disk-manager']);
  });

  it('does not restore a tab after its external process was stopped and closed', async () => {
    const first = await ready();
    first.shell.open('nas-disk-manager');
    await vi.waitFor(() => expect(first.shell.starting).toEqual([]));
    await first.shell.close('nas-disk-manager');
    expect(request).toHaveBeenCalledWith(
      '/apps/nas-disk-manager/close',
      expect.objectContaining({ method: 'POST' }),
    );

    setActivePinia(createPinia());
    vi.mocked(request).mockClear().mockResolvedValueOnce(list).mockResolvedValueOnce([]);
    const apps = useAppsStore();
    const shell = useShellStore();
    await apps.fetchInstalled();
    await nextTick();

    expect(shell.openApps).toEqual([]);
    expect(shell.activeId).toBeNull();
    expect(request).not.toHaveBeenCalledWith('/apps/nas-disk-manager/start', expect.anything());
  });

  it('deduplicates opens, restores a neighbouring tab and keeps modal navigation separate', async () => {
    const { shell } = await ready();
    shell.open('nas-app-manager');
    shell.open('nas-disk-manager');
    shell.open('nas-app-manager');
    expect(shell.openApps.map((a) => a.id)).toEqual(['nas-app-manager', 'nas-disk-manager']);
    shell.open('nas-global-search', { query: 'photo' });
    expect(shell.activeId).toBe('nas-app-manager');
    expect(shell.modalId).toBe('nas-global-search');
    expect(shell.launchQuery).toBe('photo');
    expect(shell.openApp?.id).toBe('nas-global-search');
    shell.close();
    expect(shell.modalId).toBeNull();
    expect(shell.activeId).toBe('nas-app-manager');
    shell.close('nas-app-manager');
    expect(shell.activeId).toBe('nas-disk-manager');
    shell.showDesktop();
    expect(shell.openApps).toHaveLength(1);
    expect(shell.activeId).toBeNull();
    shell.open('unknown');
    expect(shell.activeId).toBeNull();
  });

  it('remembers the page an application was asked to open on, until it is closed', async () => {
    const { shell } = await ready();
    shell.open('nas-disk-manager', { location: '?disk=sda' });
    expect(shell.locations['nas-disk-manager']).toBe('?disk=sda');
    shell.open('nas-disk-manager');
    expect(shell.locations['nas-disk-manager']).toBe('?disk=sda');
    await shell.close('nas-disk-manager');
    expect(shell.locations['nas-disk-manager']).toBeUndefined();
  });

  it('starts external apps before use and stops their process before removing the tab', async () => {
    const { shell } = await ready();
    shell.open('nas-disk-manager');
    await vi.waitFor(() => expect(shell.starting).toEqual([]));
    expect(request).toHaveBeenCalledWith(
      '/apps/nas-disk-manager/start',
      expect.objectContaining({ method: 'POST' }),
    );

    await shell.close('nas-disk-manager');
    expect(request).toHaveBeenCalledWith(
      '/apps/nas-disk-manager/close',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(shell.openApps).toHaveLength(0);

    shell.open('nas-disk-manager');
    await vi.waitFor(() => expect(shell.starting).toEqual([]));
    expect(
      vi.mocked(request).mock.calls.filter(([path]) => path === '/apps/nas-disk-manager/start'),
    ).toHaveLength(2);
  });

  it('asks the launcher, never the page of the application, whether it runs', async () => {
    const { shell } = await ready();
    shell.open('nas-disk-manager');
    await vi.waitFor(() => expect(shell.starting).toEqual([]));
    expect(request).toHaveBeenCalledWith(
      '/apps/nas-disk-manager/start',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(fetch).not.toHaveBeenCalled();
    expect(shell.openApps.map((app) => app.id)).toEqual(['nas-disk-manager']);
  });

  it('keeps the tab open when stopping fails and allows a retry', async () => {
    const { shell } = await ready();
    shell.open('nas-disk-manager');
    await vi.waitFor(() => expect(shell.starting).toEqual([]));
    vi.mocked(request).mockRejectedValueOnce(new Error('systemctl failed'));
    await shell.close('nas-disk-manager');
    expect(shell.openApps.map((app) => app.id)).toEqual(['nas-disk-manager']);
    expect(shell.closeError).toBe('systemctl failed');
    await shell.close('nas-disk-manager');
    expect(shell.openApps).toHaveLength(0);
    expect(shell.closeError).toBe('');
  });

  it('waits for a pending start before stopping an app closed immediately', async () => {
    const { shell } = await ready();
    let finishStart!: () => void;
    vi.mocked(request).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishStart = () => resolve(undefined as never);
        }),
    );
    shell.open('nas-disk-manager');
    const closed = shell.close('nas-disk-manager');
    expect(
      vi.mocked(request).mock.calls.filter(([path]) => path === '/apps/nas-disk-manager/close'),
    ).toHaveLength(0);
    await vi.waitFor(() => expect(finishStart).toBeTypeOf('function'));
    finishStart();
    await closed;
    expect(
      vi.mocked(request).mock.calls.filter(([path]) => path === '/apps/nas-disk-manager/close'),
    ).toHaveLength(1);
    expect(shell.openApps).toHaveLength(0);
  });

  it('restarts an app reopened while its previous process is still stopping', async () => {
    const { shell } = await ready();
    shell.open('nas-disk-manager');
    await vi.waitFor(() => expect(shell.starting).toEqual([]));

    let finishStop!: () => void;
    vi.mocked(request).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishStop = () => resolve(undefined as never);
        }),
    );
    const closed = shell.close('nas-disk-manager');
    await vi.waitFor(() => expect(finishStop).toBeTypeOf('function'));
    shell.open('nas-disk-manager');
    expect(
      vi.mocked(request).mock.calls.filter(([path]) => path === '/apps/nas-disk-manager/start'),
    ).toHaveLength(1);

    finishStop();
    await closed;
    await vi.waitFor(() =>
      expect(
        vi.mocked(request).mock.calls.filter(([path]) => path === '/apps/nas-disk-manager/start'),
      ).toHaveLength(2),
    );
    expect(shell.openApps.map((app) => app.id)).toEqual(['nas-disk-manager']);
  });

  it('closes the windows of an application that is no longer installed', async () => {
    const { apps, shell } = await ready();
    shell.open('nas-app-manager');
    shell.open('nas-disk-manager');
    vi.mocked(request)
      .mockResolvedValueOnce(list.filter((app) => app.id !== 'nas-disk-manager'))
      .mockResolvedValueOnce([]);
    await apps.fetchInstalled();
    await nextTick();
    expect(shell.openApps.map((app) => app.id)).toEqual(['nas-app-manager']);
    expect(shell.activeId).toBe('nas-app-manager');
  });

  it('ends a session on the desktop and lets the launcher stop what the windows started', async () => {
    const first = await ready();
    first.shell.open('nas-app-manager');
    first.shell.open('nas-disk-manager', { location: '?disk=sda' });
    await vi.waitFor(() => expect(first.shell.starting).toEqual([]));

    await first.shell.endSession();
    await nextTick();

    expect(first.shell.openApps).toEqual([]);
    expect(first.shell.activeId).toBeNull();
    expect(first.shell.locations).toEqual({});
    expect(request).toHaveBeenLastCalledWith(
      '/apps/close',
      expect.objectContaining({ method: 'POST' }),
    );

    setActivePinia(createPinia());
    const next = await ready();
    await nextTick();
    expect(next.shell.openApps).toEqual([]);
  });

  it('ends a session even when the launcher does not answer', async () => {
    const { shell } = await ready();
    shell.open('nas-app-manager');
    vi.mocked(request).mockRejectedValueOnce(new Error('offline'));
    await expect(shell.endSession()).resolves.toBeUndefined();
    expect(shell.openApps).toEqual([]);
  });
});
