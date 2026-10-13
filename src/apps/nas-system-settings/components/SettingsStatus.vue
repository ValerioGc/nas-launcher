<script setup lang="ts">

import { t } from '@/i18n';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';

withDefaults(
  defineProps<{
    loading?: boolean;
    error?: string;
    retryable?: boolean;
    busyLabel?: string;
    centered?: boolean;
  }>(),
  {
    loading: false,
    error: '',
    retryable: false,
    busyLabel: undefined,
    centered: false,
  },
);

const emit = defineEmits<{
  retry: [];
}>();

</script>

<template>
  <CommonSpinner
    v-if="loading"
    :label="busyLabel"
    :centered="centered"
    :size="centered ? 'large' : 'medium'"
    testid="settings_loading"
  />

  <AlertBanner
    v-if="error"
    tone="error"
    :title="t('somethingWentWrong')"
    data-testid="settings_error"
  >
    <p class="app_error">{{ error }}</p>

    <button v-if="retryable" type="button" @click="emit('retry')">
      {{ t('retry') }}
    </button>
  </AlertBanner>
</template>

<style scoped lang="scss">

p {
  margin: 0 0 $space_2;
}

</style>
