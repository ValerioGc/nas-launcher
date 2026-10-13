import { onBeforeUnmount, onMounted, ref, type Ref } from 'vue';
import { request } from '@/api/client';
import { t } from '@/i18n';
import { failureMessage } from '@/utils/failureMessage';

/**
 * Composable for managing a resource with HTTP requests, including loading, error, and busy states.
 * @template T - The type of the resource data.
 */
export function useResource<T>(path: string | (() => string), initial: T) {
  const data = ref(initial) as Ref<T>;
  const loading = ref(false);
  const busy = ref(false);
  const error = ref('');
  let version = 0;

  /**
   * Loads the resource data from the server and updates the reactive state accordingly.
   */
  async function load() {
    const current = ++version;
    loading.value = true;
    error.value = '';
    try {
      const result = await request<T>(typeof path === 'string' ? path : path());
      if (current === version) {
        data.value = result;
      }
    } catch {
      if (current === version) {
        error.value = t('requestFailed');
      }
    } finally {
      if (current === version) {
        loading.value = false;
      }
    }
  }

  /**
   * Performs an action on the resource by sending an HTTP request to the specified endpoint.
   * @param endpoint The API endpoint to send the request to.
   * @param method The HTTP method to use for the request (default is 'POST').
   * @param body The request body, if any.
   * @param timeoutMs How long to wait. Joining a wireless network takes more than the ten seconds
   * that are enough for everything else.
   * @returns A promise that resolves to true if the action was successful, or false if it failed.
   */
  async function act(endpoint: string, method = 'POST', body?: unknown, timeoutMs?: number) {
    if (busy.value) {
      return false;
    }

    busy.value = true;
    error.value = '';

    try {
      await request(endpoint, {
        method,
        body: body === undefined ? undefined : JSON.stringify(body),
        ...(timeoutMs === undefined ? {} : { signal: AbortSignal.timeout(timeoutMs) }),
      });

      await load();
      return true;
    } catch (error_) {
      error.value = failureMessage(error_);
      return false;
    } finally {
      busy.value = false;
    }
  }

  onMounted(load);

  onBeforeUnmount(() => {
    version++;
  });

  return { data, loading, busy, error, load, act };
}
