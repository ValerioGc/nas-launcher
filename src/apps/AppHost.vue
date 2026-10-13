<script setup lang="ts">

import { computed, inject, onBeforeUnmount, reactive, watch } from 'vue';
import { t } from '@/i18n';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { APP_BRIDGE, useAppBridge } from '@/composables/useAppBridge';
import { randomUuid } from '@/utils/randomUuid';

import CommonEmpty from '@/components/commons/CommonEmpty.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';

const apps = useAppsStore();
const shell = useShellStore();

// The main page shares its bridge with the widgets; alone, the host makes its own.
const bridge = inject(APP_BRIDGE, null) ?? useAppBridge();
const picker = bridge.picker;
const modal = computed(() => apps.apps.find((app) => app.id === shell.modalId));

const PROBE_MS = 8000;
/** How long after its page loads an application that never says `ready` is shown anyway. */
const READY_FALLBACK_MS = 4000;

const loaded = reactive(new Set<string>());
const fallbacks = new Map<string, number>();
const unreachable = reactive(new Set<string>());
const reloads = reactive(new Map<string, number>());
const launchTokens = reactive(new Map<string, string>());

watch(
  () => shell.openApps.map((app) => app.id),
  (ids) => {
    for (const id of ids) {
      if (!launchTokens.has(id)) {
        launchTokens.set(id, randomUuid());
      }
    }
    for (const id of launchTokens.keys()) {
      if (!ids.includes(id)) {
        launchTokens.delete(id);
      }
    }
    for (const id of unreachable) {
      if (!ids.includes(id)) {
        unreachable.delete(id);
      }
    }
    for (const id of reloads.keys()) {
      if (!ids.includes(id)) {
        reloads.delete(id);
      }
    }
  },
  { immediate: true },
);

function forget(id: string) {
  window.clearTimeout(fallbacks.get(id));
  fallbacks.delete(id);
}

function attach(id: string, frame: HTMLIFrameElement | null) {
  bridge.attach(id, frame);
  if (!frame) {
    loaded.delete(id);
    forget(id);
  }
}

/**
 * The cover stays until the application says it is ready, so its own first paint and loading
 * text stay behind it; an application that never says so is shown a few seconds after its page
 * loaded.
 */
function covered(id: string) {
  if (shell.startErrors[id] || unreachable.has(id)) {
    return false;
  }
  return shell.starting.includes(id) || (!bridge.ready.has(id) && !loaded.has(id));
}

async function announce(id: string, url: string) {
  forget(id);
  fallbacks.set(
    id,
    window.setTimeout(() => loaded.add(id), READY_FALLBACK_MS),
  );
  bridge.announce(id);
  try {
    const answer = await fetch(url, { cache: 'no-store', signal: AbortSignal.timeout(PROBE_MS) });
    if (!answer.ok && shell.openApps.some((app) => app.id === id)) {
      unreachable.add(id);
    }
  } catch {
    if (shell.openApps.some((app) => app.id === id)) {
      unreachable.add(id);
    }
  }
}

function address(id: string, url: string) {
  const target = new URL(`${url}${shell.locations[id] ?? ''}`, window.location.origin);
  const root = document.documentElement;
  target.searchParams.set('launch', launchTokens.get(id) ?? '');
  target.searchParams.set('theme', root.dataset.theme === 'dark' ? 'dark' : 'light');
  target.searchParams.set('lang', root.lang === 'en' ? 'en' : 'it');
  return `${target.pathname}${target.search}${target.hash}`;
}

onBeforeUnmount(() => {
  for (const id of fallbacks.keys()) {
    forget(id);
  }
});

function retry(id: string) {
  unreachable.delete(id);
  reloads.set(id, (reloads.get(id) ?? 0) + 1);
}

</script>

