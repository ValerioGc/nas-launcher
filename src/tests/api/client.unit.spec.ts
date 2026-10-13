import { afterEach, describe, expect, it, vi } from 'vitest';
import { request } from '@/api/client';

afterEach(() => vi.unstubAllGlobals());

describe('API client', () => {
  it('sends JSON and announces successful mutations only', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue({ ok: true, status: 200, json: async () => ({ accepted: true }) });
    vi.stubGlobal('fetch', fetch);
    const listener = vi.fn();
    window.addEventListener('nas:mutation', listener);

    await expect(request('/tasks/1/stop', { method: 'POST', body: '{}' })).resolves.toEqual({
      accepted: true,
    });

    expect(fetch).toHaveBeenCalledWith(
      '/api/tasks/1/stop',
      expect.objectContaining({
        method: 'POST',
        body: '{}',
        headers: { 'Content-Type': 'application/json' },
      }),
    );

    expect(listener).toHaveBeenCalledOnce();
    fetch.mockResolvedValue({ ok: false, status: 400 });
    await expect(request('/tasks/1/stop', { method: 'POST' })).rejects.toThrow();
    expect(listener).toHaveBeenCalledOnce();
    window.removeEventListener('nas:mutation', listener);
  });

  it('handles a no-content deletion without parsing JSON', async () => {
    const json = vi.fn();
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: true, status: 204, json }));
    await expect(request('/notifications/1', { method: 'DELETE' })).resolves.toBeUndefined();
    expect(json).not.toHaveBeenCalled();
  });
});

it('returns to login when an API request loses its session', async () => {
  const assign = vi.fn();
  vi.stubGlobal('location', { pathname: '/console', assign });
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue({
      ok: false,
      status: 401,
      json: async () => ({ error: 'unauthorized' }),
    }),
  );

  await expect(request('/tasks')).rejects.toThrow('unauthorized');
  expect(assign).toHaveBeenCalledWith('/login');
});
