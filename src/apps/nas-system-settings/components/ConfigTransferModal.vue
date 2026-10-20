<script setup lang="ts">

import { computed, onMounted, ref } from 'vue';
import { t } from '@/i18n';
import { request } from '@/api/client';
import {
  CONFIRMATION_HEADER,
  confirmPassword,
  saveSignInPolicy,
  signInPolicy,
} from '@/api/gateway';
import type { LauncherConfig } from '../types/config';
import fileIcon from '@/assets/icons/file.svg';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import ConfigAppPicker from './ConfigAppPicker.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonModal from '@/components/commons/CommonModal.vue';
import CommonPasswordInput from '@/components/commons/CommonPasswordInput.vue';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import {
  isEncrypted,
  NotAConfigFile,
  readConfig,
  saveConfigFile,
  WrongFilePassword,
  writeConfig,
} from '../configFile';
import { passwordFailure } from '../passwordFailure';

interface Props {
  mode: 'export' | 'import';
}

const props = defineProps<Props>();

const IMPORT_EFFECTS = ['configImportAddresses', 'configImportApps', 'configImportKept'] as const;

const open = defineModel<boolean>({ required: true });

const password = ref('');
const filePassword = ref('');
const fileText = ref<string | null>(null);
const fileName = ref('');
const encrypted = ref(false);
const busy = ref(false);
const failure = ref('');
const saved = ref('');
const exportable = ref<string[]>([]);
const chosenApps = ref<string[]>([]);

onMounted(async () => {
  if (props.mode === 'export') {
    exportable.value = await request<string[]>('/system/config/apps').catch(() => []);
  }
});

const ready = computed(() => {
  if (busy.value || password.value === '') {
    return false;
  }
  if (props.mode === 'export') {
    return true;
  }

  return fileText.value !== null && (!encrypted.value || filePassword.value !== '');
});

const askFilePassword = computed(() => props.mode === 'export' || encrypted.value);

const shown = computed({
  get: () => open.value,
  set: (value: boolean) => {
    if (!busy.value) {
      open.value = value;
    }
  },
});

function failed(error: unknown) {
  if (error instanceof NotAConfigFile) {
    return t('configNotAFile');
  }
  if (error instanceof WrongFilePassword) {
    return t('configWrongFilePassword');
  }
  return passwordFailure(error);
}

async function chosen(event: Event) {
  failure.value = '';
  fileText.value = null;
  encrypted.value = false;
  const chosenFile = (event.target as HTMLInputElement).files?.[0];
  fileName.value = chosenFile?.name ?? '';
  if (!chosenFile) {
    return;
  }
  try {
    const text = await chosenFile.text();
    encrypted.value = isEncrypted(text);
    fileText.value = text;
  } catch (error) {
    failure.value = failed(error);
  }
}

/**
 * The password checked by the gateway, the configuration from the launcher, the sign-in rule from
 * the gateway, encrypted here when a password for the file was given, and saved as a file.
 */
async function exportConfig() {
  const code = await confirmPassword(password.value);
  const apps = chosenApps.value.length
    ? `?apps=${encodeURIComponent(chosenApps.value.join(','))}`
    : '';

  const config = await request<LauncherConfig>(`/system/config${apps}`, {
    headers: { [CONFIRMATION_HEADER]: code },
  });

  const signIn = await signInPolicy().catch(() => null);
  saved.value = saveConfigFile(await writeConfig({ config, signIn }, filePassword.value));
}

// Handles the import of a configuration file, verifying the password if necessary and applying the settings.
async function importConfig(text: string) {
  const content = await readConfig(text, filePassword.value);
  const code = await confirmPassword(password.value);

  await request<LauncherConfig>('/system/config', {
    method: 'POST',
    body: JSON.stringify(content.config),
    headers: { [CONFIRMATION_HEADER]: code },
  });
  if (content.signIn) {
    await saveSignInPolicy(content.signIn);
  }

  globalThis.location.reload();
}

async function submit() {
  failure.value = '';
  busy.value = true;
  try {
    if (props.mode === 'export') {
      await exportConfig();
    } else if (fileText.value !== null) {
      await importConfig(fileText.value);
    }
  } catch (error) {
    failure.value = failed(error);
  } finally {
    password.value = '';
    filePassword.value = '';
    busy.value = false;
  }
}

</script>

