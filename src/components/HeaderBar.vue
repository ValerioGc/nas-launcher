<script lang="ts" setup>

import { computed } from 'vue';
import { t } from '@/i18n';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import SearchField from '@/components/header/SearchField.vue';
import ExternalDisksDropdown from '@/components/header/ExternalDisksDropdown.vue';
import SystemActionDropdown from '@/components/header/SystemActionDropdown.vue';
import NotificationDropdown from '@/apps/nas-notification-manager/components/NotificationDropdown.vue';

import menu from '@/assets/icons/navigation/menu.svg';
import desktop from '@/assets/icons/navigation/desktop.svg';
import processes from '@/assets/icons/system/processes.svg';
import tasks from '@/assets/icons/system/tasks.svg';

const props = defineProps<{
  menuOpen?: boolean;
  processesOpen?: boolean;
  processesAvailable?: boolean;
  tasksOpen?: boolean;
  appOpen?: boolean;
}>();

const emit = defineEmits<{
  toggleMenu: [];
  toggleProcesses: [];
  toggleTasks: [];
  showDesktop: [];
  search: [query: string];
}>();

const widgetControls = computed(() =>
  [
    {
      id: 'process',
      icon: processes,
      label: t('processes'),
      open: Boolean(props.processesOpen),
      toggle: () => emit('toggleProcesses'),
    },
    {
      id: 'task',
      icon: tasks,
      label: t('tasks'),
      open: Boolean(props.tasksOpen),
      toggle: () => emit('toggleTasks'),
    },
  ].filter((control) => control.id !== 'process' || props.processesAvailable),
);

</script>

<template>
  <header>
    <button
      type="button"
      class="header_menu_button"
      :class="{ header_menu_button_active: props.menuOpen }"
      :aria-expanded="Boolean(props.menuOpen)"
      aria-controls="app_menu_panel"
      :aria-label="t('openAppMenu')"
      data-testid="header_menu_toggle"
      @click="emit('toggleMenu')"
    >
      <CommonIcon :src="menu" size="large" />
    </button>

    <button
      v-if="props.appOpen"
      type="button"
      class="header_menu_button"
      :aria-label="t('desktop')"
      :title="t('desktop')"
      data-testid="header_desktop"
      @click="emit('showDesktop')"
    >
      <CommonIcon :src="desktop" size="large" />
    </button>

    <div class="header_actions">
      <SearchField @search="emit('search', $event)" />

      <span class="header_separator" aria-hidden="true"></span>

      <nav :aria-label="t('notifications')">
        <ul>
          <li><NotificationDropdown /></li>
          <li><ExternalDisksDropdown /></li>
        </ul>
      </nav>

      <span class="header_separator" aria-hidden="true"></span>

      <nav :aria-label="t('widgets')">
        <ul>
          <li v-for="control in widgetControls" :key="control.id">
            <button
              type="button"
              class="header_action_button"
              :class="{ header_menu_button_active: control.open }"
              :aria-expanded="control.open"
              :aria-controls="`${control.id}_panel`"
              :aria-label="control.label"
              :title="control.label"
              :data-testid="`header_${control.id}_toggle`"
              @click="control.toggle()"
            >
              <CommonIcon :src="control.icon" />
            </button>
          </li>
        </ul>
      </nav>

      <SystemActionDropdown />
    </div>
  </header>
</template>

<style lang="scss" scoped>

header {
  @extend %fx_between_center;
  flex-shrink: 0;
  gap: $space_4;
  padding: $space_2 $space_4;
  border-bottom: $border_thin solid var(--surface-border);
  background: var(--surface);
  font-family: $font_ui;

  nav ul {
    @extend %fx_base;
    gap: $space_2;
  }
}

.header_actions {
  @extend %fx_base;
  gap: $space_2;
  margin-left: auto;
}

.header_menu_button,
.header_action_button {
  @extend %icon_button;
}

.header_menu_button_active {
  background: var(--surface-hover);
}

.header_separator {
  width: $border_thin;
  height: $icon_large;
  background: var(--surface-border);
  margin-inline: $space_2;
}

@media (max-width: $breakpoint_small) {
  header {
    gap: 0;
    padding-inline: $space_2;

    nav ul {
      gap: 0;
    }
  }

  .header_actions {
    gap: 0;
  }
}

</style>
