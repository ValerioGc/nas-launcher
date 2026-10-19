<script setup lang="ts">

import { computed, ref } from 'vue';
import { t, formatDate } from '@/i18n';
import { useNotificationsStore } from '../store';
import { noticeTitle } from '../text';
import { useShellStore } from '@/stores/shellStore';

import deleteIcon from '@/assets/icons/navigation/delete.svg';
import enterIcon from '@/assets/icons/navigation/enter.svg';
import bell from '@/assets/icons/system/bell.svg';

import CommonDropdown from '@/components/commons/CommonDropdown.vue';
import CommonEmpty from '@/components/commons/CommonEmpty.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';

const SHOWN = 5;

const notifications = useNotificationsStore();
const shell = useShellStore();
const open = ref(false);

const hidden = computed(() => Math.max(0, notifications.items.length - SHOWN));

function showAll() {
  open.value = false;
  shell.open('nas-notification-manager');
}

</script>

<template>
  <CommonDropdown
    v-model="open"
    :icon="bell"
    :trigger-label="t('notifications')"
    :label="notifications.unread ? String(notifications.unread) : undefined"
    align="end"
    width="wide"
  >
    <li v-if="notifications.error" role="presentation">
      <p role="alert">{{ notifications.error }}</p>
      <button type="button" @click="notifications.refresh()">
        {{ t('retry') }}
      </button>
    </li>

    <li
      v-for="item in notifications.items.slice(0, SHOWN)"
      :key="item.id"
      class="notification_row"
      role="presentation"
      :data-testid="`notification_menu_${item.id}`"
    >
      <button type="button" class="notification_item" role="menuitem" @click="showAll">
        <strong>{{ noticeTitle(item) }}</strong>
        <small>{{ formatDate(item.at) }}</small>
      </button>
      <button
        type="button"
        class="notification_remove"
        role="menuitem"
        :title="t('remove')"
        :aria-label="`${t('remove')} ${noticeTitle(item)}`"
        :disabled="notifications.removing"
        @click="notifications.remove(item.id)"
      >
        <CommonIcon :src="deleteIcon" />
      </button>
    </li>

    <li v-if="!notifications.items.length && !notifications.error" role="presentation">
      <CommonEmpty
        :title="t('noUnreadNotifications')"
        :hint="t('noUnreadNotificationsHint')"
        :icon="bell"
        testid="notification_empty"
      />
    </li>

    <li class="notification_footer" role="presentation">
      <button
        type="button"
        class="notification_all"
        role="menuitem"
        data-testid="notification_open_all"
        @click="showAll"
      >
        {{ t('openNotifications') }}
        <span v-if="hidden" class="notification_hidden">+{{ hidden }}</span>
        <CommonIcon :src="enterIcon" size="small" tinted />
      </button>
    </li>
  </CommonDropdown>
</template>

<style scoped lang="scss">

.notification_row {
  position: relative;

  &:hover .notification_item {
    background: var(--surface-hover);
  }
}

.notification_item {
  @extend %button;
  width: 100%;
  align-items: start;
  flex-direction: column;
  padding-right: $space_10 + $space_2;
  border: 0;
  text-align: left;

  strong {
    overflow-wrap: anywhere;
  }

  small {
    @extend %muted;
  }
}

.notification_remove {
  @extend %icon_button;
  position: absolute;
  top: 50%;
  right: $space_2;
  transform: translateY(-50%);

  &:hover:not(:disabled) {
    border-color: var(--surface-border);
    background: var(--surface);
  }
}

.notification_footer {
  margin-top: $space_2;
  padding-top: $space_2;
  border-top: $border_thin solid var(--surface-border);
}

.notification_all {
  @extend %button;
  width: 100%;
  border: 0;
  color: var(--accent);
  font-weight: 600;
}

.notification_hidden {
  @extend %badge;
  border-color: var(--accent);
  color: var(--accent);
}

</style>
