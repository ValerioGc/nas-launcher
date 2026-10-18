import { computed, ref } from 'vue';
import { t } from '@/i18n';
import { CodeRefused, requestPasswordReset, resetPassword } from '@/api/account';
import { LoginThrottled } from '@/api/gateway';
import { failureMessage } from '@/utils/failureMessage';

function failure(reason: unknown) {
  if (reason instanceof CodeRefused) {
    return t('passwordRecoveryRefused');
  }
  if (reason instanceof LoginThrottled) {
    return t('passwordRecoveryThrottled');
  }
  return failureMessage(reason);
}

/**
 * The password reset of the sign-in page. The username asks for a code at the verified recovery
 * address, then the code and a new password set it. The gateway answers the first step the same
 * whether the address exists or not, and so does the page.
 */
export function usePasswordRecovery() {
  const step = ref<'request' | 'reset' | 'done'>('request');
  const username = ref('');
  const code = ref('');
  const password = ref('');
  const repeated = ref('');
  const busy = ref(false);
  const error = ref('');

  const mismatch = computed(() => Boolean(repeated.value) && password.value !== repeated.value);
  const ready = computed(
    () =>
      /^\d{6}$/.test(code.value.trim()) &&
      password.value.length >= 8 &&
      password.value.length <= 128 &&
      password.value === repeated.value,
  );

  async function attempt(action: () => Promise<void>) {
    busy.value = true;
    error.value = '';
    try {
      await action();
    } catch (reason) {
      error.value = failure(reason);
    } finally {
      busy.value = false;
    }
  }

  function send() {
    return attempt(async () => {
      await requestPasswordReset(username.value.trim());
      step.value = 'reset';
    });
  }

  function reset() {
    return attempt(async () => {
      await resetPassword(username.value.trim(), code.value.trim(), password.value);
      step.value = 'done';
      code.value = '';
      password.value = '';
      repeated.value = '';
    });
  }

  function restart() {
    step.value = 'request';
    error.value = '';
    code.value = '';
    password.value = '';
    repeated.value = '';
  }

  return {
    step,
    username,
    code,
    password,
    repeated,
    busy,
    error,
    mismatch,
    ready,
    send,
    reset,
    restart,
  };
}