<template>
  <CommonModal
    v-model="shown"
    :title="t(mode === 'export' ? 'configExportTitle' : 'configImportTitle')"
    :hide-close-button="busy"
  >
    <AlertBanner v-if="saved" tone="success" data-testid="config_exported">
      {{ t('configExported', { file: saved }) }}
    </AlertBanner>

    <form
      v-else
      id="config_transfer_form"
      class="app_stack"
      data-testid="config_transfer_form"
      @submit.prevent="submit"
    >
      <p v-if="mode === 'export'" class="muted">{{ t('configExportNote') }}</p>
      <AlertBanner v-else tone="warning" :title="t('configImportNote')">
        <ul class="config_import_effects">
          <li v-for="key in IMPORT_EFFECTS" :key="key">{{ t(key) }}</li>
        </ul>
      </AlertBanner>

      <div v-if="mode === 'import'" class="config_field">
        <span id="config_file_label" class="config_label">{{ t('configFile') }}</span>
        <label class="config_file" :class="{ config_file_disabled: busy }" for="config_file">
          <CommonIcon :src="fileIcon" />
          <span class="config_file_name" :class="{ muted: !fileName }">
            {{ fileName || t('configNoFile') }}
          </span>
          <span class="config_file_button">{{ t('configChooseFile') }}</span>
          <input
            id="config_file"
            class="config_file_input"
            type="file"
            accept=".json,application/json"
            aria-labelledby="config_file_label"
            aria-describedby="config_file_note"
            :disabled="busy"
            data-testid="config_file"
            @change="chosen"
          />
        </label>
        <small id="config_file_note" class="muted">{{ t('configFileHint') }}</small>
      </div>

      <ConfigAppPicker
        v-if="mode === 'export'"
        v-model="chosenApps"
        :exportable="exportable"
        :disabled="busy"
      />

      <div class="config_field">
        <label class="config_label" for="config_password">{{ t('accountPassword') }}</label>
        <CommonPasswordInput
          id="config_password"
          v-model="password"
          username=""
          required
          maxlength="128"
          aria-describedby="config_password_note"
          :disabled="busy"
          data-testid="config_password"
        />
        <small id="config_password_note" class="muted">{{ t('configPasswordNote') }}</small>
      </div>

      <div v-if="askFilePassword" class="config_field">
        <label class="config_label" for="config_file_password">{{ t('filePassword') }}</label>
        <CommonPasswordInput
          id="config_file_password"
          v-model="filePassword"
          :new-password="mode === 'export'"
          :required="mode === 'import'"
          maxlength="128"
          aria-describedby="config_file_password_note"
          :disabled="busy"
          data-testid="config_file_password"
        />
        <small id="config_file_password_note" class="muted">
          {{ t(mode === 'export' ? 'filePasswordExportNote' : 'filePasswordImportNote') }}
        </small>
      </div>

      <AlertBanner v-if="failure" tone="error" compact data-testid="config_failed">
        {{ failure }}
      </AlertBanner>
    </form>

    <template #footer>
      <button v-if="saved" type="button" @click="open = false">{{ t('close') }}</button>
      <template v-else>
        <button type="button" :disabled="busy" @click="open = false">{{ t('cancel') }}</button>
        <button
          type="submit"
          form="config_transfer_form"
          :class="mode === 'export' ? 'primary' : 'danger'"
          :disabled="!ready"
          data-testid="config_transfer_confirm"
        >
          <CommonSpinner v-if="busy" size="small" compact />
          {{ t(mode === 'export' ? 'configExportAction' : 'configImportAction') }}
        </button>
      </template>
    </template>
  </CommonModal>
</template>

<style scoped lang="scss">

.config_import_effects {
  @extend %bullet_list;
  --bullet-tone: var(--alert-tone);
}

.config_field {
  display: grid;
  gap: $space_2;
}

.config_label {
  font-weight: 600;
}

.config_file {
  @extend %panel_sunken;
  display: flex;
  align-items: center;
  gap: $space_3;
  padding: $space_2 $space_3;
  cursor: pointer;

  &:has(:focus-visible) {
    @include focus-ring;
  }

  &.config_file_disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }
}

.config_file_name {
  @extend %truncated_text;
  flex: 1;
  min-width: 0;
}

.config_file_button {
  @extend %button;
  flex: none;
}

.config_file_input {
  @extend %visually_hidden;
}

</style>
