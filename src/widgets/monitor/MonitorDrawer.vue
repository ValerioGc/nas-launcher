<script setup lang="ts">

import { computed, inject, ref, watch } from 'vue';
import { t } from '@/i18n';
import { useShellStore } from '@/stores/shellStore';
import { APP_BRIDGE, MONITOR_WIDGET } from '@/composables/useAppBridge';
import { SYSTEM_MONITOR } from '@/api/systemMonitor';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import WidgetDrawer from '@/widgets/WidgetDrawer.vue';
import closeIcon from '@/assets/icons/navigation/close.svg';
import openIcon from '@/assets/icons/navigation/open.svg';

const open = defineModel<boolean>({ default: false });

const shell = useShellStore();
const bridge = inject(APP_BRIDGE, null);
const BASE = `/apps/${SYSTEM_MONITOR}/`;
const source = ref('');
const loaded = ref(false);
const loading = computed(
  () => !source.value || !(bridge ? bridge.ready.has(MONITOR_WIDGET) : loaded.value),
);

watch(
  open,
  async (visible) => {
    source.value = '';
    loaded.value = false;
    if (!visible) {
      return;
    }
    await shell.start(SYSTEM_MONITOR);
    if (open.value && !shell.startErrors[SYSTEM_MONITOR]) {
      const target = new URL(BASE, window.location.origin);
      target.searchParams.set('view', 'widget');
      target.searchParams.set(
        'theme',
        document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light',
      );
      target.searchParams.set('lang', document.documentElement.lang === 'en' ? 'en' : 'it');
      source.value = `${target.pathname}${target.search}`;
    }
  },
  { immediate: true },
);

function onLoad() {
  loaded.value = true;
  bridge?.announce(MONITOR_WIDGET);
}

function openApplication() {
  open.value = false;
  shell.open(SYSTEM_MONITOR);
}

</script>

<template>
  <WidgetDrawer
    v-model="open"
    panel-id="process_panel"
    :label="t('processes')"
    testid="process_panel"
  >
    <section class="monitor_widget app_surface" :aria-label="t('processes')">
      <div class="app_toolbar monitor_heading">
        <h2>{{ t('processes') }}</h2>
        <button
          type="button"
          :aria-label="t('openSystemMonitor')"
          :title="t('openSystemMonitor')"
          data-testid="process_widget_open_app"
          @click="openApplication"
        >
          <CommonIcon :src="openIcon" />
        </button>
        <button
          type="button"
          :aria-label="t('close')"
          :title="t('close')"
          data-testid="widget_close"
          @click="open = false"
        >
          <CommonIcon :src="closeIcon" />
        </button>
      </div>

      <p v-if="shell.startErrors[SYSTEM_MONITOR]" role="alert" class="app_error">
        {{ shell.startErrors[SYSTEM_MONITOR] }}
      </p>
      <div v-else class="monitor_content" :aria-busy="loading">
        <CommonSpinner
          v-if="loading"
          class="monitor_loading"
          size="large"
          centered
          testid="process_widget_loading"
        />
        <iframe
          v-if="source"
          :ref="(element) => bridge?.attach(MONITOR_WIDGET, element as HTMLIFrameElement | null)"
          class="monitor_frame"
          :class="{ monitor_frame_loading: loading }"
          :src="source"
          :title="t('processes')"
          data-testid="process_widget_frame"
          @load="onLoad"
        ></iframe>
      </div>
    </section>
  </WidgetDrawer>
</template>

<style scoped lang="scss">

.monitor_widget {
  @extend %stack;
  height: 100%;
  min-height: 0;
  padding: $space_4;
}

.monitor_heading h2 {
  margin-right: auto;
}

.monitor_content {
  position: relative;
  flex: 1;
  min-height: 0;
  background: var(--surface);
}

.monitor_loading {
  position: absolute;
  inset: 0;
}

.monitor_frame {
  @extend %full_wh;
  border: 0;
  background: var(--surface);

  &_loading {
    visibility: hidden;
  }
}

</style>
