<script setup lang="ts">

import { onMounted } from 'vue';
import { t } from '@/i18n';
import { useNotificationsStore } from './store';

import AppView from '@/apps/AppView.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import NotificationToolbar from './components/NotificationToolbar.vue';
import NotificationSidebar from './components/NotificationSidebar.vue';
import NotificationList from './components/NotificationList.vue';
import CommonEmpty from '@/components/commons/CommonEmpty.vue';
import notificationIcon from '@/assets/icons/system/bell.svg';

const notifications = useNotificationsStore();

onMounted(() => {
  notifications.reset();
  notifications.load();
  notifications.loadSources();
});

</script>

<template>
  <AppView :name="t('notifications')">
    <div class="notifications_app">
      <NotificationSidebar />

      <div class="app_stack notifications_main">
        <NotificationToolbar />

        <p v-if="notifications.managerError" class="app_error" role="alert">
          {{ notifications.managerError }}
        </p>
        <CommonSpinner v-if="notifications.loading" />
        <output
          v-else-if="notifications.filtered && notifications.visible.length"
          data-testid="notifications_matches"
        >
          {{ t('matchingNotifications', { count: notifications.visible.length }) }}
        </output>

        <NotificationList v-if="notifications.visible.length" />
        <CommonEmpty
          v-else-if="!notifications.loading"
          class="notifications_empty"
          :icon="notificationIcon"
          :title="t(notifications.filtered ? 'noMatches' : 'noNotifications')"
          :hint="notifications.filtered ? undefined : t('noNotificationsHint')"
          testid="notifications_empty"
        />
      </div>
    </div>
  </AppView>
</template>

<style scoped lang="scss">

.notifications_app {
  display: flex;
  gap: $space_5;
  height: 100%;
  min-height: 0;

  @media (max-width: $breakpoint_medium) {
    flex-direction: column;
  }
}

.notifications_main {
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.notifications_empty {
  flex: 1;
  min-height: 0;
}

</style>
