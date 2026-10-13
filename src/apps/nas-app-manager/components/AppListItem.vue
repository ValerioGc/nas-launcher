<script setup lang="ts">

import { computed } from 'vue';
import { storeToRefs } from 'pinia';
import { t } from '@/i18n';
import { resolveApp } from '@/apps/registry';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import type { ManagerRow } from '@/apps/nas-app-manager/types';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';

const props = defineProps<{ app: ManagerRow }>();

const emit = defineEmits<{
  context: [event: MouseEvent | KeyboardEvent];
}>();

const manager = useAppManagerStore();
const { selected, loading, busy, installing } = storeToRefs(manager);
const installs = computed(() => installing.value.includes(props.app.id));
const builtIn = computed(() => resolveApp(props.app.id).source.kind === 'internal');

</script>

<template>
  <button
    type="button"
    class="app_row manager_app"
    :disabled="busy || loading"
    :class="{ manager_app_system: builtIn }"
    :aria-disabled="builtIn || undefined"
    :aria-pressed="selected?.id === app.id"
    :data-testid="`app_manager_app_${app.id}`"
    @click="manager.select(app.id)"
    @contextmenu.prevent="emit('context', $event)"
    @keydown.shift.f10.prevent="emit('context', $event)"
  >
    <span class="manager_icon" :class="{ manager_icon_busy: installs }">
      <img :src="app.icon" alt="" />
      <CommonSpinner
        v-if="installs"
        size="small"
        compact
        :label="t('installing')"
        :testid="`app_manager_installing_${app.id}`"
      />
    </span>
    <span>{{ app.name }}</span>
    <span
      v-if="app.updateAvailable"
      class="manager_badge manager_update_badge"
      :data-testid="`app_manager_badge_${app.id}`"
      >{{ t('updateTo', { version: app.latestVersion ?? '' }) }}</span
    >
  </button>
</template>

<style scoped lang="scss">

.manager_app {
  border: 0;
  background: transparent;
}

.manager_icon {
  position: relative;
  display: inline-flex;
  flex: none;
  border-radius: $radius_control;

  :deep(.common_spinner) {
    position: absolute;
    inset: 0;
    justify-content: center;
    border-radius: inherit;
    background: color-mix(in srgb, var(--surface) 76%, transparent);
  }

  &_busy {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 30%, transparent);
    animation: manager_icon_glow 1.4s ease-in-out infinite;

    &::after {
      position: absolute;
      inset: -4px;
      border: 2px solid transparent;
      border-top-color: var(--accent);
      border-right-color: var(--accent);
      border-radius: 50%;
      content: '';
      animation: manager_icon_turn 0.9s linear infinite;
    }

    img {
      opacity: 0.55;
      transform: scale(0.9);
      transition:
        opacity 180ms ease,
        transform 180ms ease;
    }
  }
}

@keyframes manager_icon_turn {
  to {
    transform: rotate(1turn);
  }
}

@keyframes manager_icon_glow {
  50% {
    box-shadow: 0 0 0 5px color-mix(in srgb, var(--accent) 12%, transparent);
  }
}

@media (prefers-reduced-motion: reduce) {
  .manager_icon_busy,
  .manager_icon_busy::after {
    animation: none;
  }
}

.manager_app_system {
  cursor: default;

  &:hover:not(:disabled),
  &[aria-pressed='true'] {
    background: inherit;
  }
}

.manager_badge {
  @extend %badge;
}

.manager_update_badge {
  border-color: var(--accent);
  color: var(--accent);
}

</style>
