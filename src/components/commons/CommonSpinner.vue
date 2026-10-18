<script setup lang="ts">

import { t } from '@/i18n';

interface Props {
  label?: string;
  compact?: boolean;
  size?: 'small' | 'medium' | 'large' | 'huge';
  centered?: boolean;
  testid?: string;
}

const props = withDefaults(defineProps<Props>(), {
  label: undefined,
  compact: false,
  size: 'medium',
  centered: false,
  testid: undefined,
});

</script>

<template>
  <output
    class="common_spinner"
    :class="{ common_spinner_centered: props.centered }"
    aria-live="polite"
    :data-testid="props.testid"
  >
    <span class="common_spinner_ring" :class="`common_spinner_${props.size}`"></span>

    <span :class="{ visually_hidden: props.compact }">{{ props.label ?? t('loading') }}</span>
  </output>
</template>

<style scoped lang="scss">

.common_spinner {
  @extend %fx_base;
  gap: $space_2;
  color: var(--surface-muted);
  font-size: $text_label;

  &_centered {
    @extend %full_wh;
    flex-direction: column;
    justify-content: center;
    gap: $space_3;
    animation: spinner_appear $duration_pop ease-out both;
  }

  &_ring {
    flex-shrink: 0;
    aspect-ratio: 1;
    border: $spinner_stroke solid var(--surface-border);
    border-top-color: var(--accent);
    border-radius: 50%;
    @include spinning;
  }

  &_small {
    width: $icon_small;
  }

  &_medium {
    width: $icon_medium;
  }

  &_large {
    width: $spinner_large;
  }

  &_huge {
    width: $spinner_huge;
    border-width: $spinner_stroke_huge;
  }
}

@keyframes spinner_appear {
  from {
    opacity: 0;
  }
}

</style>
