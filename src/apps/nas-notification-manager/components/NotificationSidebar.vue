<script setup lang="ts">

import { t } from '@/i18n';
import { useNotificationsStore } from '../store';
import { sourceLabel } from '../text';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import bellIcon from '@/assets/icons/system/bell.svg';
import infoIcon from '@/assets/icons/status/info.svg';
import successIcon from '@/assets/icons/status/success.svg';
import warningIcon from '@/assets/icons/status/warning.svg';
import errorIcon from '@/assets/icons/status/error.svg';

const notifications = useNotificationsStore();

const LEVELS = [
  { tone: '', label: 'all', icon: bellIcon },
  { tone: 'info', label: 'info', icon: infoIcon },
  { tone: 'success', label: 'success', icon: successIcon },
  { tone: 'warning', label: 'warning', icon: warningIcon },
  { tone: 'error', label: 'error', icon: errorIcon },
] as const;

</script>

<template>
  <aside class="notifications_sidebar" :aria-label="t('filters')">
    <nav class="notifications_levels" :aria-label="t('level')">
      <button
        v-for="level in LEVELS"
        :key="level.label"
        type="button"
        :aria-pressed="notifications.tone === level.tone"
        :disabled="!notifications.hasNotifications"
        :data-testid="`notifications_tone_${level.label}`"
        @click="notifications.tone = level.tone"
      >
        <CommonIcon :src="level.icon" size="small" />
        {{ t(level.label) }}
      </button>
    </nav>

    <label class="notifications_origin">
      <span class="muted">{{ t('source') }}</span>
      <select
        v-model="notifications.source"
        name="notification_source"
        :disabled="!notifications.hasNotifications"
        data-testid="notifications_source"
      >
        <option value="">{{ t('allSources') }}</option>
        <option v-for="value in notifications.sourceOptions" :key="value" :value="value">
          {{ sourceLabel(value) }}
        </option>
      </select>
    </label>
  </aside>
</template>

<style scoped lang="scss">

.notifications_sidebar {
  @extend %stack;
  gap: $space_6;
  width: $settings_sidebar_width;
  flex-shrink: 0;

  @media (max-width: $breakpoint_medium) {
    width: 100%;
  }
}

.notifications_levels {
  @extend %stack;
  gap: $space_2;

  @media (max-width: $breakpoint_medium) {
    flex-direction: row;
    overflow-x: auto;
  }

  button {
    justify-content: start;
    text-align: left;

    @media (max-width: $breakpoint_medium) {
      flex-shrink: 0;
    }
  }
}

.notifications_origin {
  flex-direction: column;
  align-items: stretch;
  gap: $space_2;

  select {
    width: 100%;
  }
}

</style>
