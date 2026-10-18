import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { request } from '@/api/client';
import { t } from '@/i18n';
import type { WidgetSettings } from '@/types/types';

/**
 * Store for handling widget settings and related actions.
 */
export const useWidgetsStore = defineStore('widgets', () => {
  const settings = ref<WidgetSettings | null>(null);
  const saving = ref(false);
  const error = ref('');
  const pollingMs = computed(() => (settings.value?.pollingSeconds ?? 10) * 1000);

  /**
   * Loads the current widget settings from the server.
   * @returns A promise that resolves when the settings have been loaded.
   */
  async function load() {
    try {
      settings.value = await request<WidgetSettings>('/settings/widgets');
      error.value = '';
    } catch {
      error.value = t('requestFailed');
    }
  }

  /**
   * Saves the provided widget settings to the server.
   * @param value The new widget settings to be saved.
   * @returns A promise that resolves to a boolean indicating whether the save operation was successful.
   */
  async function save(value: WidgetSettings) {
    saving.value = true;
    try {
      settings.value = await request<WidgetSettings>('/settings/widgets', {
        method: 'PUT',
        body: JSON.stringify(value),
      });
      error.value = '';
      return true;
    } catch {
      error.value = t('requestFailed');
      return false;
    } finally {
      saving.value = false;
    }
  }

  return { settings, saving, error, pollingMs, load, save };
});
