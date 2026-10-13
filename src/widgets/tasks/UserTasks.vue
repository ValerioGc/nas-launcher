<script setup lang="ts">

import { computed, ref } from 'vue';
import { t, formatDate } from '@/i18n';
import type { TaskState, UserTask } from '@/types/widgets/tasks';
import { useResource } from '@/composables/useResource';
import { usePolling } from '@/composables/usePolling';
import { useWidgetsStore } from '@/stores/widgetsStore';

import CommonEmpty from '@/components/commons/CommonEmpty.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSegmented from '@/components/commons/CommonSegmented.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import ProgressBar from '@/components/alerts/ProgressBar.vue';
import WidgetPanel from '@/widgets/WidgetPanel.vue';

import playIcon from '@/assets/icons/system/play.svg';
import pauseIcon from '@/assets/icons/system/pause.svg';
import stopIcon from '@/assets/icons/system/stop.svg';
import deleteIcon from '@/assets/icons/navigation/delete.svg';
import tasksIcon from '@/assets/icons/system/tasks.svg';

const emit = defineEmits<{ close: [] }>();

const { data, loading, error, busy, load, act } = useResource<UserTask[]>('/tasks', []);

type View = 'active' | 'finished';

const VIEWS: { value: View; label: string }[] = [
  { value: 'active', label: 'tasksActive' },
  { value: 'finished', label: 'tasksFinished' },
];

const ACTIVE = new Set<TaskState>(['running', 'paused']);

const view = ref<View>('active');

// The most recent operation first, in both lists.
const newestFirst = computed(() => [...data.value].sort((a, b) => b.createdAt - a.createdAt));
const active = computed(() => newestFirst.value.filter(isActive));
const finished = computed(() => newestFirst.value.filter((task) => !isActive(task)));
const shown = computed(() => (view.value === 'active' ? active.value : finished.value));

function isActive(task: UserTask) {
  return ACTIVE.has(task.state);
}

const widgets = useWidgetsStore();

usePolling(
  () => {
    if (!loading.value && !busy.value) {
      load();
    }
  },
  computed(() => widgets.pollingMs),
);

</script>

<template>
  <WidgetPanel :title="t('tasks')" testid="task_widget" @close="emit('close')">
    <CommonSegmented v-model="view" :options="VIEWS" :label="t('tasks')" testid="task_view" />

    <div v-if="shown.length" class="app_toolbar task_heading">
      <output v-if="view === 'active'" data-testid="task_count">
        {{ t('tasksRunning', { count: active.length }) }}
      </output>

      <button
        v-else
        type="button"
        :disabled="busy"
        data-testid="task_clear_finished"
        @click="act('/tasks/finished')"
      >
        <CommonIcon :src="deleteIcon" size="small" />{{ t('clearFinished') }}
      </button>
    </div>

    <p v-if="error" role="alert" class="app_error">{{ error }}</p>
    <CommonSpinner v-if="loading && !data.length" />

    <section class="app_scroll task_list" :aria-label="t('tasks')" data-testid="task_list">
      <ul>
        <li
          v-for="task in shown"
          :key="task.id"
          class="task_row app_panel"
          :class="[`task_${task.state}`, { task_over: !isActive(task) }]"
          :data-testid="`task_${task.id}`"
        >
          <div class="task_headline">
            <strong>{{ task.name }}</strong>

            <span class="task_badge" :data-testid="`task_state_${task.id}`">
              {{ t(`task_${task.state}`) }}
            </span>
          </div>

          <p v-if="task.detail" class="muted task_detail">{{ task.detail }}</p>

          <div class="task_progress">
            <!-- Progress bar for active tasks -->
            <ProgressBar v-if="isActive(task)" :value="task.progress" :label="task.name" />

            <div class="app_toolbar task_actions">
              <button
                v-if="isActive(task) && task.pausable"
                type="button"
                :disabled="busy"
                :title="t(task.state === 'running' ? 'pause' : 'resume')"
                :aria-label="`${t(task.state === 'running' ? 'pause' : 'resume')} ${task.name}`"
                @click="act(`/tasks/${task.id}/${task.state === 'running' ? 'pause' : 'resume'}`)"
              >
                <CommonIcon :src="task.state === 'running' ? pauseIcon : playIcon" />
              </button>

              <button
                v-if="isActive(task) && task.cancellable"
                type="button"
                :disabled="busy"
                :title="t('stop')"
                :aria-label="`${t('stop')} ${task.name}`"
                @click="act(`/tasks/${task.id}/stop`)"
              >
                <CommonIcon :src="stopIcon" />
              </button>

              <button
                v-if="!isActive(task)"
                type="button"
                :disabled="busy"
                :title="t('remove')"
                :aria-label="`${t('remove')} ${task.name}`"
                @click="act(`/tasks/${task.id}`, 'DELETE')"
              >
                <CommonIcon :src="deleteIcon" />
              </button>
            </div>
          </div>

          <p class="muted task_meta">{{ task.source }} · {{ formatDate(task.updatedAt) }}</p>
        </li>
      </ul>

      <CommonEmpty
        v-if="!shown.length && !loading && !error"
        :title="t(view === 'active' ? 'noTasks' : 'noFinishedTasks')"
        :icon="tasksIcon"
        testid="task_empty"
      />
    </section>
  </WidgetPanel>
</template>

<style lang="scss" scoped>

.task_heading {
  justify-content: flex-end;

  output {
    @extend %muted;
    margin-right: auto;
  }
}

.task_list {
  flex: 1;
  min-height: $space_10;
  overscroll-behavior: contain;

  &:focus-visible {
    @include focus-ring;
  }

  ul {
    @extend %stack;
    gap: $space_3;
  }
}

.task_row {
  @extend %stack;
  gap: $space_2;
  padding: $space_3;
  border-left: $space_2 solid var(--accent);
}

.task_over {
  border-left-color: var(--surface-border);
  opacity: 0.75;
}

.task_failed {
  border-left-color: var(--status-error);
  opacity: 1;
}

.task_paused {
  border-left-color: var(--status-warning);
}

.task_headline {
  @extend %fx_base;
  gap: $space_3;

  strong {
    margin-right: auto;
    overflow-wrap: anywhere;
  }
}

.task_badge {
  @extend %badge;
  flex-shrink: 0;
}

.task_failed .task_badge {
  border-color: var(--status-error);
  color: var(--status-error);
}

.task_detail,
.task_meta {
  margin: 0;
  overflow-wrap: anywhere;
}

.task_meta {
  font-size: $text_small;
}

.task_progress {
  @extend %fx_base;
  gap: $space_3;

  > :first-child:not(.task_actions) {
    flex: 1;
    min-width: 0;
  }
}

.task_actions {
  flex-shrink: 0;
  margin-left: auto;
}

</style>
