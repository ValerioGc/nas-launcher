<script setup lang="ts">

import { computed, ref } from 'vue';
import { t } from '@/i18n';
import { useResource } from '@/composables/useResource';
import { usePolling } from '@/composables/usePolling';
import type { SystemPackage, SystemUpdates } from '@/apps/nas-system-settings/types/update';
import type { TableColumn } from '@/types/types';

import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';
import CommonTable from '@/components/commons/CommonTable.vue';
import refreshIcon from '@/assets/icons/system/refresh.svg';
import LauncherUpdates from '../LauncherUpdates.vue';
import SettingsTabLayout from '../SettingsTabLayout.vue';
import SettingsStatus from '../SettingsStatus.vue';

const POLL_MS = 3000;

const { data, loading, error, busy, load, act } = useResource<SystemUpdates | null>(
  '/system/updates',
  null,
);

const columns: TableColumn<SystemPackage>[] = [
  { key: 'id', label: 'package' },
  { key: 'current', label: 'installedVersion' },
  { key: 'version', label: 'availableVersion' },
];

const selected = ref<string[]>([]);
const running = computed(() => data.value?.running ?? null);
const idle = computed(() => Boolean(data.value?.manageable) && !running.value && !busy.value);

function choose(id: string, chosen: boolean) {
  selected.value = chosen ? [...selected.value, id] : selected.value.filter((item) => item !== id);
}

function refresh() {
  return act('/system/updates/refresh', 'POST', {});
}

async function update() {
  if (await act('/system/updates', 'POST', { ids: selected.value })) {
    selected.value = [];
  }
}

usePolling(
  () => {
    if (running.value && !loading.value) {
      load();
    }
  },
  computed(() => POLL_MS),
);

</script>

<template>
  <SettingsTabLayout as="section" :title="t('updates')" :intro="t('updatesIntro')">
    <LauncherUpdates />

    <section
      class="app_panel settings_group"
      :aria-label="t('systemUpdates')"
      data-testid="system_updates"
    >
      <header class="settings_heading">
        <h2>{{ t('systemUpdates') }}</h2>
        <div v-if="data?.packages.length" class="app_toolbar">
          <button
            type="button"
            :disabled="!idle"
            :aria-label="t('checkUpdates')"
            :title="t('checkUpdates')"
            data-testid="system_updates_recheck"
            @click="refresh"
          >
            <CommonIcon :src="refreshIcon" />
          </button>
          <button
            type="button"
            class="primary"
            :disabled="!idle || !selected.length"
            data-testid="system_updates_apply"
            @click="update"
          >
            {{ t('applyUpdates') }}
          </button>
        </div>
        <button
          v-else
          type="button"
          :disabled="!idle"
          data-testid="system_updates_refresh"
          @click="refresh"
        >
          {{ t('checkUpdates') }}
        </button>
      </header>

      <p v-if="data && !data.manageable" class="muted" data-testid="system_updates_readonly">
        {{ t('aptReadOnly') }}
      </p>

      <CommonSpinner
        v-if="running"
        :label="t(running === 'refresh' ? 'aptRefreshing' : 'aptUpgrading')"
        testid="system_updates_running"
      />
      <SettingsStatus :loading="loading && !data" :error="error" />

      <CommonTable
        v-if="data?.packages.length"
        class="system_updates_list"
        :class="{ system_updates_busy: running }"
        :aria-busy="Boolean(running)"
        :rows="data.packages"
        :columns="columns"
        row-key="id"
        testid="system_packages"
      >
        <template #cell_id="{ row }">
          <CommonSwitch
            :model-value="selected.includes(row.id)"
            :name="`system_package_${row.id}`"
            :label="row.id"
            :disabled="!idle"
            @update:model-value="choose(row.id, $event)"
          />
        </template>
      </CommonTable>

      <p v-else-if="data && !running" class="muted" data-testid="system_updates_none">
        {{ t('systemUpToDate') }}
      </p>
    </section>
  </SettingsTabLayout>
</template>

<style scoped lang="scss">

.common_table.system_updates_list {
  overflow: visible;
  max-height: none;
  transition: opacity $duration_fast;
}

.system_updates_busy {
  opacity: 0.55;
}

</style>
