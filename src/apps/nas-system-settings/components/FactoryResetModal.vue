<script setup lang="ts">

import { computed, ref } from 'vue';
import { t } from '@/i18n';
import { request } from '@/api/client';
import { CONFIRMATION_HEADER, confirmPassword } from '@/api/gateway';
import { usePolling } from '@/composables/usePolling';
import type { ResetState } from '../types/reset';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonModal from '@/components/commons/CommonModal.vue';
import CommonPasswordInput from '@/components/commons/CommonPasswordInput.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import { passwordFailure } from '../passwordFailure';

const open = defineModel<boolean>({ required: true });

const POLL_MS = 2000;
const REMOVED = [
  'factoryResetAccount',
  'factoryResetApps',
  'factoryResetTerminal',
  'factoryResetSettings',
] as const;

type Phase = 'confirm' | 'sending' | 'resetting';

const phase = ref<Phase>('confirm');
const password = ref('');
const failure = ref('');

const shown = computed({
  get: () => open.value,
  set: (value: boolean) => {
    if (phase.value === 'confirm') {
      open.value = value;
    }
  },
});

async function reset() {
  failure.value = '';
  phase.value = 'sending';
  try {
    const code = await confirmPassword(password.value);
    await request<ResetState>('/system/reset', {
      method: 'POST',
      body: '{}',
      headers: { [CONFIRMATION_HEADER]: code },
    });
    phase.value = 'resetting';
  } catch (error) {
    failure.value = passwordFailure(error);
    phase.value = 'confirm';
  } finally {
    password.value = '';
  }
}

/**
 * The launcher and the gateway stop along the way, then the wizard takes the same address: the
 * page goes to it as soon as it answers. A plain fetch, since nothing here is signed in any more.
 */
async function waitForWizard() {
  try {
    const answer = await fetch('/api/wizard', { signal: AbortSignal.timeout(POLL_MS) });
    if (answer.ok) {
      globalThis.location.assign('/');
    }
  } catch {
    // Nobody answers yet.
  }
}

usePolling(
  () => {
    if (phase.value === 'resetting') {
      waitForWizard();
    }
  },
  computed(() => POLL_MS),
);

</script>

<template>
  <CommonModal
    v-model="shown"
    :title="t('factoryResetTitle')"
    :hide-close-button="phase !== 'confirm'"
  >
    <div v-if="phase === 'resetting'" class="factory_reset_waiting" aria-live="polite">
      <CommonSpinner size="small" compact />
      <p data-testid="factory_reset_running">{{ t('factoryResetRunning') }}</p>
    </div>

    <form
      v-else
      id="factory_reset_form"
      class="app_stack"
      data-testid="factory_reset_form"
      @submit.prevent="reset"
    >
      <p class="muted">{{ t('factoryResetIntro') }}</p>
      <AlertBanner tone="warning" :title="t('factoryResetRemoves')">
        <ul class="factory_reset_list">
          <li v-for="key in REMOVED" :key="key">{{ t(key) }}</li>
        </ul>
      </AlertBanner>
      <AlertBanner tone="info" :title="t('factoryResetKeepsTitle')">
        {{ t('factoryResetKeeps') }}
      </AlertBanner>

      <div class="factory_reset_field">
        <label for="factory_reset_password">{{ t('accountPassword') }}</label>
        <CommonPasswordInput
          id="factory_reset_password"
          v-model="password"
          username=""
          required
          maxlength="128"
          aria-describedby="factory_reset_password_note"
          :disabled="phase === 'sending'"
          data-testid="factory_reset_password"
        />
        <small id="factory_reset_password_note" class="muted">
          {{ t('factoryResetPasswordNote') }}
        </small>
      </div>

      <AlertBanner v-if="failure" tone="error" compact data-testid="factory_reset_failed">
        {{ failure }}
      </AlertBanner>
    </form>

    <template v-if="phase !== 'resetting'" #footer>
      <button type="button" :disabled="phase === 'sending'" @click="open = false">
        {{ t('cancel') }}
      </button>
      <button
        type="submit"
        form="factory_reset_form"
        class="danger"
        :disabled="!password || phase === 'sending'"
        data-testid="factory_reset_confirm"
      >
        <CommonSpinner v-if="phase === 'sending'" size="small" compact />
        {{ t('factoryResetConfirm') }}
      </button>
    </template>
  </CommonModal>
</template>

<style scoped lang="scss">

.factory_reset_list {
  @extend %bullet_list;
  --bullet-tone: var(--alert-tone);
}

.factory_reset_field {
  display: grid;
  gap: $space_2;

  label {
    font-weight: 600;
  }
}

.factory_reset_waiting {
  @extend %fx_base;
  gap: $space_3;
}

</style>
