<script setup lang="ts">

import { t } from '@/i18n';

import CommonField from '@/components/commons/CommonField.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonPasswordInput from '@/components/commons/CommonPasswordInput.vue';
import lockIcon from '@/assets/icons/network/lock.svg';

interface Props {
  configured: boolean;
}

const props = defineProps<Props>();
const token = defineModel<string>({ required: true });

</script>

<template>
  <section class="app_panel app_card" :aria-label="t('tokenLabel')">
    <header class="app_card_header">
      <CommonIcon :src="lockIcon" tinted />
      <div class="app_card_title">
        <h3>{{ t('tokenLabel') }}</h3>
        <p class="muted">{{ t('tokenUse') }}</p>
      </div>
    </header>

    <CommonField>
      <label for="app_manager_token">{{ t('tokenField') }}</label>
      <CommonPasswordInput
        id="app_manager_token"
        v-model="token"
        spellcheck="false"
        maxlength="512"
        data-testid="app_manager_token_input"
      />
    </CommonField>

    <p class="muted token_note" data-testid="app_manager_token_note">
      {{ t(props.configured ? 'tokenReplace' : 'tokenWriteOnly') }}
    </p>
  </section>
</template>

<style scoped lang="scss">

.token_note {
  margin: 0;
  font-size: $text_label;
}

</style>
