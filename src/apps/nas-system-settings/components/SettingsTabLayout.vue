<script setup lang="ts">

import SettingsStatus from './SettingsStatus.vue';

defineOptions({ inheritAttrs: false });

withDefaults(
  defineProps<{
    title: string;
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
  <!-- main content container, rendered as the specified HTML element -->
  <component :is="as" v-if="ready" v-bind="$attrs" class="app_stack settings_form">
    <h1>{{ title }}</h1>
    <p v-if="intro" class="muted settings_intro">{{ intro }}</p>

    <!-- default slot for the main content of the tab -->
    <slot></slot>

    <!-- footer slot for additional content at the bottom of the tab -->
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
