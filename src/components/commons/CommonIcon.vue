<script setup lang="ts">

interface Props {
  src: string;
  label?: string;
  size?: 'small' | 'medium' | 'large';
  tinted?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  size: 'medium',
  label: undefined,
  tinted: false,
});

</script>

<template>
  <span
    class="common_icon"
    :class="[`common_icon_${props.size}`, { common_icon_tinted: props.tinted }]"
    :style="{ '--icon-source': `url(${JSON.stringify(props.src)})` }"
    :role="props.label ? 'img' : undefined"
    :aria-label="props.label"
    :aria-hidden="props.label ? undefined : 'true'"
    data-testid="common_icon"
  ></span>
</template>

<style scoped lang="scss">

.common_icon {
  display: inline-block;
  flex-shrink: 0;
  background: var(--icon-source) center / contain no-repeat;

  &.common_icon_small {
    width: $icon_small;
    height: $icon_small;
  }

  &.common_icon_medium {
    width: $icon_medium;
    height: $icon_medium;
  }

  &.common_icon_large {
    width: $icon_large;
    height: $icon_large;
  }

  &.common_icon_tinted {
    background: currentcolor;
    mask: var(--icon-source) center / contain no-repeat;
  }
}

</style>
