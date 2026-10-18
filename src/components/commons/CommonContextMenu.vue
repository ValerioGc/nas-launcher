<script setup lang="ts">

import { nextTick, onBeforeUnmount, ref, watch } from 'vue';

const props = defineProps<{ x: number; y: number; label: string }>();
const open = defineModel<boolean>({ required: true });
const menu = ref<HTMLElement | null>(null);
const position = ref({ left: '0px', top: '0px' });
let previous: HTMLElement | null = null;

function close() {
  open.value = false;
}

function outside(event: PointerEvent) {
  if (event.target instanceof Node && !menu.value?.contains(event.target)) {
    close();
  }
}

watch(open, async (value) => {
  if (!value) {
    document.removeEventListener('pointerdown', outside);
    previous?.focus();
    return;
  }

  previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  await nextTick();
  position.value = {
    left: `${Math.max(0, Math.min(props.x, window.innerWidth - (menu.value?.offsetWidth ?? 0)))}px`,
    top: `${Math.max(0, Math.min(props.y, window.innerHeight - (menu.value?.offsetHeight ?? 0)))}px`,
  };
  menu.value?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
  document.addEventListener('pointerdown', outside);
});

onBeforeUnmount(() => document.removeEventListener('pointerdown', outside));

</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      ref="menu"
      role="menu"
      class="context_menu"
      :aria-label="label"
      :style="position"
      @keydown.esc.stop="close"
    >
      <slot :close="close"></slot>
    </div>
  </Teleport>
</template>

<style scoped lang="scss">

.context_menu {
  @extend %panel;
  position: fixed;
  z-index: $z_modal;
  padding: $space_2;
  box-shadow: $shadow_raised;

  :deep(button) {
    @extend %button;
  }
}

</style>
