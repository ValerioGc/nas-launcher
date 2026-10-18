import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { request } from '@/api/client';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import { useNotificationsStore, TOAST_MS } from '@/apps/nas-notification-manager/store';
import { setLocale } from '@/i18n';
import { answerManager, credentials, deferred, installed, offer, system } from './fixtures';
import type { ManagedApp } from '@/apps/nas-app-manager/types';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  answerManager();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('app manager store', () => {
  it('filters external system apps while retaining selection and updates', async () => {
    const store = useAppManagerStore();
    await store.initialize();
    store.data = [{ ...installed, system: true }, system];
    store.setShowSystem(false);
    expect(store.listed).toHaveLength(0);
    store.setShowSystem(true);
    store.select(installed.id);
    expect(store.selected?.id).toBe(installed.id);
    store.forge = { ...credentials, configured: true };
    store.checked = { [offer.id]: { ...offer, system: true } };
    expect(await store.updateApps()).toBe(true);
    expect(request).toHaveBeenCalledWith(
      `/apps/${installed.id}/update`,
      expect.objectContaining({ method: 'POST' }),
    );
    store.select(system.id);
    expect(store.selected).toBeNull();
    await store.setEndpoint('available');
    store.data = [{ ...offer, system: true }];
    store.setShowSystem(false);
    expect(store.listed.map((app) => app.id)).toEqual([offer.id]);
    store.select(offer.id);
    expect(store.selected?.id).toBe(offer.id);
  });
  it('refreshes update notifications through the existing toast flow without duplicates', async () => {
    const notifications = useNotificationsStore();
    await notifications.refresh(false);
    const answer = vi.mocked(request).getMockImplementation();
    vi.mocked(request).mockImplementation(async (path, options) => {
      if (path.startsWith('/notifications?')) {
        return [
          {
            id: 'update-1',
            source: 'nas-app-manager',
            title: 'Aggiornamenti disponibili',
            message: '1 applicazione',
            tone: 'info',
            at: 1,
            read: false,
          },
        ] as never;
      }
      return answer?.(path, options) as never;
    });
    const store = useAppManagerStore();
    await store.initialize();
    await store.check();
    expect(notifications.toasts).toEqual([
      expect.objectContaining({ id: 'update-1', duration: TOAST_MS.info }),
    ]);
    await store.check();
    expect(notifications.toasts).toHaveLength(1);
  });
  it('loads only catalogue and capabilities on opening and protects system apps', async () => {
    const store = useAppManagerStore();
    await store.initialize();
    expect(vi.mocked(request).mock.calls.map(([path]) => path)).toEqual([
      '/apps/installed',
      '/apps/credentials',
      '/tasks',
      '/settings/apps',
    ]);
    store.select(system.id);
    expect(store.selected).toBeNull();
    expect(await store.change('uninstall')).toBe(false);
    expect(request).not.toHaveBeenCalledWith(`/apps/${system.id}/uninstall`, expect.anything());
  });

  it('discards a previous tab response even when it arrives last', async () => {
    const store = useAppManagerStore();
    const old = deferred<ManagedApp[]>();
    vi.mocked(request).mockImplementationOnce(() => old.promise);
    const first = store.load();
    await store.setEndpoint('available');
    store.select(offer.id);
    old.resolve([system]);
    await first;
    expect(store.endpoint).toBe('available');
    expect(store.rows.map((row) => row.id)).toEqual([offer.id]);
    expect(store.selected?.id).toBe(offer.id);
    expect(store.loading).toBe(false);
  });

  it('refreshes a selection after checking updates and clears it when the app disappears', async () => {
    const store = useAppManagerStore();
    await store.initialize();
    store.select(installed.id);
    await store.check();
    expect(store.selected).toMatchObject({ latestVersion: '2.0.0', updateAvailable: true });
    expect(store.updates).toBe(1);
    vi.mocked(request).mockResolvedValueOnce([]);
    await store.load();
    expect(store.selected).toBeNull();
  });

  it('clears a completed update after the installed version changes without another check', async () => {
    const store = useAppManagerStore();
    await store.initialize();
    await store.check();
    store.select(installed.id);
    expect(store.updates).toBe(1);

    vi.useFakeTimers();
    expect(await store.change('install')).toBe(true);
    vi.mocked(request).mockImplementation(async (path) => {
      if (path === '/tasks') {
        return [
          {
            id: `install-${installed.id}`,
            source: 'nas-app-manager',
            name: installed.name,
            state: 'done',
          },
        ] as never;
      }
      if (path === '/apps/installed') {
        return [{ ...installed, version: '2.0.0' }, system] as never;
      }
      return [] as never;
    });

    await vi.advanceTimersByTimeAsync(2000);

    expect(store.installing).toEqual([]);
    expect(store.updates).toBe(0);
    expect(store.rows.find((row) => row.id === installed.id)).toMatchObject({
      version: '2.0.0',
      updateAvailable: false,
    });
  });

  it('updates every available application sequentially', async () => {
    const second = { ...installed, id: 'second', name: 'Second' };
    const secondOffer = { ...offer, id: second.id, name: second.name };
    const events: string[] = [];
    const store = useAppManagerStore();

    vi.mocked(request).mockImplementation(async (path: string) => {
      if (path === '/apps/credentials') {
        return { ...credentials, configured: true } as never;
      }
      if (path === '/apps/installed') {
        return [installed, second] as never;
      }
      if (path === '/apps/updates') {
        return [offer, secondOffer] as never;
      }
      if (path.endsWith('/update')) {
        events.push(path);
        return {} as never;
      }
      if (path === '/tasks') {
        const active = events.at(-1)?.split('/')[2];
        if (active) {
          events.push(`done:${active}`);
        }
        return [] as never;
      }
      return [] as never;
    });

    await store.initialize();
    await store.check();
    events.length = 0;

    expect(await store.updateApps()).toBe(true);
    expect(events).toEqual([
      '/apps/example/update',
      'done:example',
      '/apps/second/update',
      'done:second',
    ]);
    expect(store.busy).toBe(false);
    expect(store.installing).toEqual([]);
  });

  it('checks on opening, and installs what it found only when the settings ask for both', async () => {
    const answer = vi.mocked(request).getMockImplementation();
    const opening = (checkOnOpen: boolean, updateOnOpen: boolean) =>
      vi.mocked(request).mockImplementation(async (path: string, options?: RequestInit) => {
        if (path === '/settings/apps') {
          return { autoCheck: false, frequency: 'daily', checkOnOpen, updateOnOpen } as never;
        }
        if (path === '/apps/credentials') {
          return { ...credentials, configured: true } as never;
        }
        return answer?.(path, options) as never;
      });
    const calls = () => vi.mocked(request).mock.calls.map(([path]) => path);

    opening(false, false);
    await useAppManagerStore().initialize();
    expect(calls()).not.toContain('/apps/updates');

    vi.mocked(request).mockClear();
    opening(true, false);
    const store = useAppManagerStore();
    await store.initialize();
    expect(calls()).toContain('/apps/updates');
    expect(calls()).not.toContain('/apps/example/update');
    expect(store.updates).toBe(1);

    vi.mocked(request).mockClear();
    opening(true, true);
    await store.initialize();
    expect(calls()).toContain('/apps/example/update');
  });

  it('prevents duplicate removal operations', async () => {
    const store = useAppManagerStore();
    await store.initialize();
    store.select(installed.id);
    const operation = deferred<unknown>();
    vi.mocked(request)
      .mockClear()
      .mockImplementationOnce(() => operation.promise)
      .mockResolvedValueOnce([installed, system]);
    const first = store.change('uninstall');
    expect(await store.change('uninstall')).toBe(false);
    await store.setEndpoint('available');
    expect(store.endpoint).toBe('installed');
    expect(request).toHaveBeenCalledTimes(1);
    operation.resolve({});
    await first;
    expect(
      vi.mocked(request).mock.calls.filter(([path]) => path === `/apps/${installed.id}/uninstall`),
    ).toHaveLength(1);
  });

  it('retains the selection and exposes an operation failure for retry', async () => {
    const store = useAppManagerStore();
    await store.initialize();
    store.select(installed.id);
    vi.mocked(request).mockRejectedValueOnce(new Error('Unable to remove'));
    expect(await store.change('uninstall')).toBe(false);
    expect(store.error).toBe('Unable to remove');
    expect(store.selected?.id).toBe(installed.id);
    expect(store.busy).toBe(false);
    expect(await store.change('uninstall', { removeData: true })).toBe(true);
    expect(request).toHaveBeenCalledWith(
      `/apps/${installed.id}/uninstall?removeData=true`,
      expect.objectContaining({ method: 'POST' }),
    );
    expect(store.selected).toBeNull();
    expect(store.error).toBe('');
  });

  it('does not check updates when packages are disabled and allows retry after failure', async () => {
    const store = useAppManagerStore();
    await store.initialize();
    vi.mocked(request).mockResolvedValueOnce({ ...credentials, packagesEnabled: false });
    await store.loadCredentials();
    vi.mocked(request).mockClear();
    await store.check();
    expect(request).not.toHaveBeenCalled();
    await store.loadCredentials();
    vi.mocked(request).mockRejectedValueOnce(new Error('Forge unavailable'));
    await store.check();
    expect(store.checkError).toBe('Forge unavailable');
    expect(store.checking).toBe(false);
    await store.check();
    expect(store.checkError).toBe('');
    expect(store.updates).toBe(1);
  });

  it('sends a new token write-only and reports why it was refused', async () => {
    const store = useAppManagerStore();
    vi.mocked(request).mockResolvedValueOnce({ accepted: true });
    expect(await store.saveToken('ghp_new')).toBe('');
    expect(request).toHaveBeenLastCalledWith('/apps/credentials', {
      method: 'PUT',
      body: JSON.stringify({ token: 'ghp_new' }),
    });

    vi.mocked(request).mockRejectedValueOnce(new Error('that does not look like a token'));
    expect(await store.saveToken('two words')).toBe('that does not look like a token');
  });

  it('draws an offer with the logo the launcher serves for it', async () => {
    const store = useAppManagerStore();
    vi.mocked(request).mockImplementation((path: string) =>
      Promise.resolve(
        (path === '/apps/available'
          ? [{ ...offer, id: 'photos', logo: '/api/apps/photos/logo?v=1' }]
          : credentials) as never,
      ),
    );
    await store.initialize();
    await store.setEndpoint('available');

    expect(store.rows.find((row) => row.id === 'photos')?.icon).toBe('/api/apps/photos/logo?v=1');
  });

  it('leaves itself out of the applications it lists', async () => {
    const store = useAppManagerStore();
    vi.mocked(request).mockImplementation((path: string) =>
      Promise.resolve(
        (path === '/apps/installed'
          ? [{ ...system, id: 'nas-app-manager', name: 'Application manager' }, system, installed]
          : credentials) as never,
      ),
    );

    await store.initialize();

    expect(store.rows.map((row) => row.id)).toEqual([system.id, 'example']);
  });

  it('follows an installation until it ends and says why it failed', async () => {
    vi.useFakeTimers();
    const store = useAppManagerStore();
    await store.initialize();
    await store.setEndpoint('available');
    store.select(offer.id);
    vi.mocked(request).mockResolvedValueOnce({ task: `install-${offer.id}` });
    expect(await store.change('install')).toBe(true);
    expect(store.installing).toEqual([offer.id]);

    const task = {
      id: `install-${offer.id}`,
      source: 'nas-app-manager',
      name: 'Example',
      detail: 'unpacking',
      state: 'running',
    };
    vi.mocked(request).mockClear();
    vi.mocked(request).mockResolvedValueOnce([task, { ...task, id: 'other', source: 'x' }]);
    await vi.advanceTimersByTimeAsync(2000);
    expect(store.installing).toEqual([offer.id]);
    expect(request).toHaveBeenCalledTimes(1);

    vi.mocked(request).mockResolvedValueOnce([
      { ...task, state: 'failed', detail: 'the release has no asset' },
    ]);
    await vi.advanceTimersByTimeAsync(2000);
    expect(store.installing).toEqual([]);
    expect(store.error).toBe('Installazione di Example non riuscita (the release has no asset).');
    expect(vi.mocked(request).mock.calls.map(([path]) => path)).toEqual(
      expect.arrayContaining(['/apps/available', '/apps']),
    );

    vi.mocked(request).mockClear();
    await vi.advanceTimersByTimeAsync(10000);
    expect(request).not.toHaveBeenCalled();
  });

  it('picks up an installation already running when it opens', async () => {
    vi.mocked(request).mockImplementation(
      async (path: string) =>
        (path === '/tasks'
          ? [{ id: 'install-photos', source: 'nas-app-manager', name: 'Photos', state: 'running' }]
          : []) as never,
    );
    vi.useFakeTimers();
    const store = useAppManagerStore();
    await store.initialize();
    expect(store.installing).toEqual(['photos']);
  });
});
