<script setup lang="ts">

import { t } from '@/i18n';

interface Props {
  id: string;
  min: number;
  max: number;
  step: number;
}

const props = defineProps<Props>();
const value = defineModel<number>({ required: true });

const decimals = (String(props.step).split('.')[1] ?? '').length;

function set(next: number) {
  if (Number.isNaN(next)) {
    return;
  }
  value.value = Number(Math.min(props.max, Math.max(props.min, next)).toFixed(decimals));
}

function typed(event: Event) {
  set(Number((event.target as HTMLInputElement).value));
}

</script>

<template>
  <span class="terminal_stepper">
    <button
      type="button"
      :aria-label="t('decrease')"
      :disabled="value <= props.min"
      :data-testid="`${props.id}_decrease`"
      @click="set(value - props.step)"
    >
      −
    </button>
    <input
      :id="props.id"
      :value="value"
      type="number"
      :min="props.min"
      :max="props.max"
      :step="props.step"
      inputmode="decimal"
      :data-testid="props.id"
      @change="typed"
    />
    <button
      type="button"
      :aria-label="t('increase')"
      :disabled="value >= props.max"
      :data-testid="`${props.id}_increase`"
      @click="set(value + props.step)"
    >
      +
    </button>
  </span>
</template>

<style scoped lang="scss">

.terminal_stepper {
  display: flex;
  gap: $space_2;

  input {
    flex: 1;
    text-align: center;
    appearance: textfield;

    &::-webkit-inner-spin-button,
    &::-webkit-outer-spin-button {
      -webkit-appearance: none;
      margin: 0;
    }
  }
}

</style>
