import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppsStore } from '@/stores/appsStore';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const list = ['nas-app-manager', 'nas-disk-manager', 'nas-global-search'].map((id) => ({
  id,
  name: id,
  info: '',
  system: id !== 'nas-disk-manager',
}));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset();
});

async function ready() {
  vi.mocked(request).mockResolvedValueOnce(list).mockResolvedValueOnce([]);
  const store = useAppsStore();
  await store.fetchInstalled();
  return store;
}

describe('installed applications and shortcuts', () => {
  it('reads the registry and the desktop in one go', async () => {
    const store = await ready();
    expect(store.apps.map((app) => app.id)).toEqual([
      'nas-app-manager',
      'nas-disk-manager',
      'nas-global-search',
    ]);
    expect(store.loading).toBe(false);
    expect(store.errorMessage).toBe('');
    expect(request).toHaveBeenCalledWith('/apps');
    expect(request).toHaveBeenCalledWith('/desktop/shortcuts');
  });

  it('uses each system app label from the active locale instead of the backend name', async () => {
    const store = await ready();
    expect(store.apps.map((app) => app.name)).toEqual([
      'Gestore applicazioni',
      'nas-disk-manager',
      'Ricerca globale',
    ]);

    setLocale('en');
    expect(store.apps.map((app) => app.name)).toEqual([
      'Application manager',
      'nas-disk-manager',
      'Global search',
    ]);
  });

  it('reports a registry that cannot be read instead of emptying the desktop', async () => {
    const store = await ready();
    vi.mocked(request).mockRejectedValue(new Error('offline'));
    await store.fetchInstalled();
    expect(store.errorMessage).not.toBe('');
    expect(store.loading).toBe(false);
  });

  it('takes shortcuts from the backend and preserves them on failure', async () => {
    const store = await ready();
    vi.mocked(request).mockResolvedValueOnce([
      { kind: 'app', target: 'nas-disk-manager', name: null, position: 0 },
    ]);

    await store.shortcut('nas-disk-manager');
    expect(request).toHaveBeenLastCalledWith('/desktop/shortcuts/nas-disk-manager', {
      method: 'POST',
    });
    expect(store.shortcuts[0]?.id).toBe('nas-disk-manager');

    vi.mocked(request).mockResolvedValueOnce([
      { kind: 'app', target: 'nas-disk-manager', name: null, position: 0 },
      { kind: 'folder', target: '/srv/storage/foto', name: 'Foto', position: 1 },
    ]);

    await store.shortcut('nas-disk-manager');
    expect(store.shortcuts).toHaveLength(1);
    vi.mocked(request).mockRejectedValueOnce(new Error());
    await store.shortcut('nas-disk-manager', true);
    expect(store.shortcuts).toHaveLength(1);
    expect(store.errorMessage).not.toBe('');
  });

  it('shows the new order at once and goes back to the old one when the launcher refuses it', async () => {
    const shortcuts = ['nas-app-manager', 'nas-disk-manager', 'nas-global-search'].map(
      (target, position) => ({
        kind: 'app',
        target,
        name: null,
        position,
      }),
    );
    vi.mocked(request).mockResolvedValueOnce(list).mockResolvedValueOnce(shortcuts);
    const store = useAppsStore();
    await store.fetchInstalled();

    let answer: (value: unknown) => void = () => {};
    vi.mocked(request).mockReturnValueOnce(new Promise((resolve) => (answer = resolve)));
    const saving = store.reorder(['nas-global-search', 'nas-app-manager', 'nas-disk-manager']);
    expect(store.shortcuts.map((app) => app.id)).toEqual([
      'nas-global-search',
      'nas-app-manager',
      'nas-disk-manager',
    ]);
    expect(request).toHaveBeenLastCalledWith('/desktop/shortcuts/order', {
      method: 'PUT',
      body: JSON.stringify({ order: ['nas-global-search', 'nas-app-manager', 'nas-disk-manager'] }),
    });
    answer([shortcuts[2], shortcuts[0], shortcuts[1]]);
    await saving;

    vi.mocked(request).mockRejectedValueOnce(new Error());
    await store.reorder(['nas-disk-manager', 'nas-global-search', 'nas-app-manager']);
    expect(store.shortcuts.map((app) => app.id)).toEqual([
      'nas-global-search',
      'nas-app-manager',
      'nas-disk-manager',
    ]);
    expect(store.errorMessage).not.toBe('');
  });
});
