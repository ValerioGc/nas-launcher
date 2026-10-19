<script setup lang="ts">

import { ref } from 'vue';
import { useResource } from '@/composables/useResource';
import { t } from '@/i18n';
import type { ResetState } from '../types/reset';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import resetIcon from '@/assets/icons/system/reset.svg';
import FactoryResetModal from './FactoryResetModal.vue';
import SettingsStatus from './SettingsStatus.vue';

const { data, loading, error } = useResource<ResetState | null>('/system/reset', null);

const confirming = ref(false);

</script>

<template>
  <section
    class="app_panel settings_group"
    :aria-label="t('factoryReset')"
    data-testid="factory_reset"
  >
    <h2>{{ t('factoryReset') }}</h2>
    <p class="muted">{{ t('factoryResetNote') }}</p>

    <AlertBanner v-if="data?.reason" tone="info" compact data-testid="factory_reset_unavailable">
      {{ t(`factoryReset_${data.reason}`) }}
    </AlertBanner>

    <div class="app_toolbar">
      <button
        type="button"
        class="danger"
        :disabled="!data?.available"
        data-testid="factory_reset_button"
        @click="confirming = true"
      >
        <CommonIcon :src="resetIcon" size="small" tinted />
        {{ t('factoryResetAction') }}
      </button>
    </div>

    <SettingsStatus :loading="loading" :error="error" />

    <FactoryResetModal v-if="confirming" v-model="confirming" />
  </section>
</template>
