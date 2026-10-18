<script setup lang="ts">

import { computed, onMounted, reactive, ref, watch } from 'vue';
import { t } from '@/i18n';
import { request } from '@/api/client';
import { failureMessage } from '@/utils/failureMessage';
import { useAppManagerStore } from '@/apps/nas-app-manager/store';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import type { CatalogueEntry, ManagerSettings } from '@/apps/nas-app-manager/types';

import AlertBanner from '@/components/alerts/AlertBanner.vue';
import AppToken from '@/apps/nas-app-manager/components/AppToken.vue';
import CatalogueEditor from '@/apps/nas-app-manager/components/CatalogueEditor.vue';
import CatalogueEntryForm from '@/apps/nas-app-manager/components/CatalogueEntryForm.vue';
import CommonIcon from '@/components/commons/CommonIcon.vue';
import CommonModal from '@/components/commons/CommonModal.vue';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';
import desktopIcon from '@/assets/icons/navigation/desktop.svg';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import UpdateSchedule from '@/apps/nas-app-manager/components/UpdateSchedule.vue';
import ConfirmationModal from '@/apps/nas-app-manager/components/modals/ConfirmationModal.vue';

const SETTINGS_PATH = '/settings/apps';
const CATALOGUE_PATH = '/apps/catalogue';

const open = defineModel<boolean>({ required: true });

const manager = useAppManagerStore();
// The choice is kept with the general settings of the NAS, where the launcher reads it.
const general = useSettingsStore();
const createShortcut = ref(true);
const settings = ref<ManagerSettings | null>(null);
const apps = ref<CatalogueEntry[]>([]);
const token = ref('');
const stored = ref('');
const entry = ref<CatalogueEntry | null>(null);
const position = ref<number | null>(null);
const logos = reactive(new Map<string, File | null>());
const logo = ref<File | null | undefined>();
const loading = ref(true);
const busy = ref(false);
const error = ref('');
const removal = ref<CatalogueEntry | null>(null);
const confirmRemoval = computed({
  get: () => removal.value !== null,
  set: (value: boolean) => {
    if (!value) {
      removal.value = null;
    }
  },
});

const catalogueChanged = computed(() => JSON.stringify(apps.value) !== stored.value);

function keep(catalogue: CatalogueEntry[]) {
  apps.value = catalogue;
  stored.value = JSON.stringify(catalogue);
}

async function load() {
  try {
    const [current, catalogue] = await Promise.all([
      request<ManagerSettings>(SETTINGS_PATH),
      request<CatalogueEntry[]>(CATALOGUE_PATH),
      general.general ? Promise.resolve() : general.load(),
    ]);
    settings.value = current;
    createShortcut.value = general.general?.createShortcut ?? true;
    keep(catalogue);
  } catch (reason) {
    error.value = failureMessage(reason);
  } finally {
    loading.value = false;
  }
}

function add() {
  position.value = null;
  logo.value = undefined;
  entry.value = {
    id: '',
    name: '',
    info: '',
    provider: 'github',
    repository: '',
    asset: '',
    preinstall: false,
    logo: '',
    category: null,
  };
}

function edit(index: number) {
  const chosen = apps.value[index];
  if (chosen) {
    position.value = index;
    logo.value = logos.has(chosen.id) ? logos.get(chosen.id) : undefined;
    entry.value = { ...chosen };
  }
}

function apply() {
  if (!entry.value) {
    return;
  }
  if (position.value === null) {
    apps.value.push(entry.value);
  } else {
    apps.value[position.value] = entry.value;
  }
  if (logo.value !== undefined) {
    logos.set(entry.value.id, logo.value);
  }
  entry.value = null;
}

function requestRemoval(index: number) {
  removal.value = apps.value[index] ?? null;
}

function confirmRemove() {
  const index = apps.value.findIndex((app) => app.id === removal.value?.id);
  if (index !== -1) {
    remove(index);
  }
  removal.value = null;
}

function remove(index: number) {
  const [removed] = apps.value.splice(index, 1);
  if (removed) {
    logos.delete(removed.id);
  }
}

