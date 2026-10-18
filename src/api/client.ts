import { t } from '@/i18n';

/**
 * Sends an HTTP request to the specified API endpoint and returns the response data.
 * @param path - The API endpoint path.
 * @param options - The fetch options, such as method, headers, and body.
 * @returns A promise that resolves to the response data of type T.
 */
export async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const response = await fetch(`/api${path}`, {
    ...options,
    headers: { 'Content-Type': 'application/json', ...options.headers },
    signal: options.signal ?? AbortSignal.timeout(10000),
  });

  if (!response.ok) {
    if (response.status === 401 && window.location.pathname !== '/login') {
      window.location.assign('/login');
    }
    const body: unknown = await response.json().catch(() => null);
    const reason =
      typeof body === 'object' && body !== null && 'error' in body && typeof body.error === 'string'
        ? body.error
        : '';

    throw new Error(reason || t('requestFailed'));
  }

  const result = response.status === 204 ? undefined : await response.json();
  if (options.method && options.method !== 'GET') {
    window.dispatchEvent(new CustomEvent('nas:mutation', { detail: path }));
  }

  return result as T;
}
