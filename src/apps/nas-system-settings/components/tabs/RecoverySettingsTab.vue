<script setup lang="ts">

import { t } from '@/i18n';

import ConfigTransfer from '../ConfigTransfer.vue';
import FactoryReset from '../FactoryReset.vue';
import SettingsTabLayout from '../SettingsTabLayout.vue';

</script>

<template>
  <SettingsTabLayout as="section" :title="t('recovery')">
    <p class="muted">{{ t('recoveryNote') }}</p>

    <ConfigTransfer />

    <FactoryReset />
  </SettingsTabLayout>
</template>
