<script lang="ts" setup>

import { t } from '@/i18n';
import { computed } from 'vue';
import type { AlertTone } from '@/types/types';

import CommonIcon from '@/components/commons/CommonIcon.vue';

import infoIcon from '@/assets/icons/status/info.svg';
import successIcon from '@/assets/icons/status/success.svg';
import warningIcon from '@/assets/icons/status/warning.svg';
import errorIcon from '@/assets/icons/status/error.svg';
import closeIcon from '@/assets/icons/navigation/close.svg';

interface Props {
  title?: string;
  tone?: AlertTone;
  dismissible?: boolean;
  compact?: boolean;
}

const props = withDefaults(defineProps<Props>(), { tone: 'info', title: undefined });

defineEmits<{
  dismiss: [];
}>();

const toneIcons: Record<AlertTone, string> = {
  info: infoIcon,
  success: successIcon,
  warning: warningIcon,
  error: errorIcon,
};

const toneLabels: Record<AlertTone, string> = {
  info: 'toneInfo',
  success: 'toneSuccess',
  warning: 'toneWarning',
  error: 'toneError',
};

const icon = computed(() => toneIcons[props.tone]);
const iconLabel = computed(() => t(toneLabels[props.tone]));

</script>

<template>
  <div
    class="alert_banner"
    :class="[`alert_banner_${props.tone}`, { alert_banner_compact: props.compact }]"
    :role="props.tone === 'error' ? 'alert' : 'status'"
    data-testid="alert_banner"
  >
    <CommonIcon class="alert_banner_icon" :src="icon" :label="iconLabel" />

    <div class="alert_banner_body">
      <h3 v-if="props.title">{{ props.title }}</h3>

      <slot></slot>
    </div>

    <button
      v-if="props.dismissible"
      type="button"
      class="alert_banner_close"
      data-testid="alert_banner_close"
      @click="$emit('dismiss')"
    >
      <CommonIcon :src="closeIcon" :label="t('close')" size="small" />
    </button>
  </div>
</template>

<style scoped lang="scss">

.alert_banner {
  display: flex;
  align-items: flex-start;
  gap: $space_3;
  padding: $space_4;
  border: $border_thin solid color-mix(in srgb, var(--alert-tone) 45%, transparent);
  border-radius: $radius_control;
  background: color-mix(in srgb, var(--alert-tone) 12%, var(--surface));
  font-size: $text_label;
  line-height: 1.5;

  .alert_banner_icon {
    color: var(--alert-tone);
  }

  .alert_banner_body {
    flex: 1;

    h3 {
      margin-bottom: $space_2;
      font-size: $text_label;
      font-weight: 650;
    }
  }

  .alert_banner_close {
    @extend %icon_button;
  }

  &.alert_banner_compact {
    align-items: center;
    gap: $space_2;
    padding: $space_2 $space_3;
  }
}

.alert_banner_info {
  --alert-tone: var(--status-info);
}

.alert_banner_success {
  --alert-tone: var(--status-success);
}

.alert_banner_warning {
  --alert-tone: var(--status-warning);
}

.alert_banner_error {
  --alert-tone: var(--status-error);
}

</style>
