<script setup lang="ts">

import { t } from '@/i18n';
import type { SystemAction } from '@/types/types';
import CommonModal from '@/components/commons/CommonModal.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';

defineProps<{
  open: boolean;
  selected: SystemAction | null;
  remaining: number | null;
  busy: boolean;
  error: string;
  tasks?: string[];
  jobs?: string[];
}>();

const emit = defineEmits<{
  (e: 'cancel'): void;
  (e: 'execute'): void;
}>();

function cancel() {
  emit('cancel');
}

function execute() {
  emit('execute');
}

</script>

<template>
  <CommonModal
    :model-value="open"
    :title="selected ? t(selected.id) : t('confirm')"
    :hide-close-button="busy"
    @update:model-value="!$event && cancel()"
  >
    <p>{{ t('confirmSystemAction', { action: selected ? t(selected.id) : '' }) }}</p>
    <p v-if="remaining !== null" role="timer" aria-live="off" data-testid="system_action_countdown">
      {{ t('automaticSystemAction', { seconds: remaining }) }}
    </p>
    <p v-if="selected?.id === 'logout'">{{ t('logoutNote') }}</p>
    <template v-if="jobs?.length">
      <p role="alert">{{ t('powerJobsRunning') }}</p>
      <ul data-testid="system_action_jobs">
        <li v-for="job in jobs" :key="job">{{ t(`powerJob_${job}`) }}</li>
      </ul>
    </template>
    <template v-else-if="tasks?.length">
      <p>{{ t('powerTasksInterrupted') }}</p>
      <ul data-testid="system_action_tasks">
        <li v-for="task in tasks" :key="task">{{ task }}</li>
      </ul>
    </template>
    <CommonSpinner v-if="busy" />
    <p v-if="error" role="alert">{{ error }}</p>

    <template #footer>
      <button type="button" :disabled="busy" data-testid="system_action_cancel" @click="cancel">
        {{ t('cancel') }}
      </button>
      <button
        type="button"
        :disabled="busy || Boolean(jobs?.length)"
        data-testid="system_action_confirm"
        @click="execute"
      >
        {{ t('confirm') }}
      </button>
    </template>
  </CommonModal>
</template>
