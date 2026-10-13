<script setup lang="ts">

import { computed, ref, useTemplateRef, watch } from 'vue';
import { storeToRefs } from 'pinia';
import { t } from '@/i18n';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import type { ManagerRow } from '@/apps/nas-app-manager/types';
import type { TableColumn } from '@/types/types';
import AppListItem from './AppListItem.vue';
import CommonEmpty from '@/components/commons/CommonEmpty.vue';
import CommonTable from '@/components/commons/CommonTable.vue';
import ShortcutContextMenu from '@/components/desktop/ShortcutContextMenu.vue';
import gridIcon from '@/assets/icons/navigation/view_grid.svg';

const manager = useAppManagerStore();
const { listed, selected, endpoint, loading, view } = storeToRefs(manager);

const columns: TableColumn<ManagerRow>[] = [
  { key: 'name', label: 'name', sortable: true },
  { key: 'info', label: 'info', sortable: true, class: 'manager_info' },
  { key: 'version', label: 'version', class: 'manager_version' },
];

const sortBy = ref<keyof ManagerRow & string>('name');
const ascending = ref(true);
const shortcutMenu = useTemplateRef<InstanceType<typeof ShortcutContextMenu>>('shortcutMenu');

const emptyTitle = computed(() =>
  t(endpoint.value === 'installed' ? 'noInstalledApps' : 'noAvailableApps'),
);

const emptyHint = computed(() =>
  t(endpoint.value === 'installed' ? 'noInstalledAppsHint' : 'noAvailableAppsHint'),
);

function context(app: ManagerRow, event: MouseEvent | KeyboardEvent) {
  if (endpoint.value === 'installed') {
    shortcutMenu.value?.show(app.id, event);
  }
}

watch(endpoint, () => shortcutMenu.value?.close());

</script>

<template>
  <div v-if="view === 'list'" class="manager_table" data-testid="app_manager_catalog">
    <CommonTable
      v-if="listed.length"
      v-model:sort-by="sortBy"
      v-model:ascending="ascending"
      :rows="listed"
      :columns="columns"
      row-key="id"
      :row-class="(app) => ({ manager_row_selected: selected?.id === app.id })"
      testid="app_manager"
    >
      <template #cell_name="{ row }">
        <AppListItem :app="row" @context="context(row, $event)" />
      </template>
      <template #cell_version="{ row }">
        <span :data-testid="`app_manager_version_${row.id}`">{{ row.version || '-' }}</span>
      </template>
    </CommonTable>

    <CommonEmpty
      v-else-if="!loading"
      :title="emptyTitle"
      :hint="emptyHint"
      :icon="gridIcon"
      testid="app_manager_empty"
    />
  </div>

  <ul
    v-else
    class="app_list manager_preview"
    :class="{ manager_preview_hollow: !listed.length && !loading }"
    data-testid="app_manager_catalog"
  >
    <li v-for="app in listed" :key="app.id">
      <AppListItem :app="app" @context="context(app, $event)" />
    </li>

    <li v-if="!listed.length && !loading" class="manager_preview_empty">
      <CommonEmpty
        :title="emptyTitle"
        :hint="emptyHint"
        :icon="gridIcon"
        testid="app_manager_empty"
      />
    </li>
  </ul>
  <ShortcutContextMenu ref="shortcutMenu" testid="app_manager_shortcut" />
</template>

<style scoped lang="scss">

.manager_table {
  @extend %panel;
  height: $catalog_height;

  :deep(td) {
    padding: $space_2 $space_3;
  }

  :deep(.manager_row_selected td) {
    background: var(--surface-hover);
  }

  :deep(.manager_version) {
    font-variant-numeric: tabular-nums;
  }

  :deep(.manager_info) {
    white-space: normal;
    min-width: $dropdown_min_width;
    color: var(--surface-muted);
  }
}

.manager_preview {
  @extend %panel;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, $metric_min_width), 1fr));
  align-content: start;
  gap: $space_4;
  padding: $space_4;
  height: $catalog_height;
  overflow-y: auto;
  background: var(--surface-sunken);

  .manager_app {
    height: 100%;
    flex-direction: column;
    justify-content: start;
    text-align: center;
    padding: $space_4;

    &:hover:not(:disabled),
    &[aria-pressed='true'] {
      background: var(--surface-hover);
    }

    &.manager_app_system:hover:not(:disabled) {
      background: transparent;
    }

    :deep(img) {
      width: $app_preview_icon;
      height: $app_preview_icon;
    }

    :deep(span) {
      font-size: $text_label;
      overflow-wrap: anywhere;
    }
  }

  &_empty {
    grid-column: 1 / -1;
  }

  &_hollow {
    display: flex;
    align-items: center;
    justify-content: center;
  }
}

</style>
