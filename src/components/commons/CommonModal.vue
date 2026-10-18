<script setup lang="ts">

import { t } from '@/i18n';
import { nextTick, ref, watch } from 'vue';

import CommonIcon from '@/components/commons/CommonIcon.vue';
import closeIcon from '@/assets/icons/navigation/close.svg';

interface Props {
  title: string;
  icon?: string;
  hideCloseButton?: boolean;
  fixedSize?: boolean;
  wide?: boolean;
}

const props = defineProps<Props>();

const open = defineModel<boolean>({ required: true });
const dialog = ref<HTMLElement | null>(null);
let lastFocused: HTMLElement | null = null;

/**
 * Traps the focus within the modal, ensuring that the user cannot tab out of it.
 * @param event The keyboard event triggered when the user presses a key within the modal.
 */
function trapFocus(event: KeyboardEvent) {
  const elements = Array.from(
    dialog.value?.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]',
    ) ?? [],
  ).filter((element) => element.getClientRects().length > 0);

  const first = elements[0];
  const last = elements.at(-1);
  if (!first) {
    event.preventDefault();
    return;
  }

  if (
    event.shiftKey &&
    (document.activeElement === first || document.activeElement === dialog.value)
  ) {
    event.preventDefault();
    last?.focus();
  } else if (
    !event.shiftKey &&
    (document.activeElement === last || document.activeElement === dialog.value)
  ) {
    event.preventDefault();
    first.focus();
  }
}

function close() {
  open.value = false;
}

/**
 * The focus is trapped within the modal while it is open, and returns to the last focused element when the modal is closed.
 */
watch(
  open,
  async (isOpen) => {
    if (isOpen) {
      lastFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      await nextTick();
      dialog.value?.focus();
      return;
    }
    lastFocused?.focus();
    lastFocused = null;
  },
  { immediate: true },
);

</script>

<template>
  <Teleport to="body">
    <Transition name="common_modal">
      <div v-if="open" class="common_modal_layer" data-testid="common_modal">
        <button
          type="button"
          class="common_modal_backdrop"
          :aria-label="t('closeWindow')"
          data-testid="common_modal_backdrop"
          @click="close"
        ></button>

        <dialog
          ref="dialog"
          open
          class="common_modal app_surface"
          :class="{ common_modal_fixed: props.fixedSize, common_modal_wide: props.wide }"
          aria-modal="true"
          :aria-label="props.title"
          tabindex="-1"
          @keydown.esc.stop="close"
          @keydown.tab="trapFocus"
        >
          <div class="common_modal_header">
            <h2>
              <CommonIcon v-if="props.icon" :src="props.icon" size="large" />
              {{ props.title }}
            </h2>
            <button
              v-if="!props.hideCloseButton"
              type="button"
              class="common_modal_close"
              data-testid="common_modal_close"
              @click="close"
            >
              <CommonIcon :src="closeIcon" :label="t('close')" size="small" />
            </button>
          </div>

          <div class="common_modal_content">
            <slot></slot>
          </div>

          <div v-if="$slots.footer" class="common_modal_footer">
            <slot name="footer" :close="close"></slot>
          </div>
        </dialog>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped lang="scss">

.common_modal_layer {
  position: fixed;
  inset: 0;
  @extend %fx_center;
  z-index: $z_modal;
  padding: $space_4;
  font-family: $font_ui;
}

.common_modal_backdrop {
  @extend %backdrop;
}

.common_modal {
  margin: auto;
  color: inherit;
  position: relative;
  width: 100%;
  max-width: $modal_width;
  max-height: calc(100svh - #{$space_8});
  overflow-y: auto;
  @extend %hidden_scrollbar;
  padding: $space_6;
  border: $border_thin solid var(--surface-border);
  border-radius: $radius_large;
  background: var(--surface);
  box-shadow: $shadow_overlay;

  &:focus-visible {
    @include focus-ring;
    outline-offset: -#{$focus_offset};
  }
}

.common_modal_fixed {
  display: flex;
  flex-direction: column;
  height: $modal_fixed_height;
  max-width: $modal_fixed_width;
  overflow: hidden;

  .common_modal_header,
  .common_modal_footer {
    flex-shrink: 0;
  }

  // It scrolls, so it would cut the focus ring of a control on its edge: the ring gets room, and
  // the content stays where it was.
  .common_modal_content {
    flex: 1;
    min-height: 0;
    margin: -($focus_width + $focus_offset);
    padding: $focus_width + $focus_offset;
    overflow: auto;
    @extend %hidden_scrollbar;
  }
}

.common_modal_wide {
  max-width: $modal_fixed_width;
}

.common_modal_close {
  @extend %icon_button;
}

.common_modal_header {
  @extend %fx_between_center;
  gap: $space_4;
  margin-bottom: $space_4;

  h2 {
    @extend %fx_base;
    gap: $space_3;
    font-size: $text_large;
    font-weight: 650;
  }
}

.common_modal_content {
  color: var(--surface-muted);
  font-size: $text_label;
  line-height: 1.5;
}

.common_modal_footer {
  display: flex;
  justify-content: flex-end;
  gap: $space_2;
  margin-top: $space_6;
}

// Modal transition animations *****************
.common_modal-enter-active,
.common_modal-leave-active {
  transition: opacity $duration_pop ease;

  .common_modal {
    transition: transform $duration_pop $ease_emphasized;
  }
}

.common_modal-enter-from,
.common_modal-leave-to {
  opacity: 0;

  .common_modal {
    transform: scale(0.96);
  }
}

@media (prefers-reduced-motion: reduce) {
  .common_modal-enter-from,
  .common_modal-leave-to {
    .common_modal {
      transform: none;
    }
  }
}

</style>
