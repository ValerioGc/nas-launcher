<script setup lang="ts">

import SettingsStatus from './SettingsStatus.vue';

defineOptions({ inheritAttrs: false });

withDefaults(
  defineProps<{
    title: string;
    /** A line under the title on what the page holds. */
    intro?: string;
    as?: 'div' | 'section' | 'form';
    ready?: boolean;
    loading?: boolean;
    error?: string;
  }>(),
  {
    intro: '',
    as: 'div',
    ready: true,
    loading: false,
    error: '',
  },
);

const emit = defineEmits<{
  retry: [];
}>();

</script>

<template>
  <component :is="as" v-if="ready" v-bind="$attrs" class="app_stack settings_form">
    <h1>{{ title }}</h1>
    <p v-if="intro" class="muted settings_intro">{{ intro }}</p>

    <slot></slot>

    <slot name="footer"></slot>
  </component>

  <SettingsStatus
    :loading="!ready && loading"
    :error="error"
    centered
    retryable
    @retry="emit('retry')"
  />
</template>
