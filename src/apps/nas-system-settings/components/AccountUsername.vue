<script setup lang="ts">

import { computed, ref } from 'vue';
import { t } from '@/i18n';
import { changeUsername } from '@/api/account';
import { confirmPassword } from '@/api/gateway';
import type { AccountInfo } from '@/types/types';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonPasswordInput from '@/components/commons/CommonPasswordInput.vue';
import { passwordFailure } from '../passwordFailure';

const props = defineProps<{ account: AccountInfo }>();
const emit = defineEmits<{ changed: [account: AccountInfo] }>();

const USERNAME = /^[A-Za-z_][A-Za-z0-9._-]{2,24}$/;

const username = ref('');
const password = ref('');
const busy = ref(false);
const failure = ref('');
const done = ref(false);

const valid = computed(
  () => USERNAME.test(username.value.trim()) && username.value.trim() !== props.account.username,
);

const invalid = computed(
  () => username.value.trim() !== '' && !USERNAME.test(username.value.trim()),
);

async function submit() {
  busy.value = true;
  failure.value = '';
  done.value = false;
  try {
    const code = await confirmPassword(password.value);
    emit('changed', await changeUsername(username.value.trim(), code));
    username.value = '';
    done.value = true;
  } catch (error) {
    failure.value = passwordFailure(error);
  } finally {
    password.value = '';
    busy.value = false;
  }
}

</script>

<template>
  <form
    class="app_panel settings_group"
    :aria-label="t('accountUsername')"
    data-testid="account_username"
    @submit.prevent="submit"
  >
    <h2>{{ t('accountUsername') }}</h2>
    <p class="muted">{{ t('accountUsernameNote') }}</p>
    <div class="settings_fields">
      <label class="settings_field">
        {{ t('accountNewUsername') }}
        <input
          v-model="username"
          name="account_username"
          type="text"
          autocomplete="username"
          autocapitalize="none"
          :placeholder="account.username"
          maxlength="25"
          required
          :aria-invalid="invalid"
          aria-describedby="account_username_rules"
          data-testid="account_new_username"
        />

        <span v-if="invalid" id="account_username_rules" class="app_error" role="alert">
          {{ t('accountUsernameInvalid') }}
        </span>
        <span v-else id="account_username_rules" class="muted">
          {{ t('accountUsernameRules') }}
        </span>
      </label>
      <label class="settings_field" for="account_username_password">
        {{ t('accountCurrentPassword') }}
        <CommonPasswordInput
          id="account_username_password"
          v-model="password"
          required
          maxlength="128"
          data-testid="account_username_password"
        />
      </label>
    </div>

    <!-- Alert banners -->
    <AlertBanner v-if="failure" tone="error" compact>{{ failure }}</AlertBanner>
    <AlertBanner v-else-if="done" tone="success" compact>
      {{ t('accountUsernameChanged') }}
    </AlertBanner>

    <!-- Submit button -->
    <div class="app_toolbar settings_actions">
      <button
        type="submit"
        class="primary"
        :disabled="busy || !valid || !password"
        data-testid="account_username_save"
      >
        {{ t('save') }}
      </button>
    </div>
  </form>
</template>
