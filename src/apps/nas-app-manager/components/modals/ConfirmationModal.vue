<script setup lang="ts">

import { t } from '@/i18n';
import type { ManageAction } from '@/apps/nas-app-manager/types';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonModal from '@/components/commons/CommonModal.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';

defineProps<{
  appName: string;
  action: ManageAction | 'remove';
  busy: boolean;
  error: string;
}>();

const open = defineModel<boolean>({ required: true });
const removeData = defineModel<boolean>('removeData', { default: true });
const emit = defineEmits<{
  confirm: [];
}>();

</script>

<template>
  <CommonModal v-model="open" :title="t('confirm')">
    <p>{{ t(`confirm_${action}`, { name: appName }) }}</p>
    <CommonSwitch
      v-if="action === 'uninstall'"
      v-model="removeData"
      :label="t('removeAppData')"
      testid="app_manager_remove_data"
    />
    <AlertBanner v-if="error" tone="error" compact>{{ error }}</AlertBanner>

    <template #footer>
      <button type="button" :disabled="busy" data-testid="app_manager_cancel" @click="open = false">
        {{ t('cancel') }}
      </button>
      <button
        type="button"
        class="primary"
        :disabled="busy"
        data-testid="app_manager_confirm"
        @click="emit('confirm')"
      >
        {{ t('confirm') }}
      </button>
    </template>
  </CommonModal>
</template>
