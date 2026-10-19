<script setup lang="ts">

import { computed } from 'vue';
import { t, formatDate } from '@/i18n';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { useNotificationsStore } from '../store';
import { hasLog, noticeTitle, sourceLabel } from '../text';
import type { AppNotification } from '../types';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import NoticeMessage from './NoticeMessage.vue';

import deleteIcon from '@/assets/icons/navigation/delete.svg';
import checkIcon from '@/assets/icons/navigation/check.svg';
import searchIcon from '@/assets/icons/navigation/search.svg';
import infoIcon from '@/assets/icons/status/info.svg';
import successIcon from '@/assets/icons/status/success.svg';
import warningIcon from '@/assets/icons/status/warning.svg';
import errorIcon from '@/assets/icons/status/error.svg';

const notifications = useNotificationsStore();
const apps = useAppsStore();
const shell = useShellStore();

const LOG_MANAGER = 'nas-log-manager';
const logManager = computed(() => apps.installed.some((app) => app.id === LOG_MANAGER));

function openLog(item: AppNotification) {
  shell.open(LOG_MANAGER, { location: `?notification=${encodeURIComponent(item.id)}` });
}
const toneIcons: Record<AppNotification['tone'], string> = {
  info: infoIcon,
  success: successIcon,
  warning: warningIcon,
  error: errorIcon,
};

</script>

<template>
  <ul class="app_scroll notifications_list">
    <li
      v-for="item in notifications.visible"
      :key="item.id"
      class="app_panel notification_row"
      :class="[`notification_${item.tone}`, { notification_unread: !item.read }]"
      :data-testid="`notification_${item.id}`"
    >
      <CommonIcon class="notification_tone" :src="toneIcons[item.tone]" :label="t(item.tone)" />

      <div class="notification_body">
        <div class="notification_header">
          <strong>{{ noticeTitle(item) }}</strong>
          <p class="muted notification_meta">
            <button
              type="button"
              class="notification_source"
              :title="t('filterBySource', { source: sourceLabel(item.source) })"
              :data-testid="`notification_source_${item.id}`"
              @click="notifications.setSource(item.source)"
            >
              {{ sourceLabel(item.source) }}
            </button>
            · {{ formatDate(item.at) }}
          </p>
        </div>

        <div class="notification_content">
          <NoticeMessage :item="item" />

          <div class="app_toolbar notification_actions">
            <button
              v-if="hasLog(item)"
              type="button"
              :disabled="!logManager"
              :aria-label="`${t('openLog')} ${noticeTitle(item)}`"
              :title="logManager ? t('openLog') : t('openLogUnavailable')"
              :data-testid="`notification_log_${item.id}`"
              @click="openLog(item)"
            >
              <CommonIcon :src="searchIcon" />
            </button>
            <button
              v-if="!item.read"
              type="button"
              :disabled="notifications.busy"
              :aria-label="`${t('read')} ${noticeTitle(item)}`"
              :title="t('read')"
              @click="notifications.change(`/notifications/${item.id}`, 'PATCH', { read: true })"
            >
              <CommonIcon :src="checkIcon" />
            </button>
            <button
              type="button"
              :disabled="notifications.busy"
              :aria-label="`${t('remove')} ${noticeTitle(item)}`"
              :title="t('remove')"
              @click="notifications.change(`/notifications/${item.id}`, 'DELETE')"
            >
              <CommonIcon :src="deleteIcon" />
            </button>
          </div>
        </div>
      </div>
    </li>
  </ul>
</template>

<style scoped lang="scss">

.notifications_list {
  @extend %stack;
  @include minimal-scrollbar;
  gap: $space_3;
  flex: 1;
  min-height: $space_10;
  padding: $border_thin;
  overscroll-behavior: contain;
}

.notification_row {
  @extend %fx_base;
  align-items: start;
  gap: $space_3;
  padding: $space_3;
  border-left: $space_2 solid var(--surface-border);
}

.notification_unread {
  background: var(--surface-hover);

  strong {
    font-weight: 700;
  }
}

.notification_info {
  border-left-color: var(--status-info);
}

.notification_success {
  border-left-color: var(--status-success);
}

.notification_warning {
  border-left-color: var(--status-warning);
}

.notification_error {
  border-left-color: var(--status-error);
}

.notification_tone {
  margin-top: $space_2;
  flex-shrink: 0;
}

.notification_body {
  @extend %stack;
  gap: $space_2;
  flex: 1;
  min-width: 0;

  p {
    margin: 0;
    overflow-wrap: anywhere;
  }
}

.notification_header {
  @extend %fx_base;
  align-items: start;
  gap: $space_3;

  strong {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
}

.notification_meta {
  @extend %fx_base;
  flex-shrink: 0;
  gap: $space_2;
  font-size: $text_label;
  white-space: nowrap;
}

.notification_source {
  @extend %text_button;
  text-decoration: underline dotted;

  &:hover:not(:disabled) {
    background: transparent;
    color: var(--accent);
  }
}

.notification_content {
  @extend %fx_base;
  align-items: end;
  gap: $space_3;

  p {
    flex: 1;
  }
}

.notification_actions {
  flex-shrink: 0;
  margin-left: auto;
}

</style>
