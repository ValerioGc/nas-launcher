import { defineStore } from 'pinia';
import { ref } from 'vue';
import { request } from '@/api/client';
import { setLocale, t } from '@/i18n';
import type { GeneralSettings } from './types/general';

export const useSettingsStore = defineStore('settings', () => {
  const general = ref<GeneralSettings | null>(null);
  const error = ref('');
  const saving = ref(false);

  /**
   * Load general settings from the server.
   */
  async function load() {
    error.value = '';

    try {
      general.value = await request<GeneralSettings>('/settings/general');
      setLocale(general.value.language);
    } catch {
      error.value = t('requestFailed');
    }
  }

  /**
   * Save general settings
   * @param value - The general settings to be saved.
   * @returns A boolean indicating whether the save operation was successful.
   */
  async function save(value: GeneralSettings) {
    if (saving.value) {
      return false;
    }

    saving.value = true;
    error.value = '';

    try {
      general.value = await request<GeneralSettings>('/settings/general', {
        method: 'PUT',
        body: JSON.stringify(value),
      });

      setLocale(general.value.language);
      return true;
    } catch {
      error.value = t('requestFailed');
      return false;
    } finally {
      saving.value = false;
    }
  }

  return { general, error, saving, load, save };
});