/** Sends the logos after the catalogue, which has to offer an application before it takes one. */
async function sendLogos() {
  for (const [id, file] of logos) {
    const path = `/apps/${encodeURIComponent(id)}/logo`;
    await request(
      path,
      file
        ? { method: 'PUT', body: file, headers: { 'Content-Type': file.type } }
        : { method: 'DELETE' },
    );
    logos.delete(id);
  }
  keep(await request<CatalogueEntry[]>(CATALOGUE_PATH));
}

/** Saves the choice of the shortcuts with the general settings, when it changed. */
async function saveShortcut() {
  const current = general.general;
  if (!current || current.createShortcut === createShortcut.value) {
    return;
  }
  if (!(await general.save({ ...current, createShortcut: createShortcut.value }))) {
    throw new Error(general.error);
  }
}

/**
 * Saves what changed: the automatic check always, the catalogue when it differs from the one the
 * launcher gave, then its logos, and the token when one was written. The token goes last, because
 * storing it restarts the launcher. Saved, the window closes; a failure keeps it open with why.
 */
async function save() {
  busy.value = true;
  error.value = '';
  try {
    settings.value = await request<ManagerSettings>(SETTINGS_PATH, {
      method: 'PUT',
      body: JSON.stringify(settings.value),
    });
    await saveShortcut();
    const catalogue = catalogueChanged.value;
    if (catalogue) {
      keep(
        await request<CatalogueEntry[]>(CATALOGUE_PATH, {
          method: 'PUT',
          body: JSON.stringify({ apps: apps.value }),
        }),
      );
    }
    const images = logos.size > 0;
    if (images) {
      await sendLogos();
    }
    if (catalogue || images) {
      await manager.load();
    }
    const written = token.value.trim();
    if (written) {
      const refused = await manager.saveToken(written);
      if (refused) {
        throw new Error(refused);
      }
      token.value = '';
    }
    open.value = false;
  } catch (reason) {
    error.value = failureMessage(reason);
  } finally {
    busy.value = false;
  }
}

watch(open, (value) => {
  if (!value) {
    removal.value = null;
  }
});

onMounted(load);

</script>

<template>
  <CommonModal v-model="open" :title="t('managerSettings')" fixed-size>
    <CommonSpinner v-if="loading" centered />

    <CatalogueEntryForm
      v-else-if="entry"
      v-model="entry"
      v-model:logo="logo"
      :existing="position !== null"
      @apply="apply"
    />

    <!-- A form, so that the token field is a password of a form and Enter saves too. -->
    <form
      v-else
      id="app_manager_settings"
      class="app_surface app_stack"
      data-testid="app_manager_settings"
      @submit.prevent="save"
    >
      <AlertBanner v-if="error" tone="error" compact data-testid="app_manager_settings_error">
        {{ error }}
      </AlertBanner>

      <section class="app_panel app_card" :aria-label="t('newAppsDesktop')">
        <header class="app_card_header">
          <CommonIcon :src="desktopIcon" tinted />
          <div class="app_card_title">
            <h3>{{ t('newAppsDesktop') }}</h3>
            <p class="muted">{{ t('newAppsDesktopNote') }}</p>
          </div>
        </header>
        <CommonSwitch
          v-model="createShortcut"
          :label="t('automaticShortcuts')"
          :disabled="!general.general"
          testid="app_manager_shortcuts"
        />
      </section>

      <UpdateSchedule v-if="settings" v-model="settings" />
      <AppToken v-model="token" :configured="!!manager.forge?.configured" />
      <CatalogueEditor :apps="apps" @add="add" @edit="edit" @remove="requestRemoval" />
    </form>

    <template v-if="entry" #footer>
      <button type="button" data-testid="catalogue_cancel" @click="entry = null">
        {{ t('cancel') }}
      </button>
      <button type="submit" form="catalogue_entry" class="primary" data-testid="catalogue_apply">
        {{ t('applyEntry') }}
      </button>
    </template>
    <template v-else-if="!loading" #footer>
      <button
        type="submit"
        form="app_manager_settings"
        class="primary"
        :disabled="busy"
        data-testid="app_manager_settings_save"
      >
        <CommonSpinner v-if="busy" size="small" compact />
        {{ t('save') }}
      </button>
    </template>
  </CommonModal>
  <ConfirmationModal
    v-model="confirmRemoval"
    :app-name="removal?.name ?? ''"
    action="remove"
    :busy="false"
    error=""
    @confirm="confirmRemove"
  />
</template>
