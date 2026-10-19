<script setup lang="ts">

import { t } from '@/i18n';
import { ref, watch, toRaw, computed, onBeforeUnmount, onMounted } from 'vue';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import { backgroundStyle, WALLPAPERS } from '@/apps/nas-system-settings/background';
import { THEMES, themeOf, type ThemeKind } from '@/apps/nas-system-settings/themes';
import type { DesktopIconSize, GeneralSettings } from '@/apps/nas-system-settings/types/general';

import SettingsTabLayout from '../SettingsTabLayout.vue';

const settings = useSettingsStore();
const draft = ref<GeneralSettings | null>(null);
const ICON_SIZES: DesktopIconSize[] = ['small', 'medium', 'large', 'xlarge'];
const patterns = ['halo', 'multiple', 'rings', 'dots', 'grid', 'stripes', 'linear'] as const;

const DIRECTIONS = [0, 45, 90, 135, 180, 225, 270, 315] as const;
const NAS_NAME = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?$/;
const HEX_COLOR = /^#[0-9a-fA-F]{6}$/;
const SAVE_DELAY_MS = 600;

const DEFAULT_ACCENT = '#3fd39b';

let timer: ReturnType<typeof setTimeout> | undefined;
let sent = '';

watch(
  () => settings.general,
  (value) => {
    if (value && (!draft.value || JSON.stringify(draft.value) === sent)) {
      draft.value = structuredClone(toRaw(value));
    }
  },
  { immediate: true },
);

const valid = computed(
  () =>
    Boolean(draft.value) &&
    NAS_NAME.test(draft.value!.nasName) &&
    HEX_COLOR.test(draft.value!.background.color),
);

async function save() {
  clearTimeout(timer);
  const current = draft.value;
  if (!current || !valid.value) {
    return;
  }
  const text = JSON.stringify(current);
  if (text === JSON.stringify(settings.general)) {
    return;
  }
  if (settings.saving) {
    timer = setTimeout(save, SAVE_DELAY_MS);
    return;
  }
  sent = text;
  await settings.save(structuredClone(toRaw(current)));
}

watch(
  draft,
  () => {
    clearTimeout(timer);
    timer = setTimeout(save, SAVE_DELAY_MS);
  },
  { deep: true },
);

onBeforeUnmount(() => void save());

const preview = computed(() => (draft.value ? backgroundStyle(draft.value.background) : {}));

const darkSystem = document.documentElement.dataset.theme === 'dark';
const kind = computed<ThemeKind>(() => {
  const chosen = draft.value?.theme ?? 'system';
  if (chosen === 'system') {
    return darkSystem ? 'dark' : 'light';
  }
  return chosen;
});

const themeName = computed({
  get: () => themeOf(kind.value, draft.value?.themes?.[kind.value]),
  set: (id: string) => {
    if (draft.value) {
      draft.value.themes = {
        ...(draft.value.themes ?? { light: 'default', dark: 'default' }),
        [kind.value]: id,
      };
    }
  },
});

onMounted(() => {
  if (!settings.general) {
    settings.load();
  }
});

</script>

