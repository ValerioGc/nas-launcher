<script setup lang="ts">

defineProps<{
  name: string;
  icon: string;
}>();

const emit = defineEmits<{
  open: [];
  context: [event: MouseEvent | KeyboardEvent];
  move: [step: number];
}>();

</script>

<template>
  <li>
    <button
      type="button"
      class="desktop_item"
      aria-keyshortcuts="Alt+ArrowLeft Alt+ArrowRight"
      @click="emit('open')"
      @contextmenu.prevent="emit('context', $event)"
      @keydown.shift.f10.prevent="emit('context', $event)"
      @keydown.alt.left.prevent="emit('move', -1)"
      @keydown.alt.right.prevent="emit('move', 1)"
    >
      <img :src="icon" alt="" draggable="false" />
      <span>{{ name }}</span>
    </button>
  </li>
</template>

<style scoped lang="scss">

.desktop_item {
  @extend %icon_button;
  width: 100%;
  flex-direction: column;
  text-align: center;
  user-select: none;

  img {
    width: var(--desktop-icon, #{$desktop_icon_size});
    height: var(--desktop-icon, #{$desktop_icon_size});
  }

  span {
    color: var(--desktop-ink, var(--theme-foreground));
    text-shadow: var(--desktop-ink-shadow, none);
    font-size: $text_label;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
}

</style>
