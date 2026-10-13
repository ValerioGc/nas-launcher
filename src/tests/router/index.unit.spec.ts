import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { router } from '@/router';
import { hasSession } from '@/api/gateway';
import { request } from '@/api/client';
import { pinia } from '@/stores/pinia';
import { useSetupStore } from '@/stores/setupStore';

vi.mock('@/api/gateway', () => ({ hasSession: vi.fn() }));
vi.mock('@/api/client', () => ({ request: vi.fn() }));

// Test navigation and guards without loading the views and their component trees.
vi.mock('@/views/LoginPage.vue', () => ({ default: { render: () => null } }));
vi.mock('@/views/MainPage.vue', () => ({ default: { render: () => null } }));
vi.mock('@/views/PreparingView.vue', () => ({ default: { render: () => null } }));

const READY = { completed: true, ready: true, steps: [] };

beforeEach(async () => {
  setActivePinia(createPinia());
  vi.mocked(hasSession).mockReset().mockResolvedValue(false);
  vi.mocked(request).mockReset().mockResolvedValue(READY);
  useSetupStore(pinia).state = null;
  await router.push('/login');
});

describe('router', () => {
  it('declares the main and login routes', () => {
    expect(router.getRoutes().map((route) => route.name)).toEqual(
      expect.arrayContaining(['main', 'login']),
    );
  });

  it('redirects unauthenticated navigation to login', async () => {
    await router.push('/console');
    expect(router.currentRoute.value.name).toBe('login');
    expect(hasSession).toHaveBeenCalledOnce();
  });

  it('allows console navigation with a server session', async () => {
    vi.mocked(hasSession).mockResolvedValue(true);
    await router.push('/console');
    expect(router.currentRoute.value.name).toBe('main');
  });

  it.each(['/', '/unknown', '/console/deep'])('sends %s to login', async (path) => {
    await router.push(path);
    expect(router.currentRoute.value.name).toBe('login');
  });

  it('waits on the preparation of a NAS the wizard has not configured', async () => {
    useSetupStore(pinia).state = null;
    vi.mocked(request).mockResolvedValue({ completed: false, ready: false, steps: [] });

    await router.push('/console');
    expect(router.currentRoute.value.name).toBe('preparing');
    expect(hasSession).not.toHaveBeenCalled();
  });

  it('never shows the preparation again once everything is done', async () => {
    await router.push('/preparing');
    expect(router.currentRoute.value.name).toBe('login');
  });

  it('waits on the preparation while the steps after the wizard run', async () => {
    useSetupStore(pinia).state = null;
    vi.mocked(request).mockResolvedValue({
      completed: true,
      ready: false,
      steps: [{ id: 'app:photos', kind: 'app', name: 'Photos', state: 'pending', detail: '' }],
    });

    await router.push('/console');
    expect(router.currentRoute.value.name).toBe('preparing');
    await router.push('/login');
    expect(router.currentRoute.value.name).toBe('preparing');
    expect(hasSession).not.toHaveBeenCalled();
  });

  it('goes on to the sign-in when the setup state cannot be read', async () => {
    vi.mocked(request).mockRejectedValue(new Error('down'));
    await router.push('/console');
    expect(router.currentRoute.value.name).toBe('login');
  });
});
