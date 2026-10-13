<script setup lang="ts">

import { t } from '@/i18n';
import { appIcon } from '@/apps/registry';
import type { CatalogueEntry } from '@/apps/nas-app-manager/types';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import addIcon from '@/assets/icons/navigation/add.svg';
import deleteIcon from '@/assets/icons/navigation/delete.svg';
import downloadIcon from '@/assets/icons/navigation/download.svg';

interface Props {
  apps: CatalogueEntry[];
}

const props = defineProps<Props>();
const emit = defineEmits<{ add: []; edit: [index: number]; remove: [index: number] }>();

</script>

<template>
  <section class="app_panel app_card" :aria-label="t('catalogue')">
    <header class="app_card_header">
      <CommonIcon :src="downloadIcon" tinted />
      <div class="app_card_title">
        <h3>{{ t('catalogue') }}</h3>
        <p class="muted">{{ t('catalogueNote') }}</p>
      </div>
      <button type="button" data-testid="catalogue_add" @click="emit('add')">
        <CommonIcon :src="addIcon" size="small" />
        {{ t('addApp') }}
      </button>
    </header>

    <p v-if="!props.apps.length" class="muted catalogue_empty">{{ t('catalogueEmpty') }}</p>
    <ul v-else class="app_list">
      <li
        v-for="(entry, index) in props.apps"
        :key="entry.id"
        class="catalogue_row"
        :data-testid="`catalogue_row_${entry.id}`"
      >
        <img :src="appIcon(entry.id, entry.logo)" alt="" />
        <span class="catalogue_name">
          <strong>{{ entry.name }}</strong>
          <span class="muted">{{ entry.repository }}</span>
        </span>
        <button
          type="button"
          :data-testid="`catalogue_edit_${entry.id}`"
          @click="emit('edit', index)"
        >
          {{ t('editApp') }}
        </button>
        <button
          type="button"
          class="app_icon_button"
          :aria-label="`${t('removeApp')} ${entry.name}`"
          :title="t('removeApp')"
          :data-testid="`catalogue_remove_${entry.id}`"
          @click="emit('remove', index)"
        >
          <CommonIcon :src="deleteIcon" size="small" />
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped lang="scss">

.catalogue_row {
  @extend %panel_sunken;
  @extend %fx_base;
  gap: $space_3;
  padding: $space_2 $space_3;

  img {
    width: $space_8;
    height: $space_8;
  }
}

.catalogue_name {
  @extend %stack;
  flex: 1;
  gap: 0;
  min-width: 0;

  span {
    @extend %truncated_text;
    font-size: $text_label;
  }
}

.catalogue_empty {
  margin: 0;
  text-align: center;
}

</style>
