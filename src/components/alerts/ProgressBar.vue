<script lang="ts" setup>

import { computed } from 'vue';

interface Props {
  value: number | null;
  label?: string;
}

const props = withDefaults(defineProps<Props>(), { label: undefined });

const percent = computed(() => {
  if (props.value === null) {
    return null;
  }

  return Number.isFinite(props.value) ? Math.min(100, Math.max(0, Math.round(props.value))) : 0;
});

</script>

<template>
  <progress
    class="progress_bar"
    :class="{ progress_bar_waiting: percent === null }"
    :value="percent ?? undefined"
    max="100"
    :aria-label="props.label"
    data-testid="progress_bar"
  ></progress>
</template>

<style scoped lang="scss">

.progress_bar {
  appearance: none;
  display: block;
  width: 100%;
  height: $progress_height;
  border: 0;
  border-radius: $progress_height;
  background: var(--surface-hover);
  overflow: hidden;

  &::-webkit-progress-bar {
    background: var(--surface-hover);
  }

  &::-webkit-progress-value {
    border-radius: inherit;
    background: var(--accent);
  }

  &::-moz-progress-bar {
    border-radius: inherit;
    background: var(--accent);
  }
}

.progress_bar_waiting {
  background-color: var(--surface-hover);
  background-image: linear-gradient(var(--accent), var(--accent));
  background-repeat: no-repeat;
  background-size: 40% 100%;
  animation: progress_waiting $duration_waiting ease-in-out infinite;

  &::-webkit-progress-bar {
    background: transparent;
  }

  @media (prefers-reduced-motion: reduce) {
    animation: none;
    background-position: center;
  }
}

@keyframes progress_waiting {
  from {
    background-position: -70% 0;
  }

  to {
    background-position: 170% 0;
  }
}

</style>
