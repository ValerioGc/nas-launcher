<script lang="ts" setup>

import { ref } from 'vue';
import { t } from '@/i18n';

import CommonIcon from '@/components/commons/CommonIcon.vue';

import search from '@/assets/icons/navigation/search.svg';
import enter from '@/assets/icons/navigation/enter.svg';

const emit = defineEmits<{
  search: [query: string];
}>();

const query = ref('');

function submit() {
  emit('search', query.value.trim());
  query.value = '';
}

</script>

<template>
  <form class="header_search" role="search" @submit.prevent="submit">
    <div class="header_search_field">
      <CommonIcon class="header_search_icon" :src="search" size="small" />

      <input
        v-model="query"
        name="header_search"
        type="search"
        class="header_search_input"
        autocapitalize="none"
        :spellcheck="false"
        :placeholder="t('searchAllPlaceholder')"
        :aria-label="t('searchAll')"
        data-testid="header_search_input"
      />

      <button
        type="submit"
        class="header_search_submit"
        :aria-label="t('startSearch')"
        data-testid="header_search_submit"
      >
        <CommonIcon :src="enter" size="small" />
      </button>
    </div>

    <button
      type="submit"
      class="header_search_compact"
      :aria-label="t('search')"
      data-testid="header_search_compact"
    >
      <CommonIcon :src="search" size="large" />
    </button>
  </form>
</template>

<style lang="scss" scoped>

.header_search {
  @extend %fx_base;

  &_field {
    position: relative;
    @extend %fx_base;

    @media (max-width: $breakpoint_medium) {
      display: none;
    }

    .header_search_icon {
      position: absolute;
      left: $space_3;
      color: var(--surface-muted);
      pointer-events: none;
    }

    .header_search_input {
      width: $header_search_width;
      min-width: 0;
      min-height: $space_8;
      padding: 0 ($space_2 + $header_search_action) 0 ($space_3 + $icon_small + $space_2);
      border: $border_thin solid var(--surface-border);
      border-radius: $radius_control;
      background: var(--surface-sunken);
      color: inherit;
      font: inherit;
      font-size: $text_label;

      &::placeholder {
        color: var(--surface-muted);
      }

      &:focus-visible {
        @include focus-ring;
      }

      &::-webkit-search-cancel-button {
        display: none;
      }
    }

    // Inside the border of the field, not on top of it: same height, same corners.
    .header_search_submit {
      position: absolute;
      top: $border_thin;
      right: $border_thin;
      bottom: $border_thin;
      @extend %fx_center;
      width: $header_search_action;
      padding: 0;
      margin: 0;
      border: 0;
      border-left: $border_thin solid var(--surface-border);
      border-radius: 0 calc(#{$radius_control} - #{$border_thin})
        calc(#{$radius_control} - #{$border_thin}) 0;
      background: var(--surface-hover);
      color: var(--surface-muted);
      cursor: pointer;
      transition:
        background $duration_fast ease,
        color $duration_fast ease;

      &:hover {
        color: inherit;
      }

      &:focus-visible {
        @include focus-ring;
      }
    }
  }
}

// Compact search button for small screens
.header_search_compact {
  @extend %icon_button;
  display: none;

  @media (max-width: $breakpoint_medium) {
    display: flex;
  }
}

</style>
