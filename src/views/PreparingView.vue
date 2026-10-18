<script setup lang="ts">

import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { t } from '@/i18n';
import { useSetupStore } from '@/stores/setupStore';
import { useAppearance } from '@/composables/useAppearance';
import { usePolling } from '@/composables/usePolling';
import type { SetupStep } from '@/types/types';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import SetupCard from '@/components/setup/SetupCard.vue';

import errorIcon from '@/assets/icons/status/error.svg';
import loadingIcon from '@/assets/icons/status/loading.svg';
import successIcon from '@/assets/icons/status/success.svg';

const POLL_INTERVAL = 2000;
const ICONS: Record<Exclude<SetupStep['state'], 'running'>, string> = {
  pending: loadingIcon,
  done: successIcon,
  failed: errorIcon,
};

const setup = useSetupStore();
const router = useRouter();
const restarting = ref(false);

const steps = computed(() => setup.state?.steps ?? []);
const ready = computed(() => setup.state?.ready === true);
const unconfigured = computed(() => setup.state?.completed === false);

useAppearance();

/**
 * Asks where the steps are, and goes on to the sign-in once all of them succeeded.
 */
async function refresh() {
  if (ready.value) {
    return;
  }

  try {
    const current = await setup.load();
    restarting.value = false;
    if (current.ready && current.steps.every((step) => step.state === 'done')) {
      await router.push({ name: 'login' });
    }
  } catch {
    restarting.value = true;
  }
}

usePolling(refresh, ref(POLL_INTERVAL));
onMounted(refresh);

</script>

<template>
  <main class="app_surface preparing_page" data-testid="preparing_page">
    <SetupCard
      v-if="unconfigured"
      :title="t('unconfiguredTitle')"
      :description="t('unconfiguredIntro')"
      logo
    />

    <SetupCard
      v-else
      :title="t('preparingTitle')"
      :description="t(ready ? 'preparingIncomplete' : 'preparingIntro')"
      logo
    >
      <CommonSpinner
        v-if="!ready"
        :label="t(restarting ? 'preparingRestart' : 'preparingWait')"
        centered
        testid="preparing_spinner"
      />

      <!-- Steps for each setup task -->
      <ul v-if="steps.length" class="app_list">
        <li
          v-for="step in steps"
          :key="step.id"
          class="preparing_step"
          :data-testid="`preparing_step_${step.id}`"
        >
          <CommonSpinner v-if="step.state === 'running'" size="small" compact />
          <CommonIcon
            v-else
            :src="ICONS[step.state]"
            size="small"
            :label="t(`task_${step.state}`)"
          />
          <span class="preparing_name">{{ step.name }}</span>
          <span v-if="step.state === 'failed'" class="preparing_detail">{{ step.detail }}</span>
        </li>
      </ul>

      <!-- Actions for when the setup is ready -->
      <template v-if="ready" #actions>
        <button
          type="button"
          class="primary"
          data-testid="preparing_continue"
          @click="router.push({ name: 'login' })"
        >
          {{ t('goToSignIn') }}
        </button>
      </template>
    </SetupCard>
  </main>
</template>

<style scoped lang="scss">

.preparing_page {
  @extend %centered_page;
}

.preparing_step {
  @extend %panel_sunken;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: $space_3;
  padding: $space_3;
}

.preparing_name {
  flex: 1;
}

.preparing_detail {
  flex-basis: 100%;
  color: var(--status-error);
  font-size: $text_label;
}

</style>
