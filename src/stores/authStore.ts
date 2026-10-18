import { defineStore } from 'pinia';
import { ref } from 'vue';
import { request } from '@/api/client';
import { validateLogin } from '@/utils/loginValidation';
import { hasSession, login as gatewayLogin, LoginRefused, LoginThrottled } from '@/api/gateway';
import { t } from '@/i18n';
import type { LoginResult } from '@/types/types';

type FailedLoginField = 'username' | 'password';

function reportFailure(field: FailedLoginField) {
  request('/access/failures', { method: 'POST', body: JSON.stringify({ field }) }).catch(() => {});
}

/**
 * Store for handling authentication state and actions.
 */
export const useAuthStore = defineStore('auth', () => {
  const isLoading = ref(false);
  const authenticated = ref(false);

  /**
   * Attempts to log in a user with the provided username and password.
   * @param username - The username of the user attempting to log in.
   * @param password - The password of the user attempting to log in.
   * @returns A promise that resolves to a LoginResult indicating the success or failure of the login attempt.
   */
  async function login(username: string, password: string): Promise<LoginResult> {
    if (isLoading.value) {
      return { success: false, code: 'busy', message: t('signInBusy') };
    }

    const errors = validateLogin(username, password);
    if (errors.username || errors.password) {
      reportFailure(errors.username ? 'username' : 'password');
      return { success: false, code: 'invalid_input', message: errors.username || errors.password };
    }

    isLoading.value = true;
    try {
      const response = await gatewayLogin(username.trim(), password);
      if (!(await hasSession())) {
        throw new Error('gateway session missing');
      }
      authenticated.value = true;
      return { success: true, username: response.username };
    } catch (error) {
      authenticated.value = false;
      if (error instanceof LoginRefused) {
        reportFailure('password');
        return { success: false, code: 'refused', message: t('signInRefused') };
      }
      if (error instanceof LoginThrottled) {
        return { success: false, code: 'throttled', message: t('signInThrottled') };
      }
      return { success: false, code: 'unavailable', message: t('signInFailed') };
    } finally {
      isLoading.value = false;
    }
  }

  async function checkSession() {
    authenticated.value = await hasSession();
    return authenticated.value;
  }

  return { isLoading, authenticated, login, checkSession };
});
