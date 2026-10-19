<script setup lang="ts">

import { nextTick, useTemplateRef, watch } from 'vue';
import { t } from '@/i18n';
import addIcon from '@/assets/icons/navigation/add.svg';

import CommonIcon from '@/components/commons/CommonIcon.vue';

interface Props {
  title: string;
  formTitle: string;
  testid: string;
  busy?: boolean;
  ready?: boolean;
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), { busy: false, ready: true, disabled: false });

const open = defineModel<boolean>('open', { default: false });

const emit = defineEmits<{
  submit: [];
}>();

const form = useTemplateRef<HTMLElement>('form');

function submit() {
  if (!props.busy && props.ready) {
    emit('submit');
  }
}

function submitOnEnter(event: KeyboardEvent) {
  if (event.target instanceof HTMLInputElement) {
    event.preventDefault();
    submit();
  }
}

watch(open, async (opened) => {
  if (opened) {
    await nextTick();
    form.value?.querySelector<HTMLElement>('input, select, button')?.focus();
  }
});

</script>

<template>
  <section class="app_panel settings_group" :aria-label="title">
    <div class="settings_rule_header">
      <h2>{{ title }}</h2>

      <button
        v-if="!open"
        type="button"
        class="primary"
        :disabled="disabled"
        :data-testid="`${testid}_open`"
        @click="open = true"
      >
        <CommonIcon :src="addIcon" size="small" tinted />
        {{ t('add') }}
      </button>
    </div>

    <slot name="header"></slot>

    <Transition name="settings_rule">
      <fieldset
        v-if="open"
        ref="form"
        class="settings_rule_form"
        :aria-label="formTitle"
        :data-testid="`${testid}_form`"
        @keydown.enter="submitOnEnter"
      >
        <h3>{{ formTitle }}</h3>

        <div class="settings_fields">
          <slot name="fields"></slot>
        </div>

        <div class="app_toolbar settings_rule_actions">
          <button type="button" :data-testid="`${testid}_cancel`" @click="open = false">
            {{ t('cancel') }}
          </button>
          <button
            type="button"
            class="primary"
            :disabled="busy || !ready"
            :data-testid="`${testid}_add`"
            @click="submit"
          >
            {{ t('add') }}
          </button>
        </div>
      </fieldset>
    </Transition>

    <slot></slot>
  </section>
</template>

<style scoped lang="scss">

.settings_rule_header {
  @extend %fx_between_center;
  gap: $space_3;

  h2 {
    margin: 0;
  }
}

.settings_rule_form {
  @extend %stack;
  @extend %fieldset_reset;
  padding: $space_4;
  border: $border_thin solid var(--accent);
  border-radius: $radius_control;
  background: var(--surface-sunken);

  h3 {
    margin: 0;
  }
}

.settings_rule_actions {
  justify-content: flex-end;
}

.settings_rule-enter-active,
.settings_rule-leave-active {
  transition:
    opacity $duration_fast ease-out,
    transform $duration_fast ease-out;

  @media (prefers-reduced-motion: reduce) {
    transition: none;
  }
}

.settings_rule-enter-from,
.settings_rule-leave-to {
  opacity: 0;
  transform: translateY(-$space_2);
}

</style>
