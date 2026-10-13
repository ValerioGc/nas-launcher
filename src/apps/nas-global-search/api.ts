import { t } from '@/i18n';
import { request } from '@/api/client';
import { externalUrl } from '@/apps/registry';
import type { FileSearch, SearchProvider, SearchResult } from '@/apps/nas-global-search/types';

const PROVIDER_TIMEOUT_MS = 10000;

export class NoSearchProviderError extends Error {
  constructor() {
    super(t('searchNoProvider'));
  }
}

/**
 * Asks the launcher which applications answer the `search` capability.
 */
export function searchProviders(): Promise<SearchProvider[]> {
  return request<SearchProvider[]>('/capabilities/search');
}

/**
 * Searches one provider directly, on its own path. The results open in that provider, whatever
 * application they name.
 * @param provider - The application to ask.
 * @param params - The search parameters.
 */
export async function searchProvider(
  provider: SearchProvider,
  params: URLSearchParams,
): Promise<SearchResult[]> {
  const base = externalUrl(provider.app).replace(/\/$/, '');
  const url = new URL(`${base}${provider.endpoint}`, window.location.origin);
  for (const [key, value] of params) {
    url.searchParams.set(key, value);
  }

  const options: RequestInit = {
    headers: { Accept: 'application/json' },
    signal: AbortSignal.timeout(PROVIDER_TIMEOUT_MS),
  };
  let response = await fetch(url.toString(), options).catch(() => null);
  if (!response || response.status === 502 || response.status === 503) {
    await request(`/apps/${encodeURIComponent(provider.app)}/start`, {
      method: 'POST',
      signal: AbortSignal.timeout(35000),
    });
    response = await fetch(url.toString(), options);
  }
  if (!response.ok) {
    throw new Error(t('searchFailed'));
  }
  const results = (await response.json()) as SearchResult[];
  return results.map((result) => ({
    ...result,
    id: `${provider.app}:${result.id}`,
    app_id: provider.app,
  }));
}

/**
 * Searches the files through every provider at once. A provider that fails or does not answer in
 * time is named in `failed`, and the others still count.
 * @param params - The search parameters.
 */
export async function searchFiles(params: URLSearchParams): Promise<FileSearch> {
  const providers = await searchProviders();
  if (!providers.length) {
    throw new NoSearchProviderError();
  }

  const answers = await Promise.allSettled(
    providers.map((provider) => searchProvider(provider, params)),
  );
  const failed = providers
    .filter((_, index) => answers[index]?.status === 'rejected')
    .map((provider) => provider.app);
  if (failed.length === providers.length) {
    throw new Error(t('searchFailed'));
  }

  return {
    results: answers.flatMap((answer) => (answer.status === 'fulfilled' ? answer.value : [])),
    failed,
  };
}
