<script setup lang="ts">

import { computed, nextTick, onBeforeUnmount, onMounted, ref, useTemplateRef, watch } from 'vue';
import { t } from '@/i18n';
import { useTerminalStore } from '@/apps/nas-system-terminal/store';

import AppView from '@/apps/AppView.vue';
import CommandLine from '@/apps/nas-system-terminal/components/CommandLine.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSegmented from '@/components/commons/CommonSegmented.vue';
import OptionsModal from '@/apps/nas-system-terminal/components/OptionsModal.vue';
import OutputPanel from '@/apps/nas-system-terminal/components/OutputPanel.vue';
import settingsIcon from '@/assets/icons/system/settings.svg';
import clearIcon from '@/assets/icons/navigation/clear.svg';
import connectedIcon from '@/assets/icons/status/connected.svg';
import connectingIcon from '@/assets/icons/status/loading.svg';
import disconnectedIcon from '@/assets/icons/status/disconnected.svg';

type View = 'console' | 'log';

const VIEWS: { value: View; label: string }[] = [
  { value: 'console', label: 'terminalConsole' },
  { value: 'log', label: 'terminalLog' },
];

const terminal = useTerminalStore();
const settingsOpen = ref(false);
const view = ref<View>('console');
const onLog = computed(() => view.value === 'log');
const connection = computed(() => (onLog.value ? terminal.logConnection : terminal.connection));
const connectionIcons = {
  connecting: connectingIcon,
  open: connectedIcon,
  closed: disconnectedIcon,
} as const;
const shown = computed(() => (onLog.value ? terminal.log : terminal.lines));
const prompt = useTemplateRef<InstanceType<typeof CommandLine>>('prompt');

const look = computed(() => {
  const chosen = terminal.settings;
  if (!chosen) {
    return {};
  }
  return {
    '--terminal-text': chosen.textColor,
    '--terminal-background': `color-mix(in srgb, ${chosen.backgroundColor} ${100 - chosen.transparency}%, transparent)`,
    '--terminal-highlight': chosen.highlightColor,
    '--terminal-font-size': `${chosen.fontSize}px`,
    '--terminal-line-height': String(chosen.lineHeight),
  };
});

// The window lets the desktop through as much as the output does.
const surface = computed(() => {
  const chosen = terminal.settings;
  return chosen
    ? {
        background: `color-mix(in srgb, var(--surface) ${100 - chosen.transparency}%, transparent)`,
      }
    : {};
});

watch(
  () => terminal.connection,
  async (state) => {
    if (state === 'open') {
      await nextTick();
      prompt.value?.focus();
    }
  },
);

watch(onLog, (following) => {
  if (following) {
    terminal.followLog();
  } else {
    terminal.stopLog();
  }
});

function reconnect() {
  if (onLog.value) {
    terminal.followLog();
  } else {
    terminal.connect();
  }
}

function clear() {
  if (onLog.value) {
    terminal.clearLog();
  } else {
    terminal.clear();
  }
}

onMounted(() => {
  terminal.connect();
  terminal.loadSettings();
});

onBeforeUnmount(() => {
  terminal.disconnect();
  terminal.stopLog();
});

</script>

<template>
  <AppView :name="t('systemTerminal')" :style="surface">
    <div class="app_stack terminal_app" :style="look" data-testid="terminal_app">
      <div class="app_toolbar">
        <CommonSegmented
          v-model="view"
          :options="VIEWS"
          :label="t('systemTerminal')"
          testid="terminal_view"
        />
        <output
          class="terminal_connection"
          :title="t(`terminal_${connection}`)"
          data-testid="terminal_connection"
        >
          <CommonIcon :src="connectionIcons[connection]" :label="t(`terminal_${connection}`)" />
        </output>
        <button
          v-if="connection === 'closed'"
          type="button"
          data-testid="terminal_reconnect"
          @click="reconnect"
        >
          {{ t('terminalReconnect') }}
        </button>
        <button
          type="button"
          class="terminal_clear"
          :aria-label="t('terminalClear')"
          :title="t('terminalClear')"
          :disabled="!shown.length"
          data-testid="terminal_clear"
          @click="clear"
        >
          <CommonIcon :src="clearIcon" />
        </button>
        <button
          type="button"
          class="terminal_settings_button"
          :aria-label="t('terminalSettings')"
          :title="t('terminalSettings')"
          :disabled="!terminal.settings"
          data-testid="terminal_settings_open"
          @click="settingsOpen = true"
        >
          <CommonIcon :src="settingsIcon" />
        </button>
      </div>

      <p v-if="terminal.error && !settingsOpen" role="alert" class="app_error">
        {{ terminal.error }}
      </p>

      <OutputPanel
        v-if="onLog"
        :lines="terminal.log"
        :received="terminal.logReceived"
        :waiting="t('terminalLogWaiting')"
        :label="t('terminalLog')"
        testid="terminal_log"
      />
      <template v-else>
        <OutputPanel
          :lines="terminal.lines"
          :received="terminal.received"
          :waiting="t('terminalWaiting')"
          :label="t('terminalConsole')"
          testid="terminal_output"
        />
        <CommandLine ref="prompt" />
      </template>

      <OptionsModal v-if="settingsOpen" v-model="settingsOpen" />
    </div>
  </AppView>
</template>

<style scoped lang="scss">

.terminal_app {
  height: 100%;
  min-height: 0;
}

.terminal_connection {
  display: inline-flex;
}

.terminal_clear {
  @extend %icon_button;
  margin-left: auto;
}

.terminal_settings_button {
  @extend %icon_button;
}

</style>
