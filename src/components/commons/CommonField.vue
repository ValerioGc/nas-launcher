<script setup lang="ts">

interface Props {
  error?: string;
}

const props = withDefaults(defineProps<Props>(), { error: '' });

</script>

<template>
  <div class="common_field">
    <slot></slot>
    <span v-if="props.error" class="common_field_error" aria-live="polite">{{ props.error }}</span>
  </div>
</template>

<style scoped lang="scss">

.common_field {
  @extend %stack;
  align-items: stretch;
  gap: $space_2;
}

.common_field_error {
  color: var(--status-error);
  font-size: $text_label;
}

</style>
