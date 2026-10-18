import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { request } from '@/api/client';
import { NoSearchProviderError, searchFiles, searchProvider } from '@/apps/nas-global-search/api';
import type { SearchResult } from '@/apps/nas-global-search/types';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const found: SearchResult = {
  id: '7',
  name: 'Report',
  path: '/docs/report.pdf',
  type: 'document',
  extension: 'pdf',
  size: 10,
  modified: '',
  app_id: 'somebodyElse',
  target: '?path=%2Fdocs',
};

beforeEach(() => {
  vi.stubGlobal('fetch', vi.fn());
  vi.mocked(request).mockReset();
});
afterEach(() => vi.unstubAllGlobals());

it('asks a provider directly on its own path and opens the results in it', async () => {
  vi.mocked(fetch).mockResolvedValue(new Response(JSON.stringify([found])));
  const results = await searchProvider(
    { app: 'nas-file-manager', endpoint: '/api/search', version: 1 },
    new URLSearchParams('q=report&type=document'),
  );

  expect(fetch).toHaveBeenCalledExactlyOnceWith(
    `${window.location.origin}/apps/nas-file-manager/api/search?q=report&type=document`,
    expect.objectContaining({ signal: expect.any(AbortSignal) }),
  );
  expect(results).toEqual([{ ...found, id: 'nas-file-manager:7', app_id: 'nas-file-manager' }]);
});

it('makes no search when no application provides one', async () => {
  vi.mocked(request).mockResolvedValue([]);
  await expect(searchFiles(new URLSearchParams('q=report'))).rejects.toBeInstanceOf(
    NoSearchProviderError,
  );
  expect(request).toHaveBeenCalledWith('/capabilities/search');
  expect(fetch).not.toHaveBeenCalled();
});

it('keeps the answers that came and names the providers that failed', async () => {
  vi.mocked(request).mockResolvedValue([
    { app: 'nas-file-manager', endpoint: '/api/search', version: 1 },
    { app: 'photos', endpoint: '/search', version: 1 },
  ]);
  vi.mocked(fetch).mockImplementation((url) =>
    Promise.resolve(
      String(url).includes('/apps/photos/')
        ? new Response('', { status: 503 })
        : new Response(JSON.stringify([found])),
    ),
  );

  const search = await searchFiles(new URLSearchParams('q=report'));
  expect(search.results.map((result) => result.id)).toEqual(['nas-file-manager:7']);
  expect(search.failed).toEqual(['photos']);
});

it('fails when no provider answers', async () => {
  vi.mocked(request).mockResolvedValue([{ app: 'photos', endpoint: '/search', version: 1 }]);
  vi.mocked(fetch).mockResolvedValue(new Response('', { status: 503 }));
  await expect(searchFiles(new URLSearchParams('q=report'))).rejects.toThrow();
});
