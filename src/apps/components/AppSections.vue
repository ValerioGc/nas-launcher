<script setup lang="ts">

import { computed } from 'vue';
import { t } from '@/i18n';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import type { AppSection } from './types';

const props = defineProps<{
  sections: AppSection[];
  label: string;
}>();

const selected = defineModel<string>('selected');

const active = computed(
  () => props.sections.find((item) => item.id === selected.value) ?? props.sections[0],
);

</script>

<template>
  <div class="app_sections">
    <nav :aria-label="label" class="app_sections_nav">
      <button
        v-for="item in sections"
        :key="item.id"
        type="button"
        :aria-pressed="active?.id === item.id"
        @click="selected = item.id"
      >
        <CommonIcon v-if="item.icon" :src="item.icon" size="small" />{{ t(item.label) }}
      </button>
    </nav>
    <div class="app_sections_content">
      <KeepAlive><component :is="active.component" v-if="active" :key="active.id" /></KeepAlive>
    </div>
  </div>
</template>

<style scoped lang="scss">

.app_sections {
  display: flex;
  gap: $space_5;
  min-height: 100%;

  @media (max-width: $breakpoint_medium) {
    flex-direction: column;
  }

  &_nav {
    @extend %stack;
    gap: $space_2;
    width: $settings_sidebar_width;
    flex-shrink: 0;

    @media (max-width: $breakpoint_medium) {
      flex-direction: row;
      overflow-x: auto;
      width: 100%;
    }

    button {
      justify-content: start;
      text-align: left;

      @media (max-width: $breakpoint_medium) {
        flex-shrink: 0;
      }
    }
  }

  &_content {
    flex: 1;
    min-width: 0;
  }
}

</style>
