<script lang="ts" setup>

import { computed, nextTick, ref, useTemplateRef, watch } from 'vue';
import { t } from '@/i18n';
import { appLabel, resolveApp, SETTINGS_APP_ID } from '@/apps/registry';
import { useAppsStore } from '@/stores/appsStore';
import type { MenuFolder, ShellApp } from '@/types/types';

import ShortcutContextMenu from '@/components/desktop/ShortcutContextMenu.vue';

const open = defineModel<boolean>({ required: true });

const emit = defineEmits<{
  select: [app: ShellApp];
  openSettings: [];
}>();

const appsStore = useAppsStore();

function inFolder(app: ShellApp, folder: string) {
  if (folder === 'all') {
    return true;
  }
  return folder === 'system' ? app.system || app.category === 'system' : app.category === folder;
}

const listedApps = computed(() =>
  appsStore.apps.filter((app) => app.id !== SETTINGS_APP_ID && inFolder(app, activeFolder.value)),
);

const settingsLogo = resolveApp(SETTINGS_APP_ID).icon;

const folders = computed<MenuFolder[]>(() => [
  { id: 'all', name: 'allApps' },
  { id: 'system', name: 'systemApp' },
  ...[
    ...new Set(
      appsStore.apps.flatMap((app) =>
        app.category && app.category !== 'system' ? [app.category] : [],
      ),
    ),
  ]
    .map((category) => ({ id: category, name: `category_${category}` }))
    .sort((one, other) => t(one.name).localeCompare(t(other.name))),
]);

const activeFolder = ref('all');
const shortcutMenu = useTemplateRef<InstanceType<typeof ShortcutContextMenu>>('shortcutMenu');

function context(app: ShellApp, event: MouseEvent | KeyboardEvent) {
  shortcutMenu.value?.show(app.id, event);
}

const panel = ref<HTMLElement | null>(null);
let lastFocused: HTMLElement | null = null;

function close() {
  shortcutMenu.value?.close();
  open.value = false;
}

function selectFolder(id: string) {
  activeFolder.value = id;
}

function selectApp(app: ShellApp) {
  emit('select', app);
  close();
}

function openSettings() {
  emit('openSettings');
  close();
}

/**
 * Watch for changes to the `open` prop and manage focus accordingly.
 */
watch(open, async (isOpen) => {
  if (isOpen) {
    lastFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    await nextTick();
    panel.value?.focus();
    return;
  }

  lastFocused?.focus();
  lastFocused = null;
});

</script>

<template>
  <Transition name="app_menu">
    <div v-if="open" class="app_menu_layer" data-testid="app_menu">
      <button
        type="button"
        class="app_menu_backdrop"
        :aria-label="t('closeMenu')"
        data-testid="app_menu_backdrop"
        @click="close"
      ></button>

      <dialog
        id="app_menu_panel"
        ref="panel"
        open
        class="app_menu_panel"
        aria-modal="true"
        :aria-label="t('appMenu')"
        tabindex="-1"
        @keydown.esc="close"
      >
        <div class="app_menu_body">
          <aside class="app_menu_folders">
            <ul>
              <li v-for="folder in folders" :key="folder.id">
                <button
                  type="button"
                  class="app_menu_row"
                  :class="{ app_menu_row_active: folder.id === activeFolder }"
                  :aria-pressed="folder.id === activeFolder"
                  :data-testid="`app_menu_folder_${folder.id}`"
                  @click="selectFolder(folder.id)"
                >
                  {{ t(folder.name) }}
                </button>
              </li>
            </ul>

            <button
              type="button"
              class="app_menu_row app_menu_settings"
              data-testid="app_menu_settings"
              @click="openSettings"
            >
              <img class="app_logo" :src="settingsLogo" alt="" />
              <span>{{ appLabel(SETTINGS_APP_ID, t('settings')) }}</span>
            </button>
          </aside>

          <ul class="app_menu_apps">
            <li v-for="app in listedApps" :key="app.id">
              <button
                type="button"
                class="app_menu_row"
                :data-testid="`app_menu_app_${app.id}`"
                @click="selectApp(app)"
                @contextmenu.prevent="context(app, $event)"
                @keydown.shift.f10.prevent="context(app, $event)"
              >
                <img class="app_logo app_menu_app_logo" :src="app.entry.icon" alt="" />
                <span>{{ app.name }}</span>
              </button>
            </li>

            <li v-if="appsStore.errorMessage" class="app_menu_empty" role="alert">
              {{ appsStore.errorMessage }}
            </li>
            <li v-else-if="listedApps.length === 0" class="app_menu_empty">
              {{ t('noAppsInstalled') }}
            </li>
          </ul>
        </div>
      </dialog>
    </div>
  </Transition>

  <ShortcutContextMenu ref="shortcutMenu" />
