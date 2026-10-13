<script setup lang="ts" generic="T extends object">

import { computed } from 'vue';
import { t, locale } from '@/i18n';
import { sortRows } from '@/utils/sortRows';
import type { TableColumn } from '@/types/types';

interface Props {
  rows: readonly T[];
  columns: TableColumn<T>[];
  rowKey: keyof T & string;
  rowClass?: (row: T) => string | Record<string, boolean>;
  actionsLabel?: string;
  testid?: string;
  /** A menu for a row, opened with the right button or the menu key: the row can take focus. */
  onRowContext?: (row: T, event: MouseEvent) => void;
}

const props = withDefaults(defineProps<Props>(), {
  rowClass: undefined,
  actionsLabel: 'actions',
  testid: undefined,
  onRowContext: undefined,
});

defineSlots<
  {
    actions?: (scope: { row: T }) => unknown;
  } & {
    [key: `cell_${string}`]: ((scope: { row: T }) => unknown) | undefined;
  }
>();

const arrows = { ascending: '↑', descending: '↓', none: '' } as const;

const column = defineModel<keyof T & string>('sortBy');
const ascending = defineModel<boolean>('ascending', { default: true });

const sorted = computed(() =>
  column.value
    ? sortRows(props.rows, column.value, ascending.value, locale.value, props.rowKey)
    : props.rows,
);

function sort(key: keyof T & string) {
  ascending.value = column.value === key ? !ascending.value : true;
  column.value = key;
}

function context(row: T, event: MouseEvent) {
  if (props.onRowContext) {
    event.preventDefault();
    props.onRowContext(row, event);
  }
}

function order(key: keyof T & string): keyof typeof arrows {
  if (column.value !== key) {
    return 'none';
  }

  return ascending.value ? 'ascending' : 'descending';
}

</script>

<template>
  <div class="common_table">
    <table :data-testid="testid">
      <thead>
        <tr>
          <th
            v-for="item in columns"
            :key="item.key"
            scope="col"
            :aria-sort="item.sortable ? order(item.key) : undefined"
          >
            <button
              v-if="item.sortable"
              type="button"
              :data-testid="testid && `${testid}_sort_${item.key}`"
              @click="sort(item.key)"
            >
              {{ t(item.label) }}
              <span aria-hidden="true">{{ arrows[order(item.key)] }}</span>
            </button>
            <template v-else>{{ t(item.label) }}</template>
          </th>

          <th v-if="$slots.actions" scope="col" class="common_table_actions">
            <span class="visually_hidden">{{ t(actionsLabel) }}</span>
          </th>
        </tr>
      </thead>

      <tbody>
        <tr
          v-for="row in sorted"
          :key="String(row[rowKey])"
          :class="rowClass?.(row)"
          :data-testid="testid && `${testid}_row_${String(row[rowKey])}`"
          :tabindex="onRowContext ? 0 : undefined"
          @contextmenu="context(row, $event)"
        >
          <td v-for="item in columns" :key="item.key" :class="item.class">
            <slot :name="`cell_${item.key}`" :row="row">{{ row[item.key] }}</slot>
          </td>

          <td v-if="$slots.actions" class="common_table_actions">
            <slot name="actions" :row="row"></slot>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped lang="scss">

.common_table {
  @extend %sortable_columns;
  overflow: auto;
  min-width: 0;
  max-height: 100%;

  th button {
    font-weight: inherit;
  }
}

.common_table_actions {
  width: 1%;
  text-align: right;
}

</style>
