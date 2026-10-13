<script setup lang="ts">

interface Props {
  width?: 'short' | 'medium' | 'long' | 'icon';
}

withDefaults(defineProps<Props>(), { width: 'medium' });

</script>

<template>
  <span
    class="common_skeleton"
    :class="`common_skeleton_${width}`"
    aria-hidden="true"
    data-testid="common_skeleton"
  ></span>
</template>

<style scoped lang="scss">

.common_skeleton {
  display: inline-block;
  height: 1em;
  max-width: 100%;
  border-radius: $radius_control;
  background: var(--surface-hover);
  vertical-align: middle;
  animation: skeleton_pulse $duration_waiting ease-in-out infinite;

  @media (prefers-reduced-motion: reduce) {
    animation: none;
  }

  &_short {
    width: $skeleton_short;
  }

  &_medium {
    width: $skeleton_medium;
  }

  &_long {
    width: $skeleton_long;
  }

  &_icon {
    width: $icon_large;
    height: $icon_large;
  }
}

@keyframes skeleton_pulse {
  50% {
    opacity: 0.45;
  }
}

</style>
