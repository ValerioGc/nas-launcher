<script setup lang="ts" generic="T extends string">

import { t } from '@/i18n';

import CommonIcon from '@/components/commons/CommonIcon.vue';

interface Option {
  value: T;
  label: string;
  icon?: string;
}

interface Props {
  options: Option[];
  label: string;
  testid?: string;
}

withDefaults(defineProps<Props>(), { testid: undefined });

const model = defineModel<T>({ required: true });

</script>

<template>
  <fieldset class="common_segmented" :aria-label="label">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      class="common_segmented_option"
      :aria-pressed="model === option.value"
      :data-testid="testid && `${testid}_${option.value}`"
      @click="model = option.value"
    >
      <CommonIcon v-if="option.icon" :src="option.icon" size="small" tinted />
      {{ t(option.label) }}
    </button>
  </fieldset>
</template>

<style scoped lang="scss">

.common_segmented {
  @extend %fieldset_reset;
  display: flex;
  flex-wrap: wrap;
  gap: $space_2;
  padding: $space_2 * 0.5;
  border: $border_thin solid var(--surface-border);
  border-radius: $radius_control;
  background: var(--surface-sunken);

  &_option {
    @extend %button;
    flex: 1;
    border-color: transparent;
    background: transparent;
    white-space: nowrap;

    &[aria-pressed='true'] {
      border-color: var(--accent);
      background: var(--surface);
      color: var(--accent);
    }
  }
}

</style>
