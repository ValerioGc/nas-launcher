<script setup lang="ts">

import { computed } from 'vue';
import { t } from '@/i18n';
import CommonModal from '@/components/commons/CommonModal.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';

const open = defineModel<boolean>({ required: true });

const shown = computed({
  get: () => open.value,
  set: () => {},
});

</script>

<template>
  <CommonModal v-model="shown" :title="t('restartInProgressTitle')" hide-close-button>
    <div class="system_restart_content" aria-live="polite" data-testid="system_restart_overlay">
      <CommonSpinner centered compact :label="t('restartInProgressTitle')" />
      <p>{{ t('restartInProgress') }}</p>
    </div>
  </CommonModal>
</template>

<style scoped lang="scss">

.system_restart_content {
  @extend %stack;
  gap: $space_4;
  text-align: center;

  p {
    margin: 0;
  }
}

</style>
