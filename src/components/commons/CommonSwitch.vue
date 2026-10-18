<script setup lang="ts">

interface Props {
  label: string;
  testid?: string;
  name?: string;
  disabled?: boolean;
  hideLabel?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  testid: undefined,
  name: undefined,
  disabled: false,
  hideLabel: false,
});

const checked = defineModel<boolean>({ default: false });

</script>

<template>
  <label
    class="common_switch"
    :class="{ common_switch_disabled: props.disabled }"
    :data-testid="props.testid"
  >
    <input
      v-model="checked"
      type="checkbox"
      :name="props.name ?? props.testid"
      :disabled="props.disabled"
      :data-testid="props.testid ? `${props.testid}_input` : undefined"
    />
    <span class="common_switch_track"></span>
    <span class="common_switch_label" :class="{ visually_hidden: props.hideLabel }">
      {{ props.label }}
    </span>
  </label>
</template>

<style scoped lang="scss">

.common_switch {
  @extend %fx_base;
  gap: $space_3;
  cursor: pointer;

  &_disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  input {
    @extend %visually_hidden;

    &:checked + .common_switch_track {
      background: var(--accent);

      &::after {
        transform: translate(calc(#{$space_8} - #{$space_5}), -50%);
      }
    }

    &:focus-visible + .common_switch_track {
      @include focus-ring;
    }
  }

  &_track {
    position: relative;
    flex-shrink: 0;
    width: $space_8;
    height: $space_5;
    border-radius: $radius_large;
    background: var(--surface-border);
    transition: background $duration_fast ease;

    // Knob of the switch.
    &::after {
      content: '';
      position: absolute;
      top: 50%;
      left: $border_thin;
      width: calc(#{$space_5} - #{$space_2} * 0.5);
      height: calc(#{$space_5} - #{$space_2} * 0.5);
      border-radius: 50%;
      background: var(--surface);
      transform: translateY(-50%);
      transition: transform $duration_fast ease;
    }
  }

  &_label {
    font-size: $text_label;
  }
}

@media (prefers-reduced-motion: reduce) {
  .common_switch_track,
  .common_switch_track::after {
    transition: none;
  }
}

</style>
