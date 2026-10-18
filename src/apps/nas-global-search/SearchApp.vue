<script lang="ts" setup>

import { t } from '@/i18n';
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { NoSearchProviderError, searchFiles } from '@/apps/nas-global-search/api';
import { appEntries, fileEntry, settingEntries } from '@/apps/nas-global-search/entries';
import { useDebounced } from '@/composables/useDebounced';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { appLabel } from '@/apps/registry';
import type { SettingsSection } from '@/apps/nas-system-settings/types/search';
import type { AppOrigin, FileType, SearchEntry, SearchResult, SearchScope } from './types';

import searchIcon from '@/assets/icons/navigation/search.svg';
import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonModal from '@/components/commons/CommonModal.vue';
import CommonSegmented from '@/components/commons/CommonSegmented.vue';
import SearchFilters from './components/SearchFilters.vue';
import SearchResults from './components/SearchResults.vue';

const open = defineModel<boolean>({ required: true });

const shell = useShellStore();
const apps = useAppsStore();

const emit = defineEmits<{
  open: [entry: SearchEntry];
}>();

const scopes: { value: SearchScope; label: string }[] = [
  { value: 'all', label: 'scopeAll' },
  { value: 'files', label: 'scopeFiles' },
  { value: 'apps', label: 'scopeApps' },
  { value: 'settings', label: 'scopeSettings' },
];

const scope = ref<SearchScope>('all');
const query = ref('');
const fileType = ref<FileType | ''>('');
const extension = ref('');
const origin = ref<AppOrigin>('');
const section = ref<SettingsSection | ''>('');

const results = ref<SearchResult[]>([]);
const failed = ref<string[]>([]);
const loading = ref(false);
const errorMessage = ref('');

const hasQuery = computed(() => query.value.trim().length > 0);
const searchesFiles = computed(() => scope.value === 'all' || scope.value === 'files');

const entries = computed(() => {
  if (!hasQuery.value) {
    return [];
  }
  const wanted = query.value;
  return [
    ...(scope.value === 'all' || scope.value === 'apps'
      ? appEntries(apps.apps, wanted, origin.value)
      : []),
    ...(scope.value === 'all' || scope.value === 'settings'
      ? settingEntries(wanted, section.value)
      : []),
    ...(scope.value === 'all' ? results.value.map(fileEntry) : []),
  ];
});

const table = computed(() => scope.value === 'files');
const listed = computed(() => (table.value ? results.value.length : entries.value.length));
const warning = computed(() => {
  if (scope.value === 'all' && errorMessage.value) {
    return errorMessage.value;
  }
  return failed.value.length
    ? t('searchPartial', { apps: failed.value.map((id) => appLabel(id, id)).join(', ') })
    : '';
});

const DEBOUNCE_MS = 250;
let lastRequest = 0;

function buildParams() {
  const params = new URLSearchParams({ q: query.value.trim() });
  if (scope.value !== 'files') {
    return params;
  }
  if (fileType.value) {
    params.set('type', fileType.value);
  }

  const ext = extension.value.trim().replace(/^\./, '').toLowerCase();
  if (ext) {
    params.set('ext', ext);
  }

  return params;
}

async function runSearch() {
  const request = ++lastRequest;
  errorMessage.value = '';

  try {
    const found = await searchFiles(buildParams());
    if (request !== lastRequest) {
      return;
    }

    results.value = found.results;
    failed.value = found.failed;
  } catch (error) {
    if (request !== lastRequest) {
      return;
    }

    results.value = [];
    failed.value = [];
    if (error instanceof NoSearchProviderError) {
      errorMessage.value = scope.value === 'files' ? t('searchNoProvider') : '';
    } else {
      errorMessage.value = t('searchFailed');
    }
  } finally {
    if (request === lastRequest) {
      loading.value = false;
    }
  }
}

const search = useDebounced(runSearch, DEBOUNCE_MS);

function reset() {
  search.cancel();
  lastRequest += 1;
  results.value = [];
  failed.value = [];
  errorMessage.value = '';
  loading.value = false;
}

/**
 * Searches the files again, after a pause, when the query, the scope or a file filter changes.
 * Applications and settings are found at once, without a request.
 */
watch([query, scope, fileType, extension], () => {
  lastRequest += 1;
  search.cancel();
  if (!hasQuery.value || !searchesFiles.value) {
    reset();
    return;
  }

  errorMessage.value = '';
  loading.value = true;
  search.schedule();
});

watch(open, (isOpen) => {
  if (!isOpen) {
    scope.value = 'all';
    query.value = '';
    fileType.value = '';
    extension.value = '';
    origin.value = '';
    section.value = '';
    reset();
  }
});

function openEntry(entry: SearchEntry) {
  emit('open', entry);
  open.value = false;
  shell.open(entry.app, { query: entry.query, location: entry.location });
}

onMounted(() => {
  if (shell.launchQuery) {
    query.value = shell.launchQuery;
  }
});

onBeforeUnmount(() => {
  lastRequest += 1;
});

</script>

<template>
  <CommonModal v-model="open" :title="t('globalSearch')" :icon="searchIcon" fixed-size>
    <div class="search_app app_surface app_stack">
      <input
        v-model="query"
        name="search_query"
        type="search"
        autocapitalize="none"
        :spellcheck="false"
        :placeholder="t(`searchPlaceholder_${scope}`)"
        :aria-label="t('searchAll')"
        data-testid="search_query"
      />

      <CommonSegmented
        v-model="scope"
        :options="scopes"
        :label="t('searchScope')"
        testid="search_scope"
      />

      <SearchFilters
        v-if="scope !== 'all'"
        v-model:file-type="fileType"
        v-model:extension="extension"
        v-model:origin="origin"
        v-model:section="section"
        :scope="scope"
      />

      <AlertBanner v-if="warning && listed" tone="warning" compact data-testid="search_warning">
        {{ warning }}
      </AlertBanner>

      <SearchResults
        :results="results"
        :entries="entries"
        :table="table"
        :loading="loading && (table || !entries.length)"
        :error-message="scope === 'all' && entries.length ? '' : errorMessage"
        :has-query="hasQuery"
        :scope="scope"
        @open="openEntry"
      />
    </div>
  </CommonModal>
</template>

<style lang="scss" scoped>

.search_app {
  height: 100%;
  min-height: 0;

  > input,
  .common_segmented,
  .search_filters {
    flex-shrink: 0;
  }

  .search_results {
    flex: 1;
    min-height: 0;
  }
}

</style>
