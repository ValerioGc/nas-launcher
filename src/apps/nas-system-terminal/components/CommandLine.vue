<script setup lang="ts">

import { ref, useTemplateRef, watch } from 'vue';
import { t } from '@/i18n';
import { useTerminalStore } from '@/apps/nas-system-terminal/store';

const terminal = useTerminalStore();
const command = ref('');
const field = useTemplateRef<HTMLInputElement>('field');
const position = ref(terminal.history.length);

watch(
  () => terminal.history.length,
  (length) => {
    position.value = length;
  },
);

function submit() {
  if (terminal.running) {
    return;
  }
  terminal.run(command.value);
  command.value = '';
}

/**
 * Walks the history with the arrows, the way a shell does: up goes back, down comes forward to an
 * empty line.
 */
function browse(step: number) {
  const next = Math.min(terminal.history.length, Math.max(0, position.value + step));
  position.value = next;
  command.value = terminal.history[next] ?? '';
}

defineExpose({ focus: () => field.value?.focus() });

</script>

<template>
  <form class="terminal_prompt" data-testid="terminal_prompt" @submit.prevent="submit">
    <label for="terminal_command" class="terminal_cwd">{{ terminal.cwd || '/' }} $</label>
    <input
      id="terminal_command"
      ref="field"
      v-model="command"
      :aria-label="t('terminalCommand')"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
      maxlength="4096"
      :disabled="terminal.connection !== 'open'"
      data-testid="terminal_command"
      @keydown.up.prevent="browse(-1)"
      @keydown.down.prevent="browse(1)"
    />

    <button
      v-if="terminal.running"
      type="button"
      data-testid="terminal_interrupt"
      @click="terminal.interrupt"
    >
      {{ t('terminalInterrupt') }}
    </button>

    <button
      v-else
      type="submit"
      class="primary"
      :disabled="terminal.connection !== 'open'"
      data-testid="terminal_run"
    >
      {{ t('terminalRun') }}
    </button>
  </form>
</template>

<style scoped lang="scss">

.terminal_prompt {
  @extend %fx_base;
  gap: $space_2;

  input {
    flex: 1;
    font-family: $font_mono;
  }
}

.terminal_cwd {
  @extend %truncated_text;
  max-width: 40%;
  color: var(--terminal-highlight, var(--accent));
  font-family: $font_mono;
}

</style>
