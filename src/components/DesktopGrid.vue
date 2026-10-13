<script setup lang="ts">

import { computed, onBeforeUnmount, ref, useTemplateRef } from 'vue';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import { t } from '@/i18n';
import type { DesktopItem } from '@/types/types';

import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import GridItem from '@/components/desktop/GridItem.vue';
import ShortcutContextMenu from '@/components/desktop/ShortcutContextMenu.vue';

const apps = useAppsStore();
const shell = useShellStore();
const settings = useSettingsStore();
const iconSize = computed(() => settings.general?.desktopIconSize ?? 'large');
const shortcutMenu = useTemplateRef<InstanceType<typeof ShortcutContextMenu>>('shortcutMenu');

// Distance in pixels a pressed icon has to travel before it counts as dragged, not clicked.
const DRAG_THRESHOLD = 6;

const dragged = ref<string | null>(null);
const target = ref<string | null>(null);
let pressed: { id: string; x: number; y: number } | null = null;
let justDragged = false;

/** Moves an icon to the place of another, which shifts the ones in between. */
function move(id: string, to: string) {
  const order = apps.desktopItems.map((item) => item.key);
  const from = order.indexOf(id);
  const index = order.indexOf(to);
  if (from === -1 || index === -1 || from === index) {
    return;
  }
  order.splice(index, 0, ...order.splice(from, 1));
  void apps.reorder(
    order.map((key) => {
      const item = apps.desktopItems.find((entry) => entry.key === key)!;
      return { kind: item.kind, target: item.target };
    }),
  );
}

function step(id: string, by: number) {
  const order = apps.desktopItems.map((item) => item.key);
  const to = order[order.indexOf(id) + by];
  if (to) {
    move(id, to);
  }
}

function drag(event: PointerEvent) {
  if (!pressed) {
    return;
  }
  const distance = Math.hypot(event.clientX - pressed.x, event.clientY - pressed.y);
  if (!dragged.value && distance < DRAG_THRESHOLD) {
    return;
  }
  dragged.value = pressed.id;
  const cell = document
    .elementFromPoint(event.clientX, event.clientY)
    ?.closest<HTMLElement>('[data-shortcut]');
  target.value = cell?.dataset.shortcut ?? null;
}

function stop() {
  globalThis.removeEventListener('pointermove', drag);
  globalThis.removeEventListener('pointerup', drop);
  globalThis.removeEventListener('pointercancel', stop);
  pressed = null;
  dragged.value = null;
  target.value = null;
}

function drop() {
  justDragged = Boolean(dragged.value);
  if (dragged.value && target.value) {
    move(dragged.value, target.value);
  }
  stop();
}

function press(id: string, event: PointerEvent) {
  if (event.button !== 0) {
    return;
  }
  justDragged = false;
  pressed = { id, x: event.clientX, y: event.clientY };
  globalThis.addEventListener('pointermove', drag);
  globalThis.addEventListener('pointerup', drop);
  globalThis.addEventListener('pointercancel', stop);
}

// A release after a drag must not also open the icon it ended on.
function open(item: DesktopItem) {
  if (justDragged) {
    justDragged = false;
    return;
  }
  if (item.kind === 'folder') {
    if (!item.available) {
      apps.errorMessage = t('explorerUnavailable');
      return;
    }
    shell.open('nas-file-manager', { location: `?${new URLSearchParams({ path: item.target })}` });
  } else {
    shell.open(item.target);
  }
}

onBeforeUnmount(stop);

</script>

<template>
  <div class="desktop_surface">
    <p v-if="apps.errorMessage" role="alert">{{ apps.errorMessage }}</p>
    <CommonSpinner
      v-if="apps.loading && !apps.desktopItems.length"
      size="large"
      centered
      testid="desktop_loading"
    />

    <ul
      v-else
      class="desktop_grid"
      :class="`desktop_grid_${iconSize}`"
      :data-icon-size="iconSize"
      :aria-label="t('desktop')"
    >
      <GridItem
        v-for="item in apps.desktopItems"
        :key="item.key"
        :data-testid="`desktop_shortcut_${item.kind === 'app' ? item.target : item.key}`"
        :data-shortcut="item.key"
        :class="{
          desktop_dragged: dragged === item.key,
          desktop_drop_target: dragged && target === item.key && dragged !== item.key,
        }"
        :name="item.name"
        :icon="item.icon"
        @pointerdown="press(item.key, $event)"
        @open="open(item)"
        @context="shortcutMenu?.show(item.key, $event)"
        @move="step(item.key, $event)"
      />
    </ul>

    <ShortcutContextMenu ref="shortcutMenu" testid="desktop_shortcut_menu" />
  </div>
</template>

<style scoped lang="scss">

.desktop_surface {
  @extend %full_wh;
  overflow: auto;
  padding: $space_5;
}

.desktop_grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(var(--desktop-cell), 1fr));
  gap: $space_3;
  margin-bottom: $space_5;
  align-items: start;
}

@each $name, $icon, $cell in $desktop_icon_sizes {
  .desktop_grid_#{$name} {
    --desktop-icon: #{$icon};
    --desktop-cell: #{$cell};
  }
}

.desktop_dragged {
  opacity: $opacity_dragged;
}

.desktop_drop_target {
  @include focus-ring;
  border-radius: $radius_control;
}

</style>
