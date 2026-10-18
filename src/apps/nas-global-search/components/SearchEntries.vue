<script lang="ts" setup>

import { computed } from 'vue';
import { t } from '@/i18n';
import type { SearchEntry } from '@/apps/nas-global-search/types';

import CommonIcon from '@/components/commons/CommonIcon.vue';

const props = defineProps<{ entries: SearchEntry[] }>();

const emit = defineEmits<{
  open: [entry: SearchEntry];
}>();

const titles: Record<SearchEntry['kind'], string> = {
  app: 'scopeApps',
  setting: 'scopeSettings',
  file: 'scopeFiles',
};

const groups = computed(() =>
  (Object.keys(titles) as SearchEntry['kind'][])
    .map((kind) => ({ kind, entries: props.entries.filter((entry) => entry.kind === kind) }))
    .filter((group) => group.entries.length),
);

</script>

<template>
  <div class="search_entries">
    <section v-for="group in groups" :key="group.kind" :data-testid="`search_group_${group.kind}`">
      <h3>{{ t(titles[group.kind]) }}</h3>

      <ul>
        <li v-for="entry in group.entries" :key="entry.key">
          <button
            type="button"
            class="search_entry"
            :data-testid="`search_entry_${entry.key}`"
            @click="emit('open', entry)"
          >
            <CommonIcon v-if="entry.icon" :src="entry.icon" />
            <span class="search_entry_text">
              <span class="search_entry_title">{{ entry.title }}</span>
              <span class="search_entry_detail">{{ entry.detail }}</span>
            </span>
          </button>
        </li>
      </ul>
    </section>
  </div>
</template>

<style lang="scss" scoped>

.search_entries {
  @extend %stack;

  h3 {
    @extend %muted;
    margin-bottom: $space_2;
  }

  ul {
    @extend %stack;
    gap: $space_2 * 0.5;
    padding: 0;
    list-style: none;
  }
}

.search_entry {
  @extend %button;
  justify-content: start;
  width: 100%;
  border-color: transparent;
  background: transparent;
  text-align: left;

  &_text {
    @extend %stack;
    gap: 0;
  }

  &_title,
  &_detail {
    @extend %truncated_text;
  }

  &_detail {
    @extend %muted;
  }
}

</style>
