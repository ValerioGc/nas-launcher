<script setup lang="ts">

import { useShellStore } from '@/stores/shellStore';
import { ref, watch } from 'vue';
import { t } from '@/i18n';
import closeIcon from '@/assets/icons/navigation/close.svg';
import chevronRightIcon from '@/assets/icons/navigation/chevron_right.svg';

import CommonIcon from '@/components/commons/CommonIcon.vue';

const shell = useShellStore();
const tabsScroll = ref<HTMLElement>();
const moreTabs = ref(false);

function updateOverflow() {
  const element = tabsScroll.value;
  moreTabs.value = !!element && element.scrollWidth - element.clientWidth - element.scrollLeft > 1;
}

watch(tabsScroll, (element, _previous, onCleanup) => {
  updateOverflow();
  if (!element) {
    return;
  }
  const observer = new ResizeObserver(updateOverflow);
  observer.observe(element);
  const list = element.querySelector('ul');
  if (list) {
    observer.observe(list);
  }
  onCleanup(() => observer.disconnect());
});

watch(() => shell.openApps, updateOverflow, { flush: 'post' });

</script>
<template>
  <nav
    v-if="shell.openApps.length"
    class="app_tabs"
    data-testid="app_tabs"
    :aria-label="t('openApps')"
  >
    <div ref="tabsScroll" class="app_tabs_scroll" @scroll="updateOverflow">
      <ul>
        <li
          v-for="app in shell.openApps"
          :key="app.id"
          class="app_tab"
          :class="{ active: shell.activeId === app.id }"
        >
          <button
            type="button"
            class="app_tab_open"
            :aria-pressed="shell.activeId === app.id"
            :data-testid="`app_tab_${app.id}`"
            @click="shell.open(app.id)"
          >
            <img :src="app.entry.icon" alt="" />{{ app.name }}
          </button>

          <button
            type="button"
            class="app_tab_close"
            :aria-label="t('closeApp', { name: app.name })"
            :title="t('close')"
            :data-testid="`app_tab_close_${app.id}`"
            :disabled="shell.closing.includes(app.id)"
            @click="shell.close(app.id)"
          >
            <CommonIcon :src="closeIcon" size="small" tinted />
          </button>
        </li>
      </ul>
    </div>
    <span v-if="moreTabs" class="app_tabs_overflow" data-testid="app_tabs_overflow">
      <CommonIcon :src="chevronRightIcon" :label="t('moreOpenApps')" tinted />
    </span>
  </nav>
</template>
<style scoped lang="scss">

.app_tabs {
  position: relative;
  min-width: 0;
  flex-shrink: 0;
  background: transparent;
  box-shadow: inset 0 -#{$border_thin} 0 var(--surface-border);

  ul {
    @extend %fx_base;
    align-items: stretch;
    gap: $space_2;
    width: max-content;
    min-width: 100%;
  }
}

.app_tabs_scroll {
  @extend %hidden_scrollbar;
  padding: $space_2 $space_2 0;
  overflow-x: auto;
}

.app_tabs_overflow {
  @extend %fx_center;
  position: absolute;
  top: $space_2;
  right: 0;
  bottom: 0;
  width: $space_10;
  pointer-events: none;
  background: linear-gradient(to right, transparent, var(--surface) 50%);
  color: var(--theme-foreground);
}

.app_tab {
  position: relative;
  flex-shrink: 0;
  border: $border_thin solid var(--surface-border);
  border-bottom-width: 0;
  border-radius: $radius_control $radius_control 0 0;
  background: var(--surface-sunken);
  color: var(--surface-muted);
  transition:
    background-color $duration_fast ease,
    color $duration_fast ease;

  &.active {
    background: var(--surface);
    border-color: var(--accent);
    color: var(--theme-foreground);

    .app_tab_open {
      font-weight: 650;
    }
  }

  &:not(.active) .app_tab_open img {
    opacity: 0.65;
    transition: opacity $duration_fast ease;
  }

  // Hovering only lightens a tab in the background: no edge grows or moves, and the active one,
  // joined to its window, stays as it is.
  &:not(.active):hover {
    background: var(--surface-hover);
    color: var(--theme-foreground);

    .app_tab_open img {
      opacity: 0.9;
    }
  }
}

.app_tab_open {
  @extend %button;
  // Room for the close button laid over the end of the tab, and a gap before it.
  padding-right: $space_10 + $space_4;
  min-width: $app_tab_min_width;
  border: 0;
  border-radius: inherit;
  background: transparent;
  white-space: nowrap;

  &:hover:not(:disabled) {
    background: transparent;
  }

  img {
    width: $icon_large;
    height: $icon_large;
  }
}

.app_tab_close {
  @extend %icon_button;
  position: absolute;
  top: 50%;
  right: $space_2;
  transform: translateY(-50%);
  min-height: $icon_large;
  padding: 0 $space_2;
  color: var(--surface-muted);

  &:hover,
  &:focus-visible {
    color: var(--theme-foreground);
    background: var(--surface-border);
  }
}

</style>
