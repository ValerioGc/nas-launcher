<script lang="ts" setup>

import { computed } from 'vue';
import { t } from '@/i18n';
import { sectionLabels } from '@/apps/nas-system-settings/search';
import type { SettingsSection } from '@/apps/nas-system-settings/types/search';
import type { AppOrigin, FileType, SearchScope } from '@/apps/nas-global-search/types';
import filterOffIcon from '@/assets/icons/navigation/filter_off.svg';

import CommonIcon from '@/components/commons/CommonIcon.vue';

interface Choice<T> {
  value: T;
  label: string;
}

/**
 * Defines the available file type filters for the search component.
 */
const types: Choice<FileType | ''>[] = [
  { value: '', label: 'allTypes' },
  { value: 'folder', label: 'folders' },
  { value: 'document', label: 'documents' },
  { value: 'image', label: 'images' },
  { value: 'audio', label: 'audio' },
  { value: 'video', label: 'video' },
  { value: 'archive', label: 'archives' },
];

const origins: Choice<AppOrigin>[] = [
  { value: '', label: 'allApps' },
  { value: 'system', label: 'systemApp' },
  { value: 'installed', label: 'installed' },
];

const sections: Choice<SettingsSection | ''>[] = [
  { value: '', label: 'allSections' },
  ...Object.entries(sectionLabels).map(([value, label]) => ({
    value: value as SettingsSection,
    label,
  })),
];

defineProps<{ scope: SearchScope }>();

const fileType = defineModel<FileType | ''>('fileType', { required: true });
const extension = defineModel<string>('extension', { required: true });
const origin = defineModel<AppOrigin>('origin', { required: true });
const section = defineModel<SettingsSection | ''>('section', { required: true });

const filtered = computed(() =>
  Boolean(fileType.value || extension.value || origin.value || section.value),
);

function resetFilters() {
  fileType.value = '';
  extension.value = '';
  origin.value = '';
  section.value = '';
}

</script>

<template>
  <div class="search_filters">
    <template v-if="scope === 'files'">
      <label>
        <span>{{ t('fileType') }}</span>

        <select v-model="fileType" name="search_type" data-testid="search_type">
          <option v-for="type in types" :key="type.value || 'all'" :value="type.value">
            {{ t(type.label) }}
          </option>
        </select>
      </label>

      <label>
        <span>{{ t('extension') }}</span>
        <input
          v-model="extension"
          name="search_extension"
          type="text"
          inputmode="text"
          autocapitalize="none"
          :spellcheck="false"
          placeholder="pdf"
          data-testid="search_extension"
        />
      </label>
    </template>

    <label v-else-if="scope === 'apps'">
      <span>{{ t('appOrigin') }}</span>

      <select v-model="origin" name="search_origin" data-testid="search_origin">
        <option v-for="item in origins" :key="item.value || 'all'" :value="item.value">
          {{ t(item.label) }}
        </option>
      </select>
    </label>

    <label v-else>
      <span>{{ t('settingsSection') }}</span>

      <select v-model="section" name="search_section" data-testid="search_section">
        <option v-for="item in sections" :key="item.value || 'all'" :value="item.value">
          {{ t(item.label) }}
        </option>
      </select>
    </label>

    <button
      type="button"
      class="search_reset"
      :disabled="!filtered"
      :aria-label="t('resetFilters')"
      :title="t('resetFilters')"
      data-testid="search_reset_filters"
      @click="resetFilters"
    >
      <CommonIcon :src="filterOffIcon" />
    </button>
  </div>
</template>

<style lang="scss" scoped>

.search_filters {
  display: flex;
  align-items: end;
  gap: $space_4;

  label {
    flex: 1;
    flex-direction: column;
    align-items: stretch;
    min-width: 0;
  }
}

.search_reset {
  @extend %icon_button;
  border: $border_thin solid var(--surface-border);
}

</style>
