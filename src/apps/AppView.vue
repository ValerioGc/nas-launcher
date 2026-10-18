<script setup lang="ts">

defineProps<{ name?: string; icon?: string }>();

</script>

<template>
  <section class="app_view app_surface" :aria-label="name">
    <slot></slot>
  </section>
</template>

<style scoped lang="scss">

.app_view {
  @extend %full_wh;
  @include minimal-scrollbar;
  padding: $space_5;
  overflow: auto;
  background: var(--app-background);
}

</style>
