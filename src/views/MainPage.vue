<script setup lang="ts">

import { t } from '@/i18n';
import { computed, onMounted, onBeforeUnmount, provide, ref, watch } from 'vue';

import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import { backgroundStyle } from '@/apps/nas-system-settings/background';
import { useNotificationsStore } from '@/apps/nas-notification-manager/store';
import { useAppearance } from '@/composables/useAppearance';
import { usePolling } from '@/composables/usePolling';
import { APP_BRIDGE, useAppBridge } from '@/composables/useAppBridge';
import { SYSTEM_MONITOR } from '@/api/systemMonitor';

import HeaderBar from '@/components/HeaderBar.vue';
import MonitorDrawer from '@/widgets/monitor/MonitorDrawer.vue';
import TaskDrawer from '@/widgets/tasks/TaskDrawer.vue';
import DesktopGrid from '@/components/DesktopGrid.vue';
import AppMenu from '@/components/header/AppMenu.vue';
import AppHost from '@/apps/AppHost.vue';
import AppTabs from '@/apps/AppTabs.vue';
import NotificationToasts from '@/apps/nas-notification-manager/components/NotificationToasts.vue';

const apps = useAppsStore();
const shell = useShellStore();
const settings = useSettingsStore();
const notifications = useNotificationsStore();
const menuVisible = ref(false);

const openWidget = ref<'processes' | 'tasks' | null>(null);

function widgetModel(name: 'processes' | 'tasks') {
  return computed({
    get: () => openWidget.value === name,
    set: (open: boolean) => {
      openWidget.value = open ? name : null;
    },
  });
}

const processesVisible = widgetModel('processes');

// The windows of the applications and the widget of the processes speak through one bridge.
provide(APP_BRIDGE, useAppBridge());

// The processes widget is drawn by nas-system-monitor: without it there is none.
const monitorInstalled = computed(() => apps.apps.some((app) => app.id === SYSTEM_MONITOR));
watch(monitorInstalled, (installed) => {
  if (!installed && openWidget.value === 'processes') {
    openWidget.value = null;
  }
});
const tasksVisible = widgetModel('tasks');

watch([() => shell.activeId, () => shell.modalId, menuVisible], () => {
  openWidget.value = null;
});

useAppearance();
const background = computed(() =>
  settings.general ? backgroundStyle(settings.general.background) : {},
);

/**
 * Reports a failure by displaying a notification with an optional retry action.
 * @param id The unique identifier for the notification.
 * @param message A reactive function that returns the error message.
 * @param title The title of the notification.
 * @param retry A function to retry the failed action.
 */
function reportFailure(id: string, message: () => string, title: string, retry: () => unknown) {
  watch(message, (text) => {
    if (text) {
      notifications.toast({
        id,
        tone: 'error',
        title: t(title),
        message: text,
        action: { label: t('retry'), run: retry },
      });
    } else {
      notifications.dismiss(id);
    }
  });
}

reportFailure('apps', () => apps.errorMessage, 'appsUnavailable', apps.fetchInstalled);
reportFailure('settings', () => settings.error, 'settingsUnavailable', settings.load);
reportFailure(
  'app-close',
  () => shell.closeError,
  'appCloseFailed',
  () => {
    if (shell.failedCloseId) {
      return shell.close(shell.failedCloseId);
    }
  },
);

const refreshNotifications = () => {
  notifications.refresh();
};

usePolling(notifications.refresh, ref(15000));

onMounted(() => {
  apps.fetchInstalled();
  settings.load();
  notifications.refresh();
  window.addEventListener('nas:mutation', refreshNotifications);
});

onBeforeUnmount(() => {
  window.removeEventListener('nas:mutation', refreshNotifications);
});

</script>

<template>
  <div class="main_page">
    <HeaderBar
      :menu-open="menuVisible"
      :processes-open="processesVisible"
      :processes-available="monitorInstalled"
      :tasks-open="tasksVisible"
      :app-open="Boolean(shell.activeId)"
      @toggle-processes="processesVisible = !processesVisible"
      @toggle-tasks="tasksVisible = !tasksVisible"
      @show-desktop="shell.showDesktop()"
      @toggle-menu="menuVisible = !menuVisible"
      @search="shell.open('nas-global-search', { query: $event })"
    />

    <AppMenu
      v-model="menuVisible"
      @select="shell.open($event.id)"
      @open-settings="shell.open('nas-system-settings')"
    />

    <div class="main_content">
      <div class="desktop_background" :style="background"></div>

      <AppTabs />

      <div class="workspace">
        <DesktopGrid v-show="!shell.activeId" />
        <AppHost />
      </div>

      <MonitorDrawer v-if="monitorInstalled" v-model="processesVisible" />
      <TaskDrawer v-model="tasksVisible" />
    </div>

    <NotificationToasts />
  </div>
</template>

<style scoped lang="scss">

.main_page {
  display: flex;
  flex-direction: column;
  height: 100vh;
  height: 100svh;
  overflow: hidden;
}

.main_content {
  position: relative;
  isolation: isolate;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  overflow: clip;
}

.workspace {
  position: relative;
  flex: 1;
  min-height: 0;
}

.desktop_background {
  position: absolute;
  inset: 0;
  z-index: -1;
  pointer-events: none;
}

</style>
