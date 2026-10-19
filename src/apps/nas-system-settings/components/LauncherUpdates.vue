<script setup lang="ts">

import { computed, ref } from 'vue';
import { useResource } from '@/composables/useResource';
import { request } from '@/api/client';
import { t } from '@/i18n';
import type { NasUpdates } from '../types/update';

import LauncherUpdateModal from './LauncherUpdateModal.vue';
import SettingsStatus from './SettingsStatus.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import successIcon from '@/assets/icons/status/success.svg';
import errorIcon from '@/assets/icons/status/error.svg';
import { failureMessage } from '@/utils/failureMessage';

const { data, loading, error, load } = useResource<NasUpdates | null>('/launcher/updates', null);

const checking = ref(false);
const checkError = ref('');
const updating = ref(false);

const repository = computed(
  () => data.value?.components.find((item) => item.component === 'launcher')?.repository ?? '',
);

const pending = computed(
  () => data.value?.components.filter((item) => item.availableVersion) ?? [],
);

/**
 * Asks the repository of every component for its latest release.
 */
async function check() {
  checking.value = true;
  checkError.value = '';
  try {
    await request<NasUpdates>('/launcher/updates/check', { method: 'POST', body: '{}' });
    await load();
  } catch (error_) {
    checkError.value = failureMessage(error_);
  } finally {
    checking.value = false;
  }
}

</script>

<template>
  <section
    class="app_panel settings_group"
    :aria-label="t('launcherUpdates')"
    data-testid="launcher_updates"
  >
    <header class="settings_heading">
      <h2>{{ t('launcherUpdates') }}</h2>
      <button
        v-if="pending.length"
        type="button"
        class="primary"
        :disabled="loading"
        data-testid="launcher_update"
        @click="updating = true"
      >
        {{ t('updateLauncher') }}
      </button>
      <button
        v-else
        type="button"
        :disabled="loading || checking || !repository"
        data-testid="launcher_check"
        @click="check"
      >
        <CommonSpinner v-if="checking" size="small" compact />
        {{ t('checkUpdates') }}
      </button>
    </header>

    <p class="muted">{{ t('launcherUpdatesDescription') }}</p>

    <ul v-if="data?.components.length" class="component_list" data-testid="nas_components">
      <li
        v-for="item in data.components"
        :key="item.component"
        :data-testid="`nas_component_${item.component}`"
      >
        <span class="component_name">{{ t(`nasComponent_${item.component}`) }}</span>
        <span class="component_versions">
          <span :class="{ muted: !item.currentVersion }">{{ item.currentVersion ?? '-' }}</span>
          <template v-if="item.availableVersion">
            <span class="update_dot" :title="t('availableVersion')" aria-hidden="true"></span>
            <span class="update_version" :aria-label="t('availableVersion')">{{
              item.availableVersion
            }}</span>
          </template>
        </span>
      </li>
    </ul>

    <p v-if="data" class="token_state" data-testid="launcher_token">
      <span class="muted">{{ t('launcherToken') }}</span>
      <CommonIcon
        :src="data.tokenConfigured ? successIcon : errorIcon"
        :label="t(data.tokenConfigured ? 'launcherTokenSet' : 'launcherTokenMissing')"
        size="small"
      />
    </p>

    <!-- Status messages for loading and errors -->
    <SettingsStatus :loading="loading" :error="error || checkError" />

    <p v-if="data && !repository" class="muted" data-testid="launcher_no_source">
      {{ t('launcherNoRepository') }}
    </p>

    <LauncherUpdateModal
      v-if="updating && pending.length"
      v-model="updating"
      :components="pending"
    />
  </section>
</template>

<style scoped lang="scss">

.component_list {
  display: grid;
  margin: 0;
  padding: 0;
  list-style: none;
  gap: $space_2;

  li {
    @extend %panel_sunken;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: $space_2 $space_4;
    padding: $space_3 $space_4;
    border-radius: $radius_control;
  }
}

.component_name {
  font-weight: 600;
}

.component_versions {
  display: inline-flex;
  align-items: center;
  gap: $space_2;
  font-variant-numeric: tabular-nums;
}

.update_dot {
  width: $space_2;
  height: $space_2;
  border-radius: 50%;
  background: var(--status-success);
}

.update_version {
  color: var(--status-success);
  font-weight: 600;
}

.token_state {
  display: flex;
  align-items: center;
  gap: $space_2;
  margin: 0;
}

</style>
