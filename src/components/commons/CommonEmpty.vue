<script setup lang="ts">

import CommonIcon from '@/components/commons/CommonIcon.vue';

interface Props {
  title: string;
  hint?: string;
  icon?: string;
  testid?: string;
}

withDefaults(defineProps<Props>(), { hint: undefined, icon: undefined, testid: undefined });

</script>

<template>
  <div class="common_empty" :data-testid="testid">
    <CommonIcon v-if="icon" :src="icon" size="large" tinted />
    <p class="common_empty_title">{{ title }}</p>
    <p v-if="hint" class="common_empty_hint">{{ hint }}</p>
    <slot></slot>
  </div>
</template>

<style scoped lang="scss">

.common_empty {
  @extend %stack;
  align-items: center;
  justify-content: center;
  gap: $space_2;
  height: 100%;
  padding: $space_8 $space_4;
  text-align: center;
  color: var(--surface-muted);

  &_title {
    margin: 0;
    font-size: $text_large;
    color: var(--theme-foreground);
  }

  &_hint {
    margin: 0;
    max-width: $dropdown_min_width * 1.5;
    font-size: $text_label;
  }
}

</style>
