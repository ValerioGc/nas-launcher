<script setup lang="ts">

import { t } from '@/i18n';
import { useNotificationsStore } from '../store';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';

import deleteIcon from '@/assets/icons/navigation/delete.svg';
import checkIcon from '@/assets/icons/navigation/check.svg';
import filterOffIcon from '@/assets/icons/navigation/filter_off.svg';
import refreshIcon from '@/assets/icons/system/refresh.svg';

const notifications = useNotificationsStore();

</script>

<template>
  <header class="notifications_header">
    <div class="app_toolbar notifications_heading">
      <h1>{{ t('notifications') }}</h1>
      <span v-if="notifications.managerUnread" class="notifications_count">
        {{ notifications.managerUnread }}
      </span>
      <button
        type="button"
        :disabled="notifications.busy || !notifications.managerUnread"
        data-testid="notifications_read_all"
        @click="notifications.change('/notifications/read')"
      >
        <CommonIcon :src="checkIcon" size="small" />{{ t('markAllRead') }}
      </button>
      <button
        type="button"
        :disabled="notifications.busy || !notifications.data.length"
        data-testid="notifications_clear_read"
        @click="notifications.change('/notifications/read/clear')"
      >
        <CommonIcon :src="deleteIcon" size="small" />{{ t('clearRead') }}
      </button>
      <button
        type="button"
        :disabled="notifications.loading || !notifications.hasNotifications"
        :aria-label="t('refresh')"
        :title="t('refresh')"
        data-testid="notifications_refresh"
        @click="notifications.load"
      >
        <CommonIcon :src="refreshIcon" />
      </button>
    </div>

    <div class="app_toolbar notifications_search" data-testid="notifications_filters">
      <input
        v-model="notifications.query"
        name="notification_query"
        :disabled="!notifications.hasNotifications"
        type="search"
        maxlength="200"
        :aria-label="t('search')"
        :placeholder="t('searchNotifications')"
        data-testid="notifications_query"
      />
      <CommonSwitch
        v-model="notifications.unreadOnly"
        :label="t('unread')"
        :disabled="!notifications.hasNotifications"
        testid="notifications_unread"
      />
      <button
        type="button"
        :disabled="!notifications.filtered"
        :aria-label="t('resetFilters')"
        :title="t('resetFilters')"
        data-testid="notifications_reset"
        @click="notifications.reset"
      >
        <CommonIcon :src="filterOffIcon" />
      </button>
    </div>
  </header>
</template>

<style scoped lang="scss">

.notifications_header {
  @extend %stack;
  gap: $space_3;
}

.notifications_heading h1 {
  margin-right: auto;
}

.notifications_count {
  @extend %badge;
  border-color: var(--accent);
  background: var(--accent);
  color: var(--surface);
}

.notifications_search {
  flex-wrap: wrap;

  input {
    flex: 1;
    min-width: $metric_min_width * 2;
  }
}

</style>