</template>

<style scoped lang="scss">

.app_menu_layer {
  position: fixed;
  inset: 0;
  z-index: $z_menu;
  font-family: $font_ui;

  .app_menu_backdrop {
    @extend %backdrop;
  }
}

.app_menu_panel {
  margin: 0;
  padding: 0;
  border: 0;
  color: inherit;
  max-width: none;
  position: relative;
  display: flex;
  flex-direction: column;
  width: min(100%, $menu_panel_width);
  height: 100%;
  background: var(--surface);
  border-right: $border_thin solid var(--surface-border);
  box-shadow: $shadow_overlay;

  &:focus-visible {
    @include focus-ring;
    outline-offset: -#{$focus_offset};
  }

  @media (max-width: $breakpoint_small) {
    width: 100%;
  }
}

.app_menu_body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.app_menu_folders {
  display: flex;
  flex-direction: column;
  gap: $space_4;

  width: $menu_aside_width;
  flex-shrink: 0;
  padding: $space_4;
  background: var(--surface-sunken);
  border-right: $border_thin solid var(--surface-border);
  overflow-y: auto;

  @media (max-width: $breakpoint_small) {
    width: $menu_aside_width * 0.75;
  }

  ul {
    @extend %stack;
    gap: $space_3;
  }
}

.app_menu_apps {
  flex: 1;
  padding: $space_4;
  overflow-y: auto;

  .app_menu_row {
    gap: $space_3;
    font-size: $text_large;
  }
}

.app_menu_row {
  @extend %button;
  width: 100%;
  justify-content: start;
  text-align: left;
  border: 0;
  background: transparent;

  &.app_menu_row_active {
    background: var(--surface-hover);
    font-weight: 650;
  }
}

.app_menu_settings {
  margin-top: auto;
  padding-top: $space_3;
  border-top: $border_thin solid var(--surface-border);
  border-radius: 0;
  color: var(--surface-muted);
}

.app_logo {
  width: $icon_large;
  height: $icon_large;
  flex-shrink: 0;
  object-fit: contain;

  &.app_menu_app_logo {
    width: $menu_app_icon;
    height: $menu_app_icon;
  }
}

.app_menu_empty {
  padding: $space_3;
  color: var(--surface-muted);
  font-size: $text_label;
}

// Backdrop animation and panel slide-in effects *********************
.app_menu-enter-active {
  .app_menu_backdrop {
    transition: opacity $duration_menu_in ease;
  }

  .app_menu_panel {
    transition:
      transform $duration_menu_in $ease_emphasized,
      opacity $duration_menu_in ease;
  }
}

.app_menu-leave-active {
  .app_menu_backdrop {
    transition: opacity $duration_menu_out ease;
  }

  .app_menu_panel {
    transition:
      transform $duration_menu_out ease-in,
      opacity $duration_menu_out ease-in;
  }
}

.app_menu-enter-from,
.app_menu-leave-to {
  .app_menu_backdrop {
    opacity: 0;
  }

  .app_menu_panel {
    opacity: 0;
    transform: translateX(-100%);
  }
}

@media (prefers-reduced-motion: reduce) {
  .app_menu-enter-active,
  .app_menu-leave-active {
    .app_menu_panel {
      transition-property: opacity;
    }
  }

  .app_menu-enter-from,
  .app_menu-leave-to {
    .app_menu_panel {
      transform: none;
    }
  }
}

</style>
