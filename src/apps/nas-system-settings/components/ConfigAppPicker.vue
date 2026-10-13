<script setup lang="ts">

import { computed } from 'vue';
import { t } from '@/i18n';
import { useAppsStore } from '@/stores/appsStore';

interface Props {
  /** The applications whose settings can go in the file. */
  exportable: readonly string[];
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), { disabled: false });

const selected = defineModel<string[]>({ required: true });

const apps = useAppsStore();

/** Every application of the NAS: the ones with settings to export first, then by name. */
const rows = computed(() =>
  apps.apps
    .map((app) => ({ id: app.id, name: app.name, exportable: props.exportable.includes(app.id) }))
    .sort(
      (left, right) =>
        Number(right.exportable) - Number(left.exportable) || left.name.localeCompare(right.name),
    ),
);

function toggle(id: string, chosen: boolean) {
  selected.value = chosen
    ? [...selected.value, id]
    : selected.value.filter((current) => current !== id);
}

</script>

<template>
  <fieldset class="config_app_picker" :disabled="disabled" data-testid="config_apps">
    <legend>{{ t('configApps') }}</legend>
    <p class="muted">{{ t('configAppsNote') }}</p>
    <ul class="config_app_list">
      <li v-for="app in rows" :key="app.id">
        <label :class="{ muted: !app.exportable }">
          <input
            type="checkbox"
            :name="`config_app_${app.id}`"
            :checked="selected.includes(app.id)"
            :disabled="!app.exportable"
            :data-testid="`config_app_${app.id}`"
            @change="toggle(app.id, ($event.target as HTMLInputElement).checked)"
          />
          <span class="config_app_name">{{ app.name }}</span>
          <small v-if="!app.exportable">{{ t('configAppNoSettings') }}</small>
        </label>
      </li>
    </ul>
  </fieldset>
</template>

<style scoped lang="scss">

.config_app_picker {
  display: grid;
  gap: $space_2;
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;

  legend {
    padding: 0;
    font-weight: 600;
  }

  p {
    margin: 0;
  }
}

.config_app_list {
  @extend %panel;
  max-height: $space_10 * 4;
  margin: 0;
  padding: $space_2;
  overflow-y: auto;
  list-style: none;

  label {
    display: flex;
    align-items: center;
    gap: $space_2;
    padding: $space_2;
  }

  small {
    margin-left: auto;
  }
}

.config_app_name {
  @extend %truncated_text;
}

</style>