<template>
  <SettingsTabLayout
    as="form"
    :title="t('general')"
    :ready="Boolean(draft)"
    :loading="!settings.error"
    :error="settings.error"
    @retry="settings.load"
    @submit.prevent="save"
  >
    <template v-if="draft">
      <p v-if="settings.error" class="app_error" role="alert">{{ settings.error }}</p>

      <div class="app_panel settings_group">
        <label class="settings_field settings_full">
          {{ t('nasName') }}

          <input
            v-model.trim="draft.nasName"
            name="nas_name"
            type="text"
            maxlength="63"
            pattern="[A-Za-z0-9](?:[A-Za-z0-9\-]{0,61}[A-Za-z0-9])?"
            class="nas_name_input"
            required
            data-testid="settings_nas_name"
          />
          <span class="muted">{{ t('nasNameHint') }}</span>
        </label>

        <div class="settings_fields">
          <label class="settings_field">
            {{ t('themeType') }}

            <select v-model="draft.theme" name="theme" data-testid="settings_theme">
              <option v-for="value in ['system', 'light', 'dark']" :key="value" :value="value">
                {{ t(value) }}
              </option>
            </select>
          </label>

          <label class="settings_field">
            {{ t('themeName') }}

            <select v-model="themeName" name="theme_name" data-testid="settings_theme_name">
              <option v-for="id in THEMES[kind]" :key="id" :value="id">
                {{ t(`theme_${id}`) }}
              </option>
            </select>
          </label>

          <p class="muted settings_full">{{ t('themeHint') }}</p>

          <!-- A row of its own, under the theme. -->
          <label class="settings_field settings_first_column">
            {{ t('language') }}

            <select v-model="draft.language" name="language" data-testid="settings_language">
              <option value="it">Italiano</option>
              <option value="en">English</option>
            </select>
          </label>
        </div>

        <fieldset class="settings_field settings_full choice_group">
          <legend>{{ t('desktopIconSize') }}</legend>
          <div class="choice_cards" data-testid="settings_desktop_icon_size">
            <label
              v-for="value in ICON_SIZES"
              :key="value"
              class="choice_card"
              :class="{ chosen: draft.desktopIconSize === value }"
            >
              <input
                v-model="draft.desktopIconSize"
                type="radio"
                name="desktop_icon_size"
                :value="value"
                :data-testid="`settings_desktop_icon_size_${value}`"
              />
              <span class="icon_window" aria-hidden="true">
                <span class="icon_sample" :class="`icon_sample_${value}`"></span>
              </span>
              <span>{{ t(`iconSize_${value}`) }}</span>
            </label>
          </div>
        </fieldset>

        <div class="settings_full">
          <div class="color_row">
            <span id="accent_color_label">{{ t('accentColor') }}</span>
            <input
              :value="draft.accent || DEFAULT_ACCENT"
              name="accent"
              type="color"
              class="color_swatch"
              :aria-labelledby="'accent_color_label'"
              data-testid="settings_accent"
              @input="draft.accent = ($event.target as HTMLInputElement).value"
            />
            <button type="button" :disabled="!draft.accent" @click="draft.accent = ''">
              {{ t('useThemeAccent') }}
            </button>
          </div>
          <p class="muted accent_hint">{{ t('accentHint') }}</p>
        </div>
      </div>

      <section class="app_panel settings_group">
        <h2>{{ t('background') }}</h2>

        <div class="background_preview" aria-hidden="true">
          <div :style="preview"></div>
        </div>

        <div class="settings_fields">
          <label class="settings_field">
            {{ t('background') }}

            <select
              v-model="draft.background.mode"
              name="background_mode"
              data-testid="settings_background_mode"
            >
              <option value="gradient">{{ t('gradient') }}</option>
              <option value="image">{{ t('image') }}</option>
            </select>
          </label>

          <label v-if="draft.background.mode === 'image'" class="settings_field">
            {{ t('image') }}

            <select
              v-model="draft.background.image"
              name="background_image"
              data-testid="settings_background_image"
            >
              <option v-for="wallpaper in WALLPAPERS" :key="wallpaper.file" :value="wallpaper.file">
                {{ t(wallpaper.label) }}
              </option>
            </select>
          </label>

          <template v-else>
            <label class="settings_field">
              {{ t('pattern') }}

              <select
                v-model="draft.background.pattern"
                name="background_pattern"
                data-testid="settings_background_pattern"
              >
                <option v-for="value in patterns" :key="value" :value="value">
                  {{ t(value) }}
                </option>
              </select>
            </label>

            <div class="settings_full color_row">
              <span id="background_color_label">{{ t('color') }}</span>
              <input
                v-model="draft.background.color"
                type="color"
                name="background_color"
                class="color_swatch"
                :aria-labelledby="'background_color_label'"
                data-testid="settings_background_color"
              />
              <input
                v-model="draft.background.color"
                type="text"
                name="background_color_code"
                class="color_code"
                :aria-label="t('color')"
                maxlength="7"
                pattern="#[0-9a-fA-F]{6}"
                spellcheck="false"
              />
            </div>

            <fieldset class="settings_field settings_full choice_group">
              <legend>{{ t('direction') }}</legend>
              <div class="choice_cards choice_cards_directions">
                <label
                  v-for="angle in DIRECTIONS"
                  :key="angle"
                  class="choice_card"
                  :class="{ chosen: draft.background.direction === angle }"
                >
                  <input
                    v-model.number="draft.background.direction"
                    type="radio"
                    name="background_direction"
                    :value="angle"
                    :data-testid="`settings_background_direction_${angle}`"
                  />
                  <span
                    class="direction_arrow"
                    :style="{ transform: `rotate(${angle}deg)` }"
                    aria-hidden="true"
                    >↑</span
                  >
                  <span>{{ t(`direction_${angle}`) }}</span>
                </label>
              </div>
            </fieldset>

            <label class="settings_field range_field">
              <span class="range_label">
                {{ t('size') }} <output>{{ draft.background.size }}</output>
              </span>
              <input
                v-model.number="draft.background.size"
                name="background_size"
                type="range"
                min="10"
                max="100"
              />
            </label>
          </template>

          <label class="settings_field range_field">
            <span class="range_label">
              {{ t('intensity') }} <output>{{ draft.background.intensity }}%</output>
            </span>
            <input
              v-model.number="draft.background.intensity"
              name="background_intensity"
              type="range"
              min="0"
              max="100"
            />
          </label>

          <label class="settings_field range_field">
            <span class="range_label">
              {{ t('blur') }} <output>{{ draft.background.blur }}px</output>
            </span>
            <input
              v-model.number="draft.background.blur"
              name="background_blur"
              type="range"
              min="0"
              max="30"
            />
          </label>
        </div>
      </section>
    </template>
  </SettingsTabLayout>
