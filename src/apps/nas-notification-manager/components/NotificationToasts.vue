<script setup lang="ts">

import { t } from '@/i18n';
import { useNotificationsStore } from '../store';
import type { Toast } from '../types';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import NoticeMessage from './NoticeMessage.vue';

const notifications = useNotificationsStore();

function act(toast: Toast) {
  notifications.dismiss(toast.id);
  toast.action?.run();
}

</script>

<template>
  <TransitionGroup
    tag="ol"
    name="notification_toast"
    class="notification_toasts app_surface"
    :aria-label="t('notifications')"
    data-testid="notification_toasts"
  >
    <li
      v-for="toast in notifications.toasts"
      :key="toast.id"
      class="notification_toast"
      :data-testid="`notification_toast_${toast.id}`"
    >
      <AlertBanner
        :tone="toast.tone"
        :title="toast.title"
        dismissible
        @dismiss="notifications.dismiss(toast.id)"
      >
        <NoticeMessage
          v-if="toast.notification"
          :item="toast.notification"
          @open="notifications.dismiss(toast.id)"
        />
        <p v-else-if="toast.message">{{ toast.message }}</p>
        <button v-if="toast.action" type="button" @click="act(toast)">
          {{ toast.action.label }}
        </button>
      </AlertBanner>

      <span
        v-if="toast.duration"
        class="notification_toast_timer"
        :class="`notification_toast_timer_${toast.tone}`"
        :style="{ animationDuration: `${toast.duration}ms` }"
        :data-testid="`notification_toast_timer_${toast.id}`"
        @animationend="notifications.dismiss(toast.id)"
      ></span>
    </li>
  </TransitionGroup>
</template>

<style scoped lang="scss">

.notification_toasts {
  position: fixed;
  right: $space_4;
  bottom: $space_4;
  z-index: $z_toast;
  display: flex;
  flex-direction: column;
  gap: $space_2;
  width: min($toast_width, calc(100vw - #{$space_8}));
  pointer-events: none;
}

.notification_toast {
  position: relative;
  overflow: hidden;
  border-radius: $radius_control;
  background: var(--surface);
  box-shadow: $shadow_overlay;
  pointer-events: auto;

  &:hover,
  &:focus-within {
    .notification_toast_timer {
      animation-play-state: paused;
    }
  }

  p {
    margin: 0 0 $space_2;
    overflow-wrap: anywhere;
  }
}

.notification_toast_timer {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: $toast_timer_height;
  transform-origin: left;
  animation: notification_toast_time linear forwards;

  &_info {
    background: var(--status-info);
  }

  &_success {
    background: var(--status-success);
  }

  &_warning {
    background: var(--status-warning);
  }

  &_error {
    background: var(--status-error);
  }
}

@keyframes notification_toast_time {
  to {
    transform: scaleX(0);
  }
}

.notification_toast-enter-active,
.notification_toast-leave-active {
  transition:
    opacity $duration_fast ease-out,
    transform $duration_fast ease-out;

  @media (prefers-reduced-motion: reduce) {
    transition: none;
  }
}

.notification_toast-enter-from,
.notification_toast-leave-to {
  opacity: 0;
  transform: translateX($space_4);
}

</style>
