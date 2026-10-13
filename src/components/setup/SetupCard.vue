<script setup lang="ts">

import logoSource from '@/assets/logo.svg';

interface Props {
  title: string;
  description?: string;
  logo?: boolean;
}

const props = withDefaults(defineProps<Props>(), { description: undefined, logo: false });

</script>

<template>
  <section
    class="app_panel setup_card"
    :class="{ setup_card_intro: props.logo }"
    :aria-label="props.title"
    data-testid="setup_card"
  >
    <header class="setup_card_header">
      <img v-if="props.logo" class="setup_card_logo" :src="logoSource" alt="" />
      <h1>{{ props.title }}</h1>
      <p v-if="props.description" class="muted">{{ props.description }}</p>
    </header>

    <div class="setup_card_body">
      <slot></slot>
    </div>

    <footer v-if="$slots.actions" class="app_toolbar setup_card_actions">
      <slot name="actions"></slot>
    </footer>
  </section>
</template>

<style scoped lang="scss">

.setup_card {
  @extend %stack;
  gap: $space_5;
  width: 100%;
  max-width: $setup_card_width;
  height: $setup_card_height;
  max-height: calc(100svh - #{$space_4 * 2});
  padding: $space_6;

  h1 {
    margin-bottom: $space_2;
  }
}

.setup_card_header {
  text-align: center;
}

.setup_card_body {
  @extend %stack;
  flex: 0 1 auto;
  gap: $space_5;
  min-height: 0;
  margin: -($focus_width + $focus_offset);
  padding: $focus_width + $focus_offset;
  overflow-y: auto;
  @extend %hidden_scrollbar;
}

.setup_card_intro {
  justify-content: center;
}

.setup_card_logo {
  width: $setup_card_logo_size;
  height: $setup_card_logo_size;
  margin-bottom: $space_4;
}

.setup_card_actions {
  justify-content: center;
}

</style>