</template>

<style scoped lang="scss">

.background_preview {
  @extend %panel;
  position: relative;
  height: $preview_height;
  overflow: hidden;
  background: var(--theme-background);

  div {
    position: absolute;
    inset: 0;
  }
}

.color_row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: $space_3;

  > span {
    margin-right: $space_2;
  }
}

.accent_hint {
  margin-top: $space_2;
}

.color_swatch {
  @extend %control;
  width: $space_10;
  height: $space_10;
  padding: $space_2 * 0.5;
  cursor: pointer;
}

.color_code {
  width: $metric_min_width;
  text-transform: uppercase;
}

.choice_group {
  margin: 0;
  padding: 0;
  border: 0;

  legend {
    margin-bottom: $space_2;
    padding: 0;
  }
}

.choice_cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax($metric_min_width, 1fr));
  gap: $space_3;
}

.choice_cards_directions {
  grid-template-columns: repeat(auto-fill, minmax($metric_min_width * 0.9, 1fr));
}

.choice_card {
  @extend %panel_sunken;
  display: grid;
  justify-items: center;
  gap: $space_2;
  padding: $space_3;
  border: $border_thin solid var(--surface-border);
  border-radius: $radius_control;
  font-size: $text_label;
  text-align: center;
  cursor: pointer;
  transition: border-color $duration_fast ease;

  input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  &:hover {
    border-color: var(--surface-muted);
  }

  &.chosen {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 $border_thin var(--accent);
  }

  &:focus-within {
    @include focus-ring;
  }
}

.icon_window {
  display: grid;
  place-items: center;
  width: $space_10 * 1.6;
  height: $space_10 * 1.2;
  border: $border_thin solid var(--surface-border);
  border-radius: $radius_control * 0.5;
  background: var(--surface);
}

.icon_sample {
  display: block;
  border-radius: 25%;
  background: var(--accent);

  &_small {
    width: $space_3;
    height: $space_3;
  }

  &_medium {
    width: $space_4;
    height: $space_4;
  }

  &_large {
    width: $space_5;
    height: $space_5;
  }

  &_xlarge {
    width: $space_6;
    height: $space_6;
  }
}

.direction_arrow {
  display: block;
  color: var(--accent);
  font-size: $text_large;
  font-weight: 700;
  line-height: 1;
}

.range_label {
  display: flex;
  justify-content: space-between;
  gap: $space_2;

  output {
    color: var(--surface-muted);
    font-variant-numeric: tabular-nums;
  }
}

</style>
