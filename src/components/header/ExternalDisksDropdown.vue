<script setup lang="ts">

import { computed, ref } from 'vue';
import { t } from '@/i18n';
import { useResource } from '@/composables/useResource';
import { usePolling } from '@/composables/usePolling';
import type { ExternalDisk } from '@/types/types';
import CommonDropdown from '@/components/commons/CommonDropdown.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonModal from '@/components/commons/CommonModal.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import externalIcon from '@/assets/icons/system/external_disk.svg';
import ejectIcon from '@/assets/icons/system/eject.svg';

const { data, busy, error, load, act } = useResource<ExternalDisk[]>('/storage/disks/external', []);
const open = ref(false);
const selected = ref<ExternalDisk | null>(null);
const confirming = computed({
  get: () => selected.value !== null,
  set: (shown: boolean) => {
    if (!shown && !busy.value) {
      selected.value = null;
    }
  },
});

usePolling(() => {
  if (!selected.value) {
    void load();
  }
}, ref(5000));

function ask(disk: ExternalDisk) {
  if (disk.canEject && !busy.value) {
    selected.value = disk;
    open.value = false;
  }
}

async function eject() {
  if (!selected.value || busy.value) {
    return;
  }
  if (await act('/storage/disks/eject', 'POST', { device: selected.value.device }, 130_000)) {
    selected.value = null;
  }
}

</script>

<template>
  <div
    v-if="data.length || selected"
    class="external_disks app_surface"
    data-testid="header_external_disks"
  >
    <CommonDropdown
      v-if="data.length"
      v-model="open"
      :icon="externalIcon"
      :trigger-label="t('externalDisks')"
      :title="t('externalDisks')"
      align="end"
    >
      <li v-for="disk in data" :key="disk.device" class="external_disk">
        <CommonIcon :src="externalIcon" />
        <span class="external_disk_name">
          <strong>{{ disk.name }}</strong>
          <small>/dev/{{ disk.device }}</small>
        </span>
        <button
          type="button"
          :aria-disabled="busy || !disk.canEject"
          :aria-label="`${t('ejectDisk')} ${disk.name}`"
          :title="disk.locked ? t('diskEjectLocked') : t('ejectDisk')"
          :data-testid="`header_eject_${disk.device}`"
          @click="ask(disk)"
        >
          <CommonIcon :src="ejectIcon" tinted />
        </button>
      </li>
    </CommonDropdown>

    <CommonModal
      v-if="selected"
      v-model="confirming"
      :title="t('ejectDisk')"
      :hide-close-button="busy"
    >
      <div class="app_stack">
        <p>{{ t('confirmEjectDisk', { name: selected.name }) }}</p>
        <p v-if="error" role="alert" class="app_error">{{ error }}</p>
        <div class="app_toolbar external_disk_actions">
          <button type="button" :disabled="busy" @click="confirming = false">
            {{ t('cancel') }}
          </button>
          <button type="button" :disabled="busy" data-testid="header_eject_confirm" @click="eject">
            <CommonSpinner v-if="busy" size="small" compact />
            {{ t('ejectDisk') }}
          </button>
        </div>
      </div>
    </CommonModal>
  </div>
</template>

<style scoped lang="scss">

.external_disk {
  @extend %fx_base;
  gap: $space_3;
  padding: $space_2;

  button {
    @extend %icon_button;

    &[aria-disabled='true'] {
      opacity: $opacity_dragged;
      cursor: not-allowed;
    }
  }
}

.external_disk_name {
  @extend %stack;
  flex: 1;
  min-width: 0;

  strong {
    @extend %truncated_text;
  }

  small {
    color: var(--surface-muted);
    font-size: $text_small;
  }
}

.external_disk_actions {
  justify-content: flex-end;
}

</style>
