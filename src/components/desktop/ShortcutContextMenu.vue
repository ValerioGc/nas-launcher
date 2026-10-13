<script setup lang="ts">

import { computed } from 'vue';
import { t } from '@/i18n';
import { useContextMenu } from '@/composables/useContextMenu';
import { useAppsStore } from '@/stores/appsStore';

import CommonContextMenu from '@/components/commons/CommonContextMenu.vue';

interface Props {
  testid?: string;
}

const props = withDefaults(defineProps<Props>(), {
  testid: 'menu_shortcut',
});

const apps = useAppsStore();
const { open, target, point, show, close } = useContextMenu<string>();

const folder = computed(() =>
  apps.desktopItems.find((item) => item.key === target.value && item.kind === 'folder'),
);
const appId = computed(() =>
  target.value?.startsWith('app:') ? target.value.slice(4) : target.value,
);
const onDesktop = computed(
  () => Boolean(folder.value) || apps.shortcuts.some((app) => app.id === appId.value),
);
const label = computed(() => t(onDesktop.value ? 'removeShortcut' : 'addShortcut'));

async function toggle() {
  if (target.value) {
    if (folder.value) {
      await apps.folderShortcut(folder.value.target, folder.value.name, true);
    } else if (appId.value) {
      await apps.shortcut(appId.value, onDesktop.value);
    }
    close();
  }
}

defineExpose({ show, close });

</script>

<template>
  <CommonContextMenu v-model="open" :x="point.x" :y="point.y" :label="label">
    <button type="button" role="menuitem" :data-testid="props.testid" @click="toggle">
      {{ label }}
    </button>
  </CommonContextMenu>
</template>
