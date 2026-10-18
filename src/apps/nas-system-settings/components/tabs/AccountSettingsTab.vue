<script setup lang="ts">

import { onMounted, ref } from 'vue';
import { t } from '@/i18n';
import { mailStatus, readAccount } from '@/api/account';
import { failureMessage } from '@/utils/failureMessage';
import type { AccountInfo, MailStatus } from '@/types/types';

import AccountEmail from '../AccountEmail.vue';
import AccountPassword from '../AccountPassword.vue';
import AccountUsername from '../AccountUsername.vue';
import SettingsTabLayout from '../SettingsTabLayout.vue';

const account = ref<AccountInfo | null>(null);
const mail = ref<MailStatus>({ installed: false, sender: false });
const loading = ref(false);
const error = ref('');

async function load() {
  loading.value = true;
  error.value = '';
  try {
    // Whether mail can go only decides if the recovery address can be set: it never stops the page.
    const [read, status] = await Promise.all([readAccount(), mailStatus().catch(() => mail.value)]);
    account.value = read;
    mail.value = status;
  } catch (reason) {
    error.value = failureMessage(reason);
  } finally {
    loading.value = false;
  }
}

onMounted(load);

</script>

<template>
  <SettingsTabLayout
    as="section"
    :title="t('account')"
    :ready="Boolean(account)"
    :loading="loading"
    :error="error"
    @retry="load"
  >
    <template v-if="account">
      <p class="muted">{{ t('accountNote') }}</p>
      <AccountUsername :account="account" @changed="account = $event" />
      <AccountPassword :username="account.username" />
      <AccountEmail :account="account" :mail="mail" @changed="account = $event" />
    </template>
  </SettingsTabLayout>
</template>
