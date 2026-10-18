<script setup lang="ts">

import { nextTick, onBeforeUnmount, ref, watch } from 'vue';

interface Props {
  panelId: string;
  label: string;
  testid: string;
}

const props = defineProps<Props>();

const open = defineModel<boolean>({ default: false });
const panel = ref<HTMLElement | null>(null);
let trigger: HTMLElement | null = null;
let restoreFocus = true;

function onDocumentPointerDown(event: PointerEvent) {
  if (
    event.target instanceof Element &&
    event.target.closest('[aria-controls="process_panel"], [aria-controls="task_panel"]')
  ) {
    restoreFocus = false;
    return;
  }
  if (event.target instanceof Node && !panel.value?.contains(event.target)) {
    restoreFocus = false;
    open.value = false;
  }
}

watch(open, async (visible) => {
  if (visible) {
    restoreFocus = true;
    trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await nextTick();
    panel.value?.focus({ preventScroll: true });
    document.addEventListener('pointerdown', onDocumentPointerDown);
  } else {
    document.removeEventListener('pointerdown', onDocumentPointerDown);
    if (restoreFocus) {
      trigger?.focus({ preventScroll: true });
    }
    trigger = null;
  }
});

onBeforeUnmount(() => document.removeEventListener('pointerdown', onDocumentPointerDown));

</script>

<template>
  <div
    v-if="open"
    class="widget_drawer_backdrop"
    aria-hidden="true"
    data-testid="widget_drawer_backdrop"
    @click="open = false"
  ></div>
  <Transition name="widget_drawer">
    <aside
      v-if="open"
      :id="props.panelId"
      ref="panel"
      class="widget_drawer"
      :aria-label="props.label"
      tabindex="-1"
      :data-testid="props.testid"
      @keydown.esc.stop="open = false"
    >
      <slot></slot>
    </aside>
  </Transition>
</template>

<style scoped lang="scss">

.widget_drawer_backdrop {
  position: absolute;
  inset: 0;
  z-index: $z_widget;
}

.widget_drawer {
  @extend %panel;
  position: absolute;
  inset: 0 0 0 auto;
  z-index: $z_widget + 1;
  width: 100%;
  overflow: auto;
  box-shadow: $shadow_overlay;

  @media (min-width: $breakpoint_desktop) {
    width: $widget_width;
  }

  &:focus-visible {
    @include focus-ring;
    outline-offset: -#{$focus_offset};
  }
}

// Animations **********************************************

.widget_drawer-enter-active {
  transition: transform $duration_menu_in $ease_emphasized;
}

.widget_drawer-leave-active {
  transition: transform $duration_menu_out $ease_emphasized;
}

.widget_drawer-enter-from,
.widget_drawer-leave-to {
  transform: translateX(100%);
}

@media (prefers-reduced-motion: reduce) {
  .widget_drawer-enter-active,
  .widget_drawer-leave-active {
    transition: none;
  }
}

</style>
