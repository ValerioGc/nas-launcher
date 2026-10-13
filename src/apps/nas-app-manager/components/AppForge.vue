<script setup lang="ts">

import { computed } from 'vue';
import { storeToRefs } from 'pinia';
import { t } from '@/i18n';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import refreshIcon from '@/assets/icons/system/refresh.svg';

const manager = useAppManagerStore();
const { forge, checking, busy, updates, checkError, endpoint, selected } = storeToRefs(manager);
const onInstalled = computed(() => endpoint.value === 'installed');
const offline = computed(() => Boolean(forge.value && !forge.value.packagesEnabled));

const tokenMissing = computed(() =>
  Boolean(forge.value?.packagesEnabled && !forge.value.configured),
);
const selectedUpdate = computed(() => Boolean(selected.value?.updateAvailable));
const updateLabel = computed(() => t(selectedUpdate.value ? 'updateSelected' : 'updateAll'));

function update() {
  void manager.updateApps(selectedUpdate.value && selected.value ? [selected.value.id] : undefined);
}

</script>

<template>
  <div
    v-if="onInstalled || offline || tokenMissing"
    class="manager_forge"
    :aria-label="t('packageSource')"
  >
    <div v-if="onInstalled || offline" class="app_toolbar manager_update_toolbar">
      <button
        v-if="onInstalled && !updates"
        type="button"
        :disabled="checking || busy || !forge?.packagesEnabled"
        data-testid="app_manager_check"
        @click="manager.check"
      >
        <CommonSpinner v-if="checking" size="small" compact />
        <CommonIcon v-else :src="refreshIcon" size="small" />
        {{ t('checkUpdates') }}
      </button>

      <button
        v-else-if="onInstalled"
        type="button"
        class="primary"
        :disabled="checking || busy || !forge?.packagesEnabled || !forge.configured"
        data-testid="app_manager_update_available"
        @click="update"
      >
        <CommonSpinner v-if="busy" size="small" compact />
        <CommonIcon v-else :src="refreshIcon" size="small" />
        {{ updateLabel }}
      </button>

      <p v-if="offline" class="muted" data-testid="app_manager_offline">
        {{ t('packagesDisabled') }}
      </p>
    </div>

    <AlertBanner v-if="tokenMissing" tone="warning" compact data-testid="app_manager_token_status">
      {{ t('tokenMissing') }}
    </AlertBanner>

    <AlertBanner
      v-if="onInstalled && checkError"
      tone="error"
      compact
      data-testid="app_manager_check_error"
    >
      {{ checkError }}
    </AlertBanner>
  </div>
</template>

<style scoped lang="scss">

.manager_forge {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: $space_2;

  p {
    margin: 0;
  }
}

.manager_update_toolbar {
  flex-wrap: nowrap;
}

</style>
