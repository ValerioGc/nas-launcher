<script setup lang="ts">

import { onBeforeUnmount, ref, watch } from 'vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';

interface Props {
  icon: string;
  triggerLabel: string;
  label?: string;
  align?: 'start' | 'end';
  width?: 'default' | 'wide';
}

const props = withDefaults(defineProps<Props>(), {
  align: 'start',
  label: undefined,
  width: 'default',
});

const open = defineModel<boolean>({ default: false });
const root = ref<HTMLElement | null>(null);

function close() {
  open.value = false;
}

function toggle() {
  open.value = !open.value;
}

function onPointerDown(event: PointerEvent) {
  if (root.value && !root.value.contains(event.target as Node)) {
    close();
  }
}

watch(open, (isOpen) => {
  if (isOpen) {
    document.addEventListener('pointerdown', onPointerDown);
    return;
  }
  document.removeEventListener('pointerdown', onPointerDown);
});

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onPointerDown);
});

</script>

<template>
  <div ref="root" class="common_dropdown" @keydown.esc="close">
    <button
      type="button"
      class="common_dropdown_trigger"
      :class="{ common_dropdown_trigger_active: open }"
      :aria-expanded="open"
      aria-haspopup="menu"
      :aria-label="props.triggerLabel"
      data-testid="common_dropdown_trigger"
      @click="toggle"
    >
      <CommonIcon :src="props.icon" />
      <span v-if="props.label" class="common_dropdown_label">{{ props.label }}</span>
    </button>

    <Transition name="common_dropdown">
      <ul
        v-if="open"
        class="common_dropdown_content"
        :class="{
          common_dropdown_content_start: props.align === 'start',
          common_dropdown_content_end: props.align === 'end',
          common_dropdown_content_wide: props.width === 'wide',
        }"
        role="menu"
        data-testid="common_dropdown_content"
      >
        <slot :close="close"></slot>
      </ul>
    </Transition>
  </div>
</template>

<style scoped lang="scss">

.common_dropdown {
  position: relative;

  .common_dropdown_trigger {
    @extend %icon_button;
    font-size: $text_label;

    &_active {
      background: var(--surface-hover);
    }
  }

  .common_dropdown_content {
    position: absolute;
    top: calc(100% + #{$dropdown_offset});
    z-index: $z_dropdown;
    min-width: min($dropdown_min_width, calc(100vw - #{$space_8}));
    max-width: calc(100vw - #{$space_8});
    max-height: 70svh;
    overflow: auto;
    padding: $space_2;
    border: $border_thin solid var(--surface-border);
    border-radius: $radius_control;
    background: var(--surface);
    box-shadow: $shadow_raised;

    &.common_dropdown_content_wide {
      min-width: min($dropdown_wide_width, calc(100vw - #{$space_8}));
    }

    &_start {
      left: 0;
    }

    &_end {
      right: 0;
    }
  }

  // Dropdown transition styles **************
  .common_dropdown-enter-active,
  .common_dropdown-leave-active {
    transition:
      opacity $duration_pop ease,
      transform $duration_pop ease;
  }

  .common_dropdown-enter-from,
  .common_dropdown-leave-to {
    opacity: 0;
    transform: translateY(-#{$space_2});
  }
}

@media (prefers-reduced-motion: reduce) {
  .common_dropdown-enter-from,
  .common_dropdown-leave-to {
    transform: none;
  }
}

</style>
