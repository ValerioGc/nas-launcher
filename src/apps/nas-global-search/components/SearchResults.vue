<script lang="ts" setup>

import { computed, ref } from 'vue';
import { t, locale, formatDate } from '@/i18n';
import { fileEntry } from '@/apps/nas-global-search/entries';
import type {
  SearchEntry,
  SearchResult,
  SearchScope,
  SearchSortColumn,
} from '@/apps/nas-global-search/types';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import SearchEntries from './SearchEntries.vue';

import searchIcon from '@/assets/icons/navigation/search.svg';
import fileIcon from '@/assets/icons/file.svg';
import folderIcon from '@/assets/icons/folder.svg';
import errorIcon from '@/assets/icons/status/error.svg';

interface Props {
  results: SearchResult[];
  entries: SearchEntry[];
  table: boolean;
  loading: boolean;
  errorMessage: string;
  hasQuery: boolean;
  scope: SearchScope;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  open: [entry: SearchEntry];
}>();

const typeLabels: Record<SearchResult['type'], string> = {
  folder: 'folder',
  document: 'document',
  image: 'image',
  audio: 'audio',
  video: 'video',
  archive: 'archive',
};

const column = ref<SearchSortColumn>('name');
const ascending = ref(true);

const columns: { key: SearchSortColumn; label: string }[] = [
  { key: 'name', label: 'name' },
  { key: 'path', label: 'filePath' },
  { key: 'type', label: 'fileType' },
  { key: 'extension', label: 'extension' },
  { key: 'size', label: 'size' },
  { key: 'modified', label: 'modified' },
];

function sort(key: SearchSortColumn) {
  ascending.value = column.value === key ? !ascending.value : true;
  column.value = key;
}

function value(result: SearchResult) {
  if (column.value === 'modified') {
    return Date.parse(result.modified) || 0;
  }

  if (column.value === 'type') {
    return t(typeLabels[result.type]);
  }

  return result[column.value];
}

const rows = computed(() => {
  const collator = new Intl.Collator(locale.value, { numeric: true, sensitivity: 'base' });

  return [...props.results].sort((a, b) => {
    const left = value(a);
    const right = value(b);
    const order =
      typeof left === 'number' && typeof right === 'number'
        ? left - right
        : collator.compare(String(left), String(right));

    return (ascending.value ? order : -order) || collator.compare(a.id, b.id);
  });
});

const state = computed(() => {
  if (props.errorMessage) {
    return {
      kind: 'error',
      icon: errorIcon,
      title: t('searchUnavailable'),
      description: props.errorMessage,
    };
  }

  if (props.loading) {
    return { kind: 'loading', title: t('searching'), description: '' };
  }

  if (!props.hasQuery) {
    return {
      kind: 'idle',
      icon: searchIcon,
      title: t('globalSearch'),
      description: t(`searchPrompt_${props.scope}`),
    };
  }

  if (!(props.table ? props.results.length : props.entries.length)) {
    return {
      kind: 'empty',
      icon: searchIcon,
      title: t('noResults'),
      description: t('searchEmptyHint'),
    };
  }
  return null;
});

const units = ['B', 'kB', 'MB', 'GB', 'TB'];

function formatSize(bytes: number) {
  if (bytes <= 0) {
    return '-';
  }

  let value = bytes;
  let unit = 0;

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }

  return `${value < 10 && unit > 0 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

</script>

<template>
  <div class="search_results" data-testid="search_results" :aria-busy="props.loading">
    <div
      v-if="state"
      class="search_state"
      :role="state.kind === 'error' ? 'alert' : 'status'"
      :data-testid="`search_${state.kind}`"
    >
      <span v-if="state.icon" class="search_state_icon">
        <CommonIcon :src="state.icon" size="large" />
      </span>
      <CommonSpinner v-else size="huge" compact :label="state.title" />
      <p v-if="state.description">{{ state.description }}</p>
    </div>

    <SearchEntries v-else-if="!props.table" :entries="props.entries" @open="emit('open', $event)" />

    <table v-else :aria-label="t('searchResults')">
      <thead>
        <tr>
          <th
            v-for="item in columns"
            :key="item.key"
            scope="col"
            :aria-sort="column === item.key ? (ascending ? 'ascending' : 'descending') : 'none'"
          >
            <button type="button" :data-testid="`search_sort_${item.key}`" @click="sort(item.key)">
              {{ t(item.label) }}
              <span aria-hidden="true">
                {{ column === item.key ? (ascending ? '↑' : '↓') : '↕' }}
              </span>
            </button>
          </th>
        </tr>
      </thead>

      <tbody>
        <tr v-for="result in rows" :key="result.id">
          <td>
            <button
              type="button"
              class="search_result_name"
              :title="result.name"
              :data-testid="`search_result_${result.id}`"
              @click="emit('open', fileEntry(result))"
            >
              <CommonIcon :src="result.type === 'folder' ? folderIcon : fileIcon" size="small" />
              <span>{{ result.name }}</span>
            </button>
          </td>

          <td>
            <span class="search_result_path" :title="result.path">{{ result.path }}</span>
          </td>
          <td>{{ t(typeLabels[result.type]) }}</td>
          <td>{{ result.extension || '-' }}</td>
          <td class="search_result_size">{{ formatSize(result.size) }}</td>
          <td>{{ formatDate(result.modified) || '-' }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style lang="scss" scoped>

.search_results {
  @extend %sortable_columns;
  overflow: auto;
  overscroll-behavior: contain;

  &_name {
    @extend %text_button;
    display: flex;
    align-items: center;
    gap: $space_2;
    max-width: $app_tab_max_width;
    text-align: left;

    span {
      @extend %truncated_text;
    }
  }

  &_path {
    @extend %truncated_text;
    display: block;
    max-width: $app_tab_max_width;
  }

  &_size {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
}

.search_state {
  @extend %stack;
  min-height: 100%;
  align-items: center;
  justify-content: center;
  padding: $space_6;
  gap: $space_3;
  text-align: center;

  p {
    @extend %muted;
    max-width: $modal_width;
  }

  &_icon {
    @extend %fx_center;
    width: $space_10 + $space_2;
    height: $space_10 + $space_2;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--surface-hover);
  }
}

</style>
