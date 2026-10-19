<script setup lang="ts">

import { computed, onMounted, ref, useTemplateRef, watch } from 'vue';
import { t } from '@/i18n';
import { appIcon } from '@/apps/registry';
import { APP_CATEGORIES, LOGO_MAX_BYTES, LOGO_TYPES } from '@/apps/nas-app-manager/types';
import type { CatalogueEntry, Provider } from '@/apps/nas-app-manager/types';

import CommonField from '@/components/commons/CommonField.vue';

interface Props {
  existing: boolean;
}

const PROVIDERS: Provider[] = ['github', 'gitlab'];

const props = defineProps<Props>();
const entry = defineModel<CatalogueEntry>({ required: true });
const logo = defineModel<File | null | undefined>('logo');
const emit = defineEmits<{ apply: [] }>();
const form = useTemplateRef<HTMLFormElement>('form');
const logoError = ref('');
const chosen = ref('');

const preview = computed(() => {
  if (logo.value === null) {
    return appIcon(entry.value.id);
  }
  return chosen.value || appIcon(entry.value.id, entry.value.logo);
});

const hasLogo = computed(() => logo.value !== null && Boolean(logo.value || entry.value.logo));

function dataUrl(file: File) {
  return new Promise<string>((resolve) => {
    const reader = new FileReader();
    reader.onload = () => resolve(typeof reader.result === 'string' ? reader.result : '');
    reader.readAsDataURL(file);
  });
}

watch(
  logo,
  async (file) => {
    chosen.value = file ? await dataUrl(file) : '';
  },
  { immediate: true },
);

function choose(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = '';
  if (!file) {
    return;
  }
  if (!LOGO_TYPES.includes(file.type)) {
    logoError.value = t('appLogoFormat');
  } else if (file.size > LOGO_MAX_BYTES) {
    logoError.value = t('appLogoTooLarge');
  } else {
    logoError.value = '';
    logo.value = file;
  }
}

onMounted(() => {
  form.value?.parentElement?.scrollTo?.({ top: 0 });
  form.value?.querySelector<HTMLElement>('input:not(:disabled)')?.focus({ preventScroll: true });
});

</script>

<template>
  <form
    id="catalogue_entry"
    ref="form"
    class="app_stack catalogue_entry"
    data-testid="catalogue_entry"
    @submit.prevent="emit('apply')"
  >
    <div class="catalogue_fields">
      <CommonField>
        <label for="catalogue_id">{{ t('appId') }}</label>
        <input
          id="catalogue_id"
          v-model.trim="entry.id"
          required
          maxlength="64"
          pattern="[A-Za-z0-9_\-]+"
          :disabled="props.existing"
          data-testid="catalogue_id"
        />
      </CommonField>

      <CommonField>
        <label for="catalogue_name">{{ t('appName') }}</label>
        <input
          id="catalogue_name"
          v-model="entry.name"
          required
          maxlength="60"
          data-testid="catalogue_name"
        />
      </CommonField>

      <CommonField>
        <label for="catalogue_category">{{ t('appCategory') }}</label>
        <select id="catalogue_category" v-model="entry.category" data-testid="catalogue_category">
          <option :value="null">{{ t('noCategory') }}</option>
          <option v-for="category in APP_CATEGORIES" :key="category" :value="category">
            {{ t(`category_${category}`) }}
          </option>
        </select>
      </CommonField>

      <CommonField>
        <label for="catalogue_provider">{{ t('appProvider') }}</label>
        <select id="catalogue_provider" v-model="entry.provider" data-testid="catalogue_provider">
          <option v-for="provider in PROVIDERS" :key="provider" :value="provider">
            {{ provider === 'github' ? 'GitHub' : 'GitLab' }}
          </option>
        </select>
      </CommonField>

      <CommonField>
        <label for="catalogue_repository">{{ t('appRepository') }}</label>
        <input
          id="catalogue_repository"
          v-model.trim="entry.repository"
          required
          maxlength="200"
          pattern="[\w.\-]+(/[\w.\-]+)+"
          data-testid="catalogue_repository"
        />
      </CommonField>

      <CommonField>
        <label for="catalogue_asset">{{ t('appAsset') }}</label>
        <input
          id="catalogue_asset"
          v-model.trim="entry.asset"
          required
          maxlength="200"
          :placeholder="t('appAssetHint')"
          data-testid="catalogue_asset"
        />
      </CommonField>

      <CommonField class="catalogue_logo_field" :error="logoError">
        <span>{{ t('appLogo') }}</span>
        <div class="catalogue_logo">
          <img :src="preview" alt="" data-testid="catalogue_logo_preview" />
          <div class="catalogue_logo_details">
            <dl>
              <div>
                <dt>{{ t('appLogoFormats') }}</dt>
                <dd>{{ t('appLogoFormatsValue') }}</dd>
              </div>
              <div>
                <dt>{{ t('appLogoMaxSize') }}</dt>
                <dd>{{ t('appLogoMaxSizeValue', { size: LOGO_MAX_BYTES / 1024 }) }}</dd>
              </div>
            </dl>
            <div class="app_toolbar">
              <input
                id="catalogue_logo"
                type="file"
                class="visually_hidden"
                :accept="LOGO_TYPES.join(',')"
                data-testid="catalogue_logo"
                @change="choose"
              />
              <label for="catalogue_logo" class="catalogue_logo_pick">
                {{ t('appLogoChoose') }}
              </label>
              <button
                v-if="hasLogo"
                type="button"
                data-testid="catalogue_logo_remove"
                @click="logo = null"
              >
                {{ t('appLogoRemove') }}
              </button>
            </div>
          </div>
        </div>
      </CommonField>
    </div>

    <CommonField>
      <label for="catalogue_info">{{ t('appInfo') }}</label>
      <textarea
        id="catalogue_info"
        v-model="entry.info"
        rows="3"
        maxlength="300"
        data-testid="catalogue_info"
      ></textarea>
    </CommonField>
  </form>
</template>

<style scoped lang="scss">

.catalogue_entry {
  @extend %panel_sunken;
  padding: $space_4;

  textarea {
    min-height: 0;
  }
}

.catalogue_logo_field {
  grid-column: 1 / -1;
}

.catalogue_logo {
  display: grid;
  grid-template-columns: $space_8 * 4 minmax(0, 1fr);
  align-items: center;
  gap: $space_4;

  img {
    width: $space_8 * 4;
    height: $space_8 * 4;
    object-fit: contain;
  }

  input:focus-visible + label {
    @include focus-ring;
  }

  &_pick {
    @extend %button;
  }

  @media (max-width: $breakpoint_small) {
    grid-template-columns: 1fr;
  }
}

.catalogue_logo_details {
  @extend %stack;
  gap: $space_2;
  padding: $space_3;
  border: $border_thin solid var(--surface-border);
  border-radius: $radius_control;

  dl {
    @extend %stack;
    gap: $space_2 * 0.5;
    margin: 0;
  }

  dl > div {
    @extend %fx_base;
    flex-wrap: wrap;
    gap: $space_2;
  }

  dt {
    font-weight: 650;
    color: var(--theme-foreground);
  }

  dd {
    margin: 0;
  }
}

.catalogue_fields {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 14rem), 1fr));
  gap: $space_3;
}

</style>