<template>
  <div v-show="shell.activeId" class="app_host" data-testid="app_host">
    <div
      v-for="app in shell.openApps"
      v-show="shell.activeId === app.id"
      :key="app.id"
      class="app_host_panel"
    >
      <component
        :is="app.entry.source.component"
        v-if="app.entry.source.kind === 'internal'"
        :key="app.id"
      />

      <!-- External application in its own process, on the origin of the launcher. No sandbox: one
           that runs scripts and keeps the origin is no wall there and only makes the browser warn.
           The application keeps its rules itself (frameGuard.ts of every application): no window
           of its own, no dialog, no navigation of this page. -->
      <template v-else>
        <div v-if="covered(app.id)" class="app_host_loading" :data-testid="`app_loading_${app.id}`">
          <!-- Only the spinner shows: the name is for screen readers. -->
          <CommonSpinner
            size="huge"
            centered
            compact
            :label="t('appOpening', { name: app.name })"
          />
        </div>

        <CommonEmpty
          v-if="shell.startErrors[app.id]"
          class="app_host_unreachable"
          :title="t('appUnreachable')"
          :hint="shell.startErrors[app.id]"
          :testid="`app_start_error_${app.id}`"
        >
          <button type="button" @click="shell.start(app.id)">{{ t('retry') }}</button>
        </CommonEmpty>

        <CommonEmpty
          v-if="unreachable.has(app.id)"
          class="app_host_unreachable"
          :title="t('appUnreachable')"
          :hint="t('appUnreachableHint')"
          :testid="`app_unreachable_${app.id}`"
        >
          <button type="button" @click="retry(app.id)">{{ t('retry') }}</button>
        </CommonEmpty>

        <iframe
          v-if="!shell.starting.includes(app.id) && !shell.startErrors[app.id]"
          :key="reloads.get(app.id) ?? 0"
          :ref="(element) => attach(app.id, element as HTMLIFrameElement | null)"
          :src="address(app.id, app.entry.source.url)"
          :title="app.name"
          class="app_frame"
          :data-testid="`app_frame_${app.id}`"
          referrerpolicy="no-referrer"
          @load="announce(app.id, address(app.id, app.entry.source.url))"
        ></iframe>
      </template>
    </div>
  </div>

  <component
    :is="modal.entry.source.component"
    v-if="modal?.entry.source.kind === 'internal'"
    :key="modal.id"
    :model-value="true"
    @update:model-value="shell.close(modal?.id)"
  />

  <!-- The picker draws its own title and cross. Escape, or a click beside its window, cancels it
       here too, should the page of the picker not answer. -->
  <dialog
    v-if="picker"
    open
    class="file_picker_overlay"
    aria-modal="true"
    :aria-label="t('filePicker')"
    data-testid="app_file_picker"
    @click.self="bridge.cancelPicker()"
    @keydown.esc="bridge.cancelPicker()"
  >
    <div class="file_picker_window">
      <iframe
        :ref="(element) => bridge.attachPicker(element as HTMLIFrameElement | null)"
        :src="picker.url"
        :title="t('filePicker')"
        referrerpolicy="no-referrer"
        @load="bridge.announcePicker()"
      ></iframe>
    </div>
  </dialog>
</template>

<style scoped lang="scss">

.app_host {
  position: absolute;
  inset: 0;

  &_panel {
    @extend %full_wh;
    position: relative;
  }

  &_loading,
  &_unreachable {
    position: absolute;
    z-index: 1;
    inset: 0;
    background: var(--surface);
  }
}

.app_frame {
  @extend %full_wh;
  border: 0;
  background: transparent;
}

.file_picker_overlay {
  position: fixed;
  z-index: 100;
  inset: 0;
  // A plain overlay, not the box the browser draws for a dialog.
  width: auto;
  max-width: none;
  height: auto;
  max-height: none;
  margin: 0;
  border: 0;
  color: inherit;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgb(0 0 0 / 55%);
}

.file_picker_window {
  display: flex;
  flex-direction: column;
  width: min(900px, 100%);
  height: min(700px, 100%);
  overflow: hidden;
  border-radius: 14px;
  background: var(--surface);
  box-shadow: var(--shadow);

  iframe {
    flex: 1;
    width: 100%;
    border: 0;
  }
}

</style>
