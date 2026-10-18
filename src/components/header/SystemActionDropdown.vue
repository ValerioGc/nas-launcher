<script setup lang="ts">

import { onBeforeUnmount, ref } from 'vue';
import { request } from '@/api/client';
import { logout } from '@/api/gateway';
import { useShellStore } from '@/stores/shellStore';
import { t } from '@/i18n';
import type { PowerBusy, SystemAction } from '@/types/types';

import CommonDropdown from '@/components/commons/CommonDropdown.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import SystemActionConfirmModal from './SystemActionConfirmModal.vue';
import SystemRestartOverlay from './SystemRestartOverlay.vue';

import powerIcon from '@/assets/icons/system/power.svg';
import restartIcon from '@/assets/icons/system/restart.svg';
import shutdownIcon from '@/assets/icons/system/shutdown.svg';
import logoutIcon from '@/assets/icons/system/logout.svg';

const shell = useShellStore();
const root = ref<HTMLElement | null>(null);
const dropdownOpen = ref(false);
const confirmationOpen = ref(false);
const selected = ref<SystemAction | null>(null);
const remaining = ref<number | null>(null);
const busy = ref(false);
const error = ref('');
const restarting = ref(false);
const work = ref<PowerBusy>({ tasks: [], jobs: [] });

let timer: ReturnType<typeof setTimeout> | undefined;
let ticker: ReturnType<typeof setInterval> | undefined;
let restartTimer: ReturnType<typeof setTimeout> | undefined;
let restartWentDown = false;

const RESTART_POLL_MS = 2000;

function stopCountdown() {
  clearTimeout(timer);
  clearInterval(ticker);
  timer = undefined;
  ticker = undefined;
  remaining.value = null;
}

function stopRestartPolling() {
  clearTimeout(restartTimer);
  restartTimer = undefined;
}

function scheduleRestartPoll() {
  stopRestartPolling();
  if (restarting.value) {
    restartTimer = setTimeout(followRestart, RESTART_POLL_MS);
  }
}

/** Waits until the launcher goes offline and answers again before reloading the page. */
async function followRestart() {
  try {
    await request('/status');
    if (restartWentDown) {
      restarting.value = false;
      globalThis.location.reload();
      return;
    }
  } catch {
    restartWentDown = true;
  }
  scheduleRestartPoll();
}

function cancel() {
  if (busy.value) {
    return;
  }
  stopCountdown();
  confirmationOpen.value = false;
  selected.value = null;
  error.value = '';
}

/** What a restart or a shutdown would interrupt or has to wait for; nothing when it cannot tell. */
async function readWork(): Promise<PowerBusy> {
  try {
    const found = await request<PowerBusy>('/system/power/busy');
    if (Array.isArray(found?.tasks) && Array.isArray(found?.jobs)) {
      return found;
    }
  } catch {
    // The confirmation stays as it always was.
  }
  return { tasks: [], jobs: [] };
}

/**
 * Asks the user for confirmation before executing a system action. With operations under way the
 * window names them and waits for an explicit confirmation; a job of root prevents it.
 * @param action The system action to ask confirmation for.
 */
async function ask(action: SystemAction): Promise<void> {
  if (busy.value) {
    return;
  }

  stopCountdown();
  selected.value = action;
  error.value = '';
  work.value = { tasks: [], jobs: [] };
  dropdownOpen.value = false;
  root.value?.querySelector('button')?.focus();
  confirmationOpen.value = true;
  if (action.id !== 'logout') {
    work.value = await readWork();
  }
  if (
    !confirmationOpen.value ||
    selected.value?.id !== action.id ||
    work.value.tasks.length ||
    work.value.jobs.length
  ) {
    return;
  }
  remaining.value = 60;
  const deadline = Date.now() + 60000;

  timer = setTimeout(execute, 60000);
  ticker = setInterval(() => {
    remaining.value = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
  }, 1000);
}

/**
 * Executes the selected system action after confirmation.
 */
async function execute(): Promise<void> {
  if (busy.value || !confirmationOpen.value || !selected.value) {
    return;
  }

  const command = selected.value.id;
  stopCountdown();
  busy.value = true;
  error.value = '';

  try {
    if (command === 'logout') {
      await shell.endSession();
      await logout();
    } else {
      const outcome = await request<{ accepted: boolean; simulated: boolean }>(
        `/system/actions/${command}`,
        { method: 'POST' },
      );
      if (command === 'restart' && !outcome.simulated) {
        confirmationOpen.value = false;
        selected.value = null;
        restartWentDown = false;
        restarting.value = true;
        scheduleRestartPoll();
        return;
      }
      await logout().catch(() => {});
    }
    confirmationOpen.value = false;
    selected.value = null;
  } catch {
    error.value = t('requestFailed');
  } finally {
    busy.value = false;
  }
}

onBeforeUnmount(() => {
  stopCountdown();
  stopRestartPolling();
});

const systemActions: SystemAction[] = [
  { id: 'restart', icon: restartIcon },
  { id: 'shutdown', icon: shutdownIcon },
  { id: 'logout', icon: logoutIcon },
];

</script>

<template>
  <div ref="root">
    <CommonDropdown
      v-model="dropdownOpen"
      :icon="powerIcon"
      :trigger-label="t('systemActions')"
      align="end"
      data-testid="system_action_dropdown"
    >
      <template #default>
        <li v-for="action in systemActions" :key="action.id" role="presentation">
          <button
            type="button"
            class="system_action_row"
            role="menuitem"
            :disabled="busy"
            :title="t(action.id)"
            :aria-label="t(action.id)"
            :data-testid="`system_action_${action.id}`"
            @click="ask(action)"
          >
            <CommonIcon :src="action.icon" size="small" />
            <span>{{ t(action.id) }}</span>
          </button>
        </li>
      </template>
    </CommonDropdown>

    <SystemActionConfirmModal
      :open="confirmationOpen"
      :selected="selected"
      :remaining="remaining"
      :busy="busy"
      :error="error"
      :tasks="work.tasks"
      :jobs="work.jobs"
      @cancel="cancel"
      @execute="execute"
    />
    <SystemRestartOverlay v-model="restarting" />
  </div>
</template>

<style scoped lang="scss">

.system_action_row {
  @extend %button;
  width: 100%;
  justify-content: start;
  border: 0;
}

</style>
