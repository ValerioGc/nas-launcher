<script setup lang="ts">

import { computed, onMounted, ref, watch } from 'vue';
import { t } from '@/i18n';
import { useResource } from '@/composables/useResource';
import { request } from '@/api/client';
import type { PowerSettings, PowerRule } from '@/apps/nas-system-settings/types/power';
import type { TableColumn } from '@/types/types';

import deleteIcon from '@/assets/icons/navigation/delete.svg';
import startIcon from '@/assets/icons/system/power.svg';
import shutdownIcon from '@/assets/icons/system/shutdown.svg';
import restartIcon from '@/assets/icons/system/restart.svg';

import SettingsTabLayout from '../SettingsTabLayout.vue';
import SettingsRuleSection from '../SettingsRuleSection.vue';
import CommonEmpty from '@/components/commons/CommonEmpty.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSegmented from '@/components/commons/CommonSegmented.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';
import CommonTable from '@/components/commons/CommonTable.vue';

const MAX_RULES = 32;
const SCREEN_MINUTES = { min: 1, max: 240 };

const { data, loading, error, busy, act, load } = useResource<PowerSettings | null>(
  '/settings/power',
  null,
);

const simulated = ref<boolean | null>(null);
const saved = ref(false);
const adding = ref(false);

watch(
  () => data.value?.enabled,
  (enabled) => {
    if (!enabled) {
      adding.value = false;
    }
  },
);

const rule = ref<PowerRule>({
  day: 'everyday',
  time: '22:00',
  action: 'shutdown',
  active: true,
});

const days = [
  'everyday',
  'weekdays',
  'weekends',
  'monday',
  'tuesday',
  'wednesday',
  'thursday',
  'friday',
  'saturday',
  'sunday',
];

const actions = [
  { value: 'start', label: 'start', icon: startIcon },
  { value: 'shutdown', label: 'shutdown', icon: shutdownIcon },
  { value: 'restart', label: 'restart', icon: restartIcon },
];

const columns: TableColumn<PowerRule & { index: number }>[] = [
  { key: 'day', label: 'day' },
  { key: 'time', label: 'time' },
  { key: 'action', label: 'action' },
  { key: 'active', label: 'enabled' },
];

const rows = computed(() => data.value?.rules.map((item, index) => ({ ...item, index })) ?? []);

async function saveRules(rules: PowerRule[]) {
  if (data.value) {
    saved.value = await act('/settings/power', 'PUT', { ...data.value, rules });
  }
  return saved.value;
}

async function save() {
  saved.value = await act('/settings/power', 'PUT', data.value);
}

function saveScreen() {
  const minutes = data.value?.screenOffMinutes ?? 0;
  if (Number.isInteger(minutes) && minutes >= SCREEN_MINUTES.min && minutes <= SCREEN_MINUTES.max) {
    return save();
  }
}

async function add() {
  if (data.value && (await saveRules([...data.value.rules, { ...rule.value }]))) {
    adding.value = false;
  }
}

function remove(index: number) {
  return saveRules(data.value?.rules.filter((_, position) => position !== index) ?? []);
}

function toggle(index: number, active: boolean) {
  return saveRules(
    data.value?.rules.map((item, position) => (position === index ? { ...item, active } : item)) ??
      [],
  );
}

onMounted(async () => {
  try {
    simulated.value = (await request<{ simulated: boolean }>('/system/power')).simulated;
  } catch {
    simulated.value = null;
  }
});

</script>

<template>
  <SettingsTabLayout
    :title="t('power')"
    :ready="Boolean(data)"
    :loading="loading"
    :error="error"
    @retry="load"
  >
    <template v-if="data">
      <p v-if="simulated !== null" class="muted" data-testid="power_mode">
        {{ simulated ? t('mockAction') : t('powerReal') }}
      </p>

      <section class="app_panel settings_group" data-testid="power_screen">
        <h2>{{ t('screen') }}</h2>
        <label class="settings_inline">
          {{ t('screenOffMinutes') }}
          <input
            v-model.number="data.screenOffMinutes"
            name="screen_off_minutes"
            type="number"
            :min="SCREEN_MINUTES.min"
            :max="SCREEN_MINUTES.max"
            step="1"
            required
            :disabled="busy"
            data-testid="power_screen_minutes"
            @change="saveScreen"
          />
        </label>
        <p class="muted">{{ t('screenNote') }}</p>
      </section>

      <SettingsRuleSection
        v-model:open="adding"
        :title="t('powerRules')"
        :form-title="t('newPowerRule')"
        :busy="busy"
        :ready="Boolean(rule.time) && data.rules.length < MAX_RULES"
        :disabled="!data.enabled"
        testid="power_rule"
        @submit="add"
      >
        <template #header>
          <CommonSwitch
            v-model="data.enabled"
            :label="t('enabled')"
            testid="power_enabled"
            @update:model-value="save"
          />
        </template>

        <template #fields>
          <label class="settings_field">
            {{ t('day') }}
            <select v-model="rule.day" name="power_rule_day" data-testid="power_rule_day">
              <option v-for="day in days" :key="day" :value="day">{{ t(day) }}</option>
            </select>
          </label>

          <label class="settings_field">
            {{ t('time') }}
            <input
              v-model="rule.time"
              name="power_rule_time"
              type="time"
              required
              data-testid="power_rule_time"
            />
          </label>

          <div class="settings_field settings_full">
            {{ t('action') }}
            <CommonSegmented
              v-model="rule.action"
              :options="actions"
              :label="t('action')"
              testid="power_rule_action"
            />
          </div>
        </template>

        <CommonTable
          v-if="rows.length"
          :class="{ rules_off: !data.enabled }"
          :inert="!data.enabled"
          :aria-disabled="!data.enabled"
          :rows="rows"
          :columns="columns"
          row-key="index"
          testid="power_rules"
        >
          <template #cell_day="{ row }">{{ t(row.day) }}</template>
          <template #cell_action="{ row }">{{ t(row.action) }}</template>
          <template #cell_active="{ row }">
            <CommonSwitch
              :model-value="row.active"
              :name="`power_rule_active_${row.index}`"
              :label="`${t('enabled')} ${t(row.day)} ${row.time}`"
              hide-label
              @update:model-value="toggle(row.index, $event)"
            />
          </template>
          <template #actions="{ row }">
            <button
              type="button"
              class="app_icon_button"
              :disabled="busy"
              :aria-label="`${t('remove')} ${t(row.day)} ${row.time}`"
              :title="t('remove')"
              @click="remove(row.index)"
            >
              <CommonIcon :src="deleteIcon" size="small" />
            </button>
          </template>
        </CommonTable>

        <CommonEmpty
          v-else
          :title="t('powerNoRules')"
          :icon="startIcon"
          testid="power_rules_empty"
        />
      </SettingsRuleSection>
    </template>
  </SettingsTabLayout>
</template>
