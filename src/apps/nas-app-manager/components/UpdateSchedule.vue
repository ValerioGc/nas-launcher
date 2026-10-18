<script setup lang="ts">

import { watch } from 'vue';
import { t } from '@/i18n';
import type { ManagerSettings, UpdateFrequency } from '@/apps/nas-app-manager/types';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';
import refreshIcon from '@/assets/icons/system/refresh.svg';

const FREQUENCIES: UpdateFrequency[] = ['hourly', 'daily', 'weekly'];

const settings = defineModel<ManagerSettings>({ required: true });

watch(
  () => settings.value.checkOnOpen,
  (checking) => {
    if (!checking) {
      settings.value.updateOnOpen = false;
    }
  },
);

</script>

<template>
  <section class="app_panel app_card" :aria-label="t('autoUpdates')">
    <header class="app_card_header">
      <CommonIcon :src="refreshIcon" tinted />
      <div class="app_card_title">
        <h3>{{ t('autoUpdates') }}</h3>
        <p class="muted">{{ t('autoUpdatesNote') }}</p>
      </div>
    </header>

    <fieldset class="update_group">
      <legend>{{ t('periodicCheck') }}</legend>
      <div class="update_periodic">
        <CommonSwitch
          v-model="settings.autoCheck"
          :label="t('autoCheckUpdates')"
          testid="app_manager_auto_check"
        />
        <select
          v-model="settings.frequency"
          name="update_frequency"
          :aria-label="t('checkFrequency')"
          :disabled="!settings.autoCheck"
          data-testid="app_manager_frequency"
        >
          <option v-for="value in FREQUENCIES" :key="value" :value="value">
            {{ t(`frequency_${value}`) }}
          </option>
        </select>
      </div>
      <p class="muted">{{ t('autoCheckNote') }}</p>
    </fieldset>

    <fieldset class="update_group">
      <legend>{{ t('onManagerOpen') }}</legend>
      <CommonSwitch
        v-model="settings.checkOnOpen"
        :label="t('checkOnOpen')"
        testid="app_manager_check_on_open"
      />
      <CommonSwitch
        v-model="settings.updateOnOpen"
        :label="t('updateOnOpen')"
        :disabled="!settings.checkOnOpen"
        testid="app_manager_update_on_open"
      />
      <p class="muted">{{ t('onManagerOpenNote') }}</p>
    </fieldset>
  </section>
</template>

<style scoped lang="scss">

.update_group {
  @extend %fieldset_reset;
  @extend %stack;
  gap: $space_3;
  padding-top: $space_4;
  border-top: $border_thin solid var(--surface-border);

  legend {
    float: left;
    width: 100%;
    font-size: $text_label;
    font-weight: 650;
  }

  p {
    margin: 0;
    font-size: $text_label;
  }
}

.update_periodic {
  @extend %fx_between_center;
  flex-wrap: wrap;
  gap: $space_3;
}

</style>
