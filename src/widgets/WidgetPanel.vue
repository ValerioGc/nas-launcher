<script setup lang="ts">

import { computed, onMounted, ref, watch } from 'vue';
import { useWidgetsStore } from '@/stores/widgetsStore';
import { t } from '@/i18n';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonSaveFooter from '@/components/commons/CommonSaveFooter.vue';
import closeIcon from '@/assets/icons/navigation/close.svg';
import settingsIcon from '@/assets/icons/system/settings.svg';

interface Props {
  title: string;
  testid: string;
}

const props = defineProps<Props>();
const emit = defineEmits<{ close: [] }>();

const widgets = useWidgetsStore();
const showSettings = ref(false);
const draft = ref(widgets.settings?.pollingSeconds ?? 10);
const saved = ref(false);
const polling = computed(() => widgets.settings?.pollingSeconds ?? 10);

watch(polling, (value) => {
  draft.value = value;
});

/**
 * Save the current polling settings for the widget.
 */
async function save() {
  saved.value = await widgets.save({ pollingSeconds: draft.value });
}

onMounted(() => {
  if (!widgets.settings) {
    widgets.load();
  }
});

</script>

<template>
  <section
    class="widget_panel app_surface app_stack"
    :aria-label="props.title"
    :data-testid="props.testid"
  >
    <div class="app_toolbar widget_heading">
      <h2>
        {{ showSettings ? t('widgetSettings') : props.title }}
      </h2>

      <button
        type="button"
        :aria-pressed="showSettings"
        :aria-label="t('widgetSettings')"
        :title="t('widgetSettings')"
        data-testid="widget_settings_toggle"
        @click="showSettings = !showSettings"
      >
        <CommonIcon :src="settingsIcon" />
      </button>
      <button
        type="button"
        :aria-label="t('close')"
        :title="t('close')"
        data-testid="widget_close"
        @click="emit('close')"
      >
        <CommonIcon :src="closeIcon" />
      </button>
    </div>

    <form v-if="showSettings" class="widget_settings app_panel" @submit.prevent="save">
      <label class="settings_field">
        <span>{{ t('pollingSeconds') }}</span>

        <input
          v-model.number="draft"
          name="widget_refresh_seconds"
          type="number"
          min="2"
          max="300"
          required
          data-testid="widget_polling"
          @input="saved = false"
        />
      </label>

      <p class="muted">{{ t('pollingHint') }}</p>

      <CommonSaveFooter :saved="saved" :busy="widgets.saving" testid="widget_settings_save">
        <AlertBanner v-if="widgets.error" tone="error" compact>{{ widgets.error }}</AlertBanner>
      </CommonSaveFooter>
    </form>

    <slot v-else></slot>
  </section>
</template>

<style scoped lang="scss">

.widget_panel {
  height: 100%;
  min-height: 0;
  padding: $space_4;
}

.widget_heading h2 {
  margin-right: auto;
}

.widget_settings {
  @extend %stack;
  gap: $space_3;
  padding: $space_4;

  input {
    width: 100%;
  }
}

</style>
