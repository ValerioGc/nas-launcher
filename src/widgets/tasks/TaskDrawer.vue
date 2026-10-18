<script setup lang="ts">

import { t } from '@/i18n';

import WidgetDrawer from '@/widgets/WidgetDrawer.vue';
import UserTasks from '@/widgets/tasks/UserTasks.vue';

const open = defineModel<boolean>({ default: false });

</script>

<template>
  <WidgetDrawer v-model="open" panel-id="task_panel" :label="t('tasks')" testid="task_panel">
    <UserTasks @close="open = false" />
  </WidgetDrawer>
</template>
