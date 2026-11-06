<script setup lang="ts">

import { ref } from 'vue';
import { t } from '@/i18n';

import CommonField from '@/components/commons/CommonField.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonPasswordInput from '@/components/commons/CommonPasswordInput.vue';
import successIcon from '@/assets/icons/status/success.svg';
import infoIcon from '@/assets/icons/status/info.svg';

interface Props {
  configured: boolean;
}

const props = defineProps<Props>();
const token = defineModel<string>({ required: true });
const editing = ref(false);

</script>

<template>
  <section class="app_panel app_card" :aria-label="t('tokenLabel')">
    <header class="app_card_header">
      <CommonIcon
        :src="props.configured ? successIcon : infoIcon"
        :label="t(props.configured ? 'tokenConfigured' : 'tokenNotConfigured')"
        data-testid="app_manager_token_status"
      />
      <div class="app_card_title">
        <h3>{{ t('tokenLabel') }}</h3>
        <p class="muted">{{ t('tokenUse') }}</p>
      </div>
      <button
        v-if="props.configured && !editing"
        type="button"
        data-testid="app_manager_token_edit"
        @click="editing = true"
      >
        {{ t('editApp') }}
      </button>
    </header>

    <template v-if="!props.configured || editing">
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

      <p v-if="props.configured" class="muted token_note" data-testid="app_manager_token_note">
        {{ t('tokenReplace') }}
      </p>
    </template>
  </section>
</template>

<style scoped lang="scss">

.token_note {
  margin: 0;
  font-size: $text_label;
}

</style>
