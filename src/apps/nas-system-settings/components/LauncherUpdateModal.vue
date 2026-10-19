<script setup lang="ts">

import { computed, ref } from 'vue';
import { t } from '@/i18n';
import { request } from '@/api/client';
import { usePolling } from '@/composables/usePolling';
import type { ComponentUpdate, NasUpdates } from '../types/update';
import type { UserTask } from '@/types/widgets/tasks';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonModal from '@/components/commons/CommonModal.vue';
import ProgressBar from '@/components/alerts/ProgressBar.vue';
import { failureMessage } from '@/utils/failureMessage';

interface Props {
  components: ComponentUpdate[];
}

const props = defineProps<Props>();

const open = defineModel<boolean>({ required: true });

const POLL_MS = 2000;

type Phase = 'confirm' | 'progress' | 'restarting' | 'failed';

const phase = ref<Phase>('confirm');
const task = ref<UserTask | null>(null);
const failure = ref('');
let wentDown = false;

const working = computed(() => phase.value === 'progress' || phase.value === 'restarting');
const progress = computed(() =>
  phase.value === 'restarting' ? null : (task.value?.progress ?? null),
);

const step = computed(() => {
  if (phase.value === 'restarting') {
    return t('updateRestarting');
  }
  const detail = task.value?.detail || 'downloading';
  const downloading = detail.match(/^downloading-(\w+)$/);
  if (downloading) {
    return t('updateStep_downloadingComponent', {
      component: t(`nasComponent_${downloading[1]}`),
    });
  }

  return t(`updateStep_${detail}`);
});

const shown = computed({
  get: () => open.value,
  set: (value: boolean) => {
    if (!working.value) {
      open.value = value;
    }
  },
});

async function start() {
  failure.value = '';
  try {
    const { task: id } = await request<{ task: string }>('/launcher/updates', {
      method: 'POST',
      body: '{}',
    });
    task.value = { id, detail: 'downloading', progress: null } as UserTask;
    phase.value = 'progress';
  } catch (error) {
    fail(failureMessage(error));
  }
}

function fail(message: string) {
  failure.value = message;
  phase.value = 'failed';
}

async function followTask() {
  try {
    const found = (await request<UserTask[]>('/tasks')).find((item) => item.id === task.value?.id);
    if (found?.state === 'failed') {
      fail(found.detail || t('requestFailed'));
    } else if (found) {
      task.value = found;
      if (found.state === 'done') {
        phase.value = 'restarting';
      }
    }
  } catch {
    wentDown = true;
    phase.value = 'restarting';
  }
}

/**
 * Follows the restart process after the update has been applied.
 * Checks if the system has come back online and verifies if the update was successful.
 */
async function followRestart() {
  try {
    const running = await request<NasUpdates>('/launcher/updates');
    if (!wentDown) {
      return;
    }
    const updated = props.components.every(
      (planned) =>
        running.components.find((item) => item.component === planned.component)?.currentVersion ===
        planned.availableVersion,
    );
    if (updated) {
      globalThis.location.reload();
    } else {
      fail(t('launcherRolledBack'));
    }
  } catch {
    wentDown = true;
  }
}

usePolling(
  () => {
    if (phase.value === 'progress') {
      followTask();
    } else if (phase.value === 'restarting') {
      followRestart();
    }
  },
  computed(() => POLL_MS),
);

</script>

<template>
  <CommonModal
    v-model="shown"
    :title="t('launcherUpdateTitle')"
    :hide-close-button="working"
    data-testid="launcher_update_modal"
  >
    <template v-if="phase === 'confirm'">
      <ul class="update_plan" data-testid="launcher_update_plan">
        <li v-for="item in components" :key="item.component">
          {{
            t('updatePlanItem', {
              component: t(`nasComponent_${item.component}`),
              from: item.currentVersion ?? '-',
              to: item.availableVersion ?? '-',
            })
          }}
        </li>
      </ul>
      <p>{{ t('launcherUpdateConfirm') }}</p>
    </template>

    <div v-else-if="working" class="app_stack" aria-live="polite">
      <ProgressBar :value="progress" :label="t('launcherUpdateTitle')" />
      <p data-testid="launcher_update_step">{{ step }}</p>
    </div>

    <AlertBanner v-else tone="error" data-testid="launcher_update_failed">
      {{ failure }}
    </AlertBanner>

    <template #footer>
      <template v-if="phase === 'confirm'">
        <button type="button" @click="open = false">{{ t('cancel') }}</button>
        <button type="button" class="primary" data-testid="launcher_update_start" @click="start">
          {{ t('startUpdate') }}
        </button>
      </template>
      <button v-else-if="phase === 'failed'" type="button" @click="open = false">
        {{ t('close') }}
      </button>
    </template>
  </CommonModal>
</template>
