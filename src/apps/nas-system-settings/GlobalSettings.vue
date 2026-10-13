<script setup lang="ts">

import { ref, watch } from 'vue';
import { t } from '@/i18n';
import { useShellStore } from '@/stores/shellStore';
import { sectionIcons, sectionLabels } from './search';
import type { SettingsSection } from './types/search';
import type { AppSection } from '@/apps/components/types';

import AppView from '@/apps/AppView.vue';
import AppSections from '@/apps/components/AppSections.vue';
import GeneralSettingsTab from './components/tabs/GeneralSettingsTab.vue';
import AccountSettingsTab from './components/tabs/AccountSettingsTab.vue';
import NetworkSettingsTab from './components/tabs/NetworkSettingsTab.vue';
import PowerPlanningTab from './components/tabs/PowerPlanningTab.vue';
import SystemUpdatesTab from './components/tabs/SystemUpdatesTab.vue';
import RecoverySettingsTab from './components/tabs/RecoverySettingsTab.vue';

/**
 * Global settings sections configuration
 */
const sections = [
  {
    id: 'general',
    label: sectionLabels.general,
    icon: sectionIcons.general,
    component: GeneralSettingsTab,
  },
  {
    id: 'account',
    label: sectionLabels.account,
    icon: sectionIcons.account,
    component: AccountSettingsTab,
  },
  {
    id: 'network',
    label: sectionLabels.network,
    icon: sectionIcons.network,
    component: NetworkSettingsTab,
  },
  {
    id: 'power',
    label: sectionLabels.power,
    icon: sectionIcons.power,
    component: PowerPlanningTab,
  },
  {
    id: 'updates',
    label: sectionLabels.updates,
    icon: sectionIcons.updates,
    component: SystemUpdatesTab,
  },
  {
    id: 'recovery',
    label: sectionLabels.recovery,
    icon: sectionIcons.recovery,
    component: RecoverySettingsTab,
  },
] satisfies (AppSection & { id: SettingsSection })[];

const shell = useShellStore();
const selected = ref<string>();

watch(
  () => shell.launchQuery,
  (query) => {
    if (sections.some((section) => section.id === query)) {
      selected.value = query;
    }
  },
  { immediate: true, flush: 'sync' },
);

</script>

<template>
  <AppView class="settings_view" :name="t('settings')">
    <AppSections v-model:selected="selected" :sections="sections" :label="t('settings')" />
  </AppView>
</template>

<style lang="scss">

@use './styles/settings';

</style>
