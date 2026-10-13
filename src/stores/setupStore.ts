import { defineStore } from 'pinia';
import { ref } from 'vue';
import { request } from '@/api/client';
import type { SetupPhase, SetupState } from '@/types/types';

/**
 * Store for the first configuration of the NAS. The wizard is a project of its own, which
 * configures the machine before the launcher serves anybody: what is left here is the wait for the
 * steps the launcher runs after it, which opens instead of the sign-in until they are done.
 */
export const useSetupStore = defineStore('setup', () => {
  const state = ref<SetupState | null>(null);

  /**
   * Reads the state of the first configuration from the launcher.
   * @returns What the launcher answered; a launcher that does not answer rejects.
   */
  async function load() {
    state.value = await request<SetupState>('/setup');
    return state.value;
  }

  /**
   * Where the first configuration is. Once nothing is left the answer does not change and is not
   * asked again. A launcher that does not answer does not trap anybody on the waiting page: the
   * sign-in page is where an unreachable NAS is reported.
   * @returns The page the configuration needs, or `ready` when it needs none.
   */
  async function check(): Promise<SetupPhase> {
    if (state.value?.ready) {
      return 'ready';
    }
    try {
      const current = await load();
      if (!current.completed) {
        return 'unconfigured';
      }
      return current.ready ? 'ready' : 'preparing';
    } catch {
      return 'ready';
    }
  }

  return { state, load, check };
});
