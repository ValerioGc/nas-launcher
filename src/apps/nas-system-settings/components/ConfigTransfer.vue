<script setup lang="ts">

import { computed, ref } from 'vue';
import { t } from '@/i18n';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import ConfigTransferModal from './ConfigTransferModal.vue';
import exportIcon from '@/assets/icons/navigation/export.svg';
import importIcon from '@/assets/icons/navigation/download.svg';

const mode = ref<'export' | 'import' | null>(null);

const open = computed({
  get: () => mode.value !== null,
  set: (value: boolean) => {
    if (!value) {
      mode.value = null;
    }
  },
});

</script>

<template>
  <section
    class="app_panel settings_group"
    :aria-label="t('configTransfer')"
    data-testid="config_transfer"
  >
    <h2>{{ t('configTransfer') }}</h2>
    <p class="muted">{{ t('configTransferNote') }}</p>

    <div class="app_toolbar">
      <button type="button" data-testid="config_export_button" @click="mode = 'export'">
        <CommonIcon :src="exportIcon" size="small" />
        {{ t('configExportAction') }}
      </button>
      <button type="button" data-testid="config_import_button" @click="mode = 'import'">
        <CommonIcon :src="importIcon" size="small" />
        {{ t('configImportAction') }}
      </button>
    </div>

    <ConfigTransferModal v-if="mode" v-model="open" :mode="mode" />
  </section>
</template>
