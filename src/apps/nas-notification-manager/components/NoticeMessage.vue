<script setup lang="ts">

import { computed } from 'vue';
import { useShellStore } from '@/stores/shellStore';
import { noticeMessage, noticeParts } from '../text';
import type { AppNotification } from '../types';

interface Props {
  item: AppNotification;
}

const props = defineProps<Props>();
const emit = defineEmits<{ open: [] }>();

const parts = computed(() => noticeParts(props.item));

/** Opens the place the message names, on the page it is about. */
function open() {
  const target = parts.value?.target;
  if (target) {
    useShellStore().open(target.app, { location: target.location });
    emit('open');
  }
}

</script>

<template>
  <p v-if="parts">
    {{ parts.before }}
    <a
      class="notice_link"
      :href="`#${parts.target.app}`"
      :data-testid="`notice_link_${props.item.id}`"
      @click.prevent="open"
      >{{ parts.link }}</a
    >{{ parts.after }}
  </p>

  <p v-else>{{ noticeMessage(props.item) }}</p>
</template>

<style scoped lang="scss">

.notice_link {
  color: var(--accent);
  text-decoration: underline;
  cursor: pointer;

  &:focus-visible {
    @include focus-ring;
  }
}

</style>
