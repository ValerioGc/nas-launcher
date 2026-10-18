<script setup lang="ts">

import { computed, ref, watch } from 'vue';
import { storeToRefs } from 'pinia';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import ConfirmationModal from './modals/ConfirmationModal.vue';
import DetailModal from './modals/DetailModal.vue';
import { t } from '@/i18n';
import type { ManageAction } from '@/apps/nas-app-manager/types';
import CommonIcon from '@/components/commons/CommonIcon.vue';

import deleteIcon from '@/assets/icons/navigation/delete.svg';
import downloadIcon from '@/assets/icons/navigation/download.svg';
import infoIcon from '@/assets/icons/status/info.svg';

interface ActionButton {
  id: 'default' | 'details';
  label: string;
  icon: string;
  disabled: boolean;
  primary: boolean;
  testid?: string;
}

const manager = useAppManagerStore();
const { selected, endpoint, forge, busy, loading, error, installing } = storeToRefs(manager);
const confirm = ref(false);
const detail = ref(false);
const pending = ref<ManageAction>('install');
const removeData = ref(true);

watch(
  () => selected.value?.id,
  () => {
    confirm.value = false;
    detail.value = false;
  },
);

async function confirmAction() {
  if (await manager.change(pending.value, { removeData: removeData.value })) {
    confirm.value = false;
  }
}

const actions = computed<ActionButton[]>(() => {
  if (!selected.value) {
    return [];
  }

  const disabled = busy.value || loading.value;
  const fetchable =
    Boolean(forge.value?.packagesEnabled && forge.value.configured) &&
    !installing.value.includes(selected.value.id);
  return [
    {
      id: 'default',
      label: t(endpoint.value === 'installed' ? 'uninstall' : 'install'),
      icon: endpoint.value === 'installed' ? deleteIcon : downloadIcon,
      disabled: disabled || (endpoint.value === 'available' && !fetchable),
      primary: false,
    },
    {
      id: 'details',
      label: t('details'),
      icon: infoIcon,
      disabled,
      primary: false,
      testid: 'app_manager_details',
    },
  ];
});

function run(id: ActionButton['id']) {
  if (id === 'details') {
    detail.value = true;
  } else {
    pending.value = endpoint.value === 'available' ? 'install' : 'uninstall';
    removeData.value = true;
    confirm.value = true;
  }
}

</script>

<template>
  <div v-if="selected" class="app_toolbar" data-testid="app_manager_actions">
    <button
      v-for="action in actions"
      :key="action.id"
      type="button"
      :class="{ primary: action.primary }"
      :disabled="action.disabled"
      :data-testid="action.testid"
      @click="run(action.id)"
    >
      <CommonIcon :src="action.icon" size="small" />
      {{ action.label }}
    </button>
  </div>
  <DetailModal v-model="detail" :app-id="selected?.id ?? null" />
  <ConfirmationModal
    v-model="confirm"
    v-model:remove-data="removeData"
    :app-name="selected?.name ?? ''"
    :action="pending"
    :busy="busy"
    :error="error"
    @confirm="confirmAction"
  />
</template>
