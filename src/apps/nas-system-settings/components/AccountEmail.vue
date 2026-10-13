<script setup lang="ts">

import { ref, watch } from 'vue';
import { t } from '@/i18n';
import { removeEmail, requestEmail, setRecovery, verifyEmail } from '@/api/account';
import { confirmPassword } from '@/api/gateway';
import { useShellStore } from '@/stores/shellStore';
import type { AccountInfo, MailStatus } from '@/types/types';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonPasswordInput from '@/components/commons/CommonPasswordInput.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import openIcon from '@/assets/icons/navigation/open.svg';
import { passwordFailure } from '../passwordFailure';

const props = defineProps<{ account: AccountInfo; mail: MailStatus }>();
const emit = defineEmits<{ changed: [account: AccountInfo] }>();

const MAIL_APP = 'nas-mail-client';
const shell = useShellStore();

// The switch as the user set it, saved only with the password.
const enabled = ref(props.account.recovery);
const switchPassword = ref('');
const email = ref('');
const password = ref('');
const code = ref('');
// The address a code went to, waiting for that code to come back.
const pending = ref('');
const busy = ref(false);
const failure = ref('');
const notice = ref('');

/** The SMTP server of the Email app, or the App Manager while the app is missing. */
function configureMail() {
  if (props.mail.installed) {
    shell.open(MAIL_APP, { location: '?section=smtp' });
  } else {
    shell.open('nas-app-manager');
  }
}

watch(
  () => props.account.recovery,
  (recovery) => {
    enabled.value = recovery;
  },
);

async function run(action: () => Promise<void>) {
  busy.value = true;
  failure.value = '';
  notice.value = '';
  try {
    await action();
  } catch (error) {
    failure.value = passwordFailure(error);
  } finally {
    busy.value = false;
  }
}

function saveSwitch() {
  return run(async () => {
    emit('changed', await setRecovery(enabled.value, await confirmPassword(switchPassword.value)));
    switchPassword.value = '';
    notice.value = t(enabled.value ? 'accountRecoveryOn' : 'accountRecoveryOff');
  });
}

function send() {
  return run(async () => {
    const address = email.value.trim();
    await requestEmail(address, await confirmPassword(password.value));
    password.value = '';
    pending.value = address;
    code.value = '';
  });
}

function verify() {
  return run(async () => {
    emit('changed', await verifyEmail(code.value.trim()));
    pending.value = '';
    email.value = '';
    code.value = '';
    notice.value = t('accountEmailVerified');
  });
}

function remove() {
  return run(async () => {
    await removeEmail(await confirmPassword(password.value));
    password.value = '';
    emit('changed', { ...props.account, email: null });
    notice.value = t('accountEmailRemoved');
  });
}

</script>

<template>
  <section
    class="app_panel settings_group"
    :aria-label="t('accountEmail')"
    data-testid="account_email"
  >
    <h2>{{ t('accountEmail') }}</h2>
    <p class="muted">{{ t('accountEmailNote') }}</p>

    <CommonSwitch
      v-model="enabled"
      :label="t('accountRecoverySwitch')"
      :disabled="busy || (!mail.sender && !account.recovery)"
      testid="account_recovery"
    />
    <form
      v-if="enabled !== account.recovery"
      class="settings_fields"
      data-testid="account_recovery_form"
      @submit.prevent="saveSwitch"
    >
      <label class="settings_field" for="account_recovery_password">
        {{ t('accountCurrentPassword') }}
        <CommonPasswordInput
          id="account_recovery_password"
          v-model="switchPassword"
          required
          maxlength="128"
          data-testid="account_recovery_password"
        />
      </label>
      <div class="app_toolbar settings_full">
        <button type="button" :disabled="busy" @click="enabled = account.recovery">
          {{ t('cancel') }}
        </button>
        <button type="submit" class="primary" :disabled="busy || !switchPassword">
          {{ t('accountRecoverySave') }}
        </button>
      </div>
    </form>

    <p v-if="account.recovery" data-testid="account_email_current">
      {{
        account.email ? t('accountEmailCurrent', { email: account.email }) : t('accountEmailNone')
      }}
    </p>

    <template v-if="!mail.sender">
      <AlertBanner tone="info" compact data-testid="account_email_unavailable">
        {{ mail.installed ? t('accountEmailUnavailable') : t('accountEmailNotInstalled') }}
      </AlertBanner>
      <div class="app_toolbar">
        <button type="button" data-testid="account_configure_mail" @click="configureMail">
          <CommonIcon :src="openIcon" size="small" />
          {{ mail.installed ? t('accountOpenMail') : t('accountInstallMail') }}
        </button>
      </div>
    </template>

    <form v-if="account.recovery && pending" class="settings_fields" @submit.prevent="verify">
      <p class="settings_full">{{ t('accountCodeSent', { email: pending }) }}</p>
      <label class="settings_field">
        {{ t('accountCode') }}
        <input
          v-model="code"
          name="account_email_code"
          type="text"
          inputmode="numeric"
          autocomplete="one-time-code"
          pattern="[0-9]{6}"
          maxlength="6"
          required
          data-testid="account_email_code"
        />
      </label>
      <div class="app_toolbar settings_full">
        <button type="button" :disabled="busy" @click="pending = ''">{{ t('cancel') }}</button>
        <button type="submit" class="primary" :disabled="busy || code.trim().length !== 6">
          {{ t('accountVerify') }}
        </button>
      </div>
    </form>

    <form v-else-if="account.recovery" class="settings_fields" @submit.prevent="send">
      <label class="settings_field">
        {{ t('accountNewEmail') }}
        <input
          v-model="email"
          name="account_email"
          type="email"
          autocomplete="email"
          maxlength="254"
          :disabled="!mail.sender"
          required
          data-testid="account_new_email"
        />
      </label>
      <label class="settings_field" for="account_email_password">
        {{ t('accountCurrentPassword') }}
        <CommonPasswordInput
          id="account_email_password"
          v-model="password"
          maxlength="128"
          :disabled="!mail.sender && !account.email"
          data-testid="account_email_password"
        />
      </label>
      <div class="app_toolbar settings_full">
        <button
          v-if="account.email"
          type="button"
          :disabled="busy || !password"
          data-testid="account_remove_email"
          @click="remove"
        >
          {{ t('accountRemoveEmail') }}
        </button>
        <button
          type="submit"
          class="primary"
          :disabled="busy || !mail.sender || !email.trim() || !password"
          data-testid="account_send_code"
        >
          {{ t('accountSendCode') }}
        </button>
      </div>
    </form>

    <AlertBanner v-if="failure" tone="error" compact>{{ failure }}</AlertBanner>
    <AlertBanner v-else-if="notice" tone="success" compact>{{ notice }}</AlertBanner>
  </section>
</template>
