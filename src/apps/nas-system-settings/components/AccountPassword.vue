<script setup lang="ts">

import { computed, ref } from 'vue';
import { t } from '@/i18n';
import { changePassword } from '@/api/account';
import { confirmPassword } from '@/api/gateway';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonPasswordInput from '@/components/commons/CommonPasswordInput.vue';
import { passwordFailure } from '../passwordFailure';

const props = withDefaults(defineProps<{ username?: string }>(), { username: '' });

const current = ref('');
const next = ref('');
const repeated = ref('');
const busy = ref(false);
const failure = ref('');
const done = ref(false);

const mismatch = computed(() => Boolean(repeated.value) && next.value !== repeated.value);
const valid = computed(
  () => next.value.length >= 8 && next.value.length <= 128 && next.value === repeated.value,
);

async function submit() {
  busy.value = true;
  failure.value = '';
  done.value = false;
  try {
    const code = await confirmPassword(current.value);
    await changePassword(next.value, code);
    next.value = '';
    repeated.value = '';
    done.value = true;
  } catch (error) {
    failure.value = passwordFailure(error);
  } finally {
    current.value = '';
    busy.value = false;
  }
}

</script>

<template>
  <form
    class="app_panel settings_group"
    :aria-label="t('accountPasswordTitle')"
    data-testid="account_password"
    @submit.prevent="submit"
  >
    <h2>{{ t('accountPasswordTitle') }}</h2>
    <div class="settings_fields">
      <label class="settings_field settings_full" for="account_current_password">
        {{ t('accountCurrentPassword') }}
        <CommonPasswordInput
          id="account_current_password"
          v-model="current"
          :username="props.username"
          required
          maxlength="128"
          data-testid="account_current_password"
        />
      </label>

      <!-- New password field -->
      <label class="settings_field" for="account_new_password">
        {{ t('accountNewPassword') }}
        <CommonPasswordInput
          id="account_new_password"
          v-model="next"
          new-password
          required
          minlength="8"
          maxlength="128"
          data-testid="account_new_password"
        />
        <span class="muted">{{ t('fieldLength', { min: 8, max: 128 }) }}</span>
      </label>

      <!-- Repeat new password field -->
      <label class="settings_field" for="account_repeat_password">
        {{ t('accountRepeatPassword') }}
        <CommonPasswordInput
          id="account_repeat_password"
          v-model="repeated"
          new-password
          required
          maxlength="128"
          data-testid="account_repeat_password"
        />
      </label>
    </div>
    <AlertBanner v-if="mismatch" tone="warning" compact>{{
      t('accountPasswordMismatch')
    }}</AlertBanner>
    <AlertBanner v-if="failure" tone="error" compact>{{ failure }}</AlertBanner>
    <AlertBanner v-else-if="done" tone="success" compact>{{
      t('accountPasswordChanged')
    }}</AlertBanner>
    <div class="app_toolbar settings_actions">
      <button
        type="submit"
        class="primary"
        :disabled="busy || !valid || !current"
        data-testid="account_password_save"
      >
        {{ t('save') }}
      </button>
    </div>
  </form>
</template>
