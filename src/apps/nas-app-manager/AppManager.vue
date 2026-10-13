<script setup lang="ts">

import { computed, onMounted, ref, watch } from 'vue';
import { t } from '@/i18n';
import { useAppManagerStore } from './store';
import { useShellStore } from '@/stores/shellStore';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import AppView from '@/apps/AppView.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';
import AppForge from './components/AppForge.vue';
import ActionButtons from './components/ActionButtons.vue';
import AppList from './components/AppList.vue';
import ManagerSettings from './components/modals/ManagerSettings.vue';

import listIcon from '@/assets/icons/navigation/view_list.svg';
import gridIcon from '@/assets/icons/navigation/view_grid.svg';
import refreshIcon from '@/assets/icons/system/refresh.svg';
import settingsIcon from '@/assets/icons/system/settings.svg';

const manager = useAppManagerStore();
const shell = useShellStore();
const showSystem = computed({
  get: () => manager.showSystem,
  set: manager.setShowSystem,
});
const settingsOpen = ref(false);
const empty = computed(() => !manager.listed.length);
watch(
  () => shell.activeId,
  (id) => {
    if (
      id === 'nas-app-manager' &&
      manager.endpoint === 'installed' &&
      !manager.busy &&
      !manager.loading
    ) {
      void manager.load();
    }
  },
);

onMounted(() => {
  void manager.initialize();
});

</script>

<template>
  <AppView :name="t('appManager')">
    <div class="app_stack manager_content">
      <h1>{{ t('appManager') }}</h1>

      <div class="app_toolbar manager_navigation">
        <div class="app_toolbar">
          <button
            v-for="value in ['installed', 'available'] as const"
            :key="value"
            type="button"
            :aria-pressed="manager.endpoint === value"
            :disabled="manager.busy"
            @click="manager.setEndpoint(value)"
          >
            {{ t(value) }}
          </button>
        </div>

        <AppForge />

        <CommonSwitch
          v-if="manager.endpoint === 'installed'"
          v-model="showSystem"
          :label="t('showSystemApps')"
          testid="app_manager_show_system"
        />

        <fieldset class="app_toolbar manager_view" :aria-label="t('appView')">
          <button
            v-for="mode in ['list', 'preview'] as const"
            :key="mode"
            type="button"
            :aria-pressed="manager.view === mode"
            :aria-label="t(mode === 'list' ? 'listView' : 'previewView')"
            :title="t(mode === 'list' ? 'listView' : 'previewView')"
            :data-testid="`app_manager_view_${mode}`"
            :disabled="empty"
            @click="manager.setView(mode)"
          >
            <CommonIcon :src="mode === 'list' ? listIcon : gridIcon" />
          </button>

          <button
            type="button"
            :disabled="manager.loading || manager.busy || (empty && !manager.error)"
            data-testid="app_manager_refresh"
            @click="manager.load"
          >
            <CommonIcon :src="refreshIcon" size="small" />{{ t('refresh') }}
          </button>

          <button
            type="button"
            :aria-label="t('managerSettings')"
            :title="t('managerSettings')"
            data-testid="app_manager_settings_open"
            @click="settingsOpen = true"
          >
            <CommonIcon :src="settingsIcon" />
          </button>
        </fieldset>
      </div>

      <ActionButtons />

      <CommonSpinner
        v-if="manager.loading"
        size="huge"
        centered
        class="manager_loading"
        testid="app_manager_loading"
      />

      <AlertBanner v-if="manager.error" tone="error" compact data-testid="app_manager_error">
        {{ manager.error }}
      </AlertBanner>
      <AppList v-if="!manager.loading" />

      <ManagerSettings v-if="settingsOpen" v-model="settingsOpen" />
    </div>
  </AppView>
</template>

<style scoped lang="scss">

.manager_content {
  min-height: 100%;
}

.manager_loading {
  flex: 1;
  min-height: $spinner_huge + $space_10;
}

.manager_view {
  @extend %fieldset_reset;
  margin-left: auto;
}

.manager_navigation {
  flex-wrap: wrap;
}

</style>
