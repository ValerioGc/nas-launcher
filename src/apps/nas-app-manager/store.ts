import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { request } from '@/api/client';
import { appIcon, appInfo, appLabel, resolveApp } from '@/apps/registry';
import { useAppsStore } from '@/stores/appsStore';
import { useNotificationsStore } from '@/apps/nas-notification-manager/store';
import { t } from '@/i18n';
import type { UserTask } from '@/types/widgets/tasks';
import type {
  AppOffer,
  ForgeCredentials,
  ManagedApp,
  ManageAction,
  ManagerEndpoint,
  ManagerRow,
  ManagerSettings,
  ManagerView,
} from './types';
import { failureMessage } from '@/utils/failureMessage';

/**
 * Hands a new release token to the launcher, which stores it encrypted and never gives it back.
 * @returns what went wrong, or nothing when the token was accepted.
 */
async function saveToken(token: string) {
  try {
    await request('/apps/credentials', { method: 'PUT', body: JSON.stringify({ token }) });
    return '';
  } catch (reason) {
    return failureMessage(reason);
  }
}

const SELF = 'nas-app-manager';

/** The operation the launcher registers for the installation of `<id>`. */
const INSTALL_TASK = 'install-';
const FOLLOW_MS = 2000;

function installedBy(task: UserTask) {
  return task.source === SELF && task.id.startsWith(INSTALL_TASK);
}

/** Shared catalogue, view preferences and operations. Form state belongs to the components. */
export const useAppManagerStore = defineStore('nas-app-manager', () => {
  const apps = useAppsStore();
  const endpoint = ref<ManagerEndpoint>('installed');
  const view = ref<ManagerView>('preview');
  const showSystem = ref(true);
  const data = ref<(ManagedApp | AppOffer)[]>([]);
  const checked = ref<Record<string, AppOffer>>({});
  const selectedId = ref<string | null>(null);
  const forge = ref<ForgeCredentials | null>(null);
  const loading = ref(false);
  const busy = ref(false);
  const checking = ref(false);
  const error = ref('');
  const checkError = ref('');
  const installing = ref<string[]>([]);
  let loadVersion = 0;
  let followTimer: ReturnType<typeof setTimeout> | undefined;

  const rows = computed<ManagerRow[]>(() =>
    data.value
      .filter((entry) => entry.id !== SELF)
      .map((entry) => {
        const offer = 'repository' in entry ? entry : null;
        const found = checked.value[entry.id];
        const installedVersion = 'version' in entry ? entry.version : entry.installedVersion;
        const currentCheck = found?.installedVersion === installedVersion ? found : null;
        return {
          id: entry.id,
          name: appLabel(entry.id, entry.name),
          info: appInfo(entry.id, entry.info),
          system: Boolean(entry.system),
          version: 'version' in entry ? entry.version : (entry.installedVersion ?? ''),
          icon: appIcon(entry.id, 'icon' in entry ? entry.icon : offer?.logo),
          repository: offer?.repository ?? found?.repository ?? '',
          latestVersion: currentCheck?.latestVersion ?? offer?.latestVersion ?? null,
          updateAvailable: currentCheck?.updateAvailable ?? offer?.updateAvailable ?? false,
        };
      }),
  );

  const listed = computed(() =>
    rows.value.filter((app) => endpoint.value === 'available' || showSystem.value || !app.system),
  );

  const selected = computed(
    () =>
      rows.value.find(
        (app) => app.id === selectedId.value && resolveApp(app.id).source.kind === 'external',
      ) ?? null,
  );
  const updates = computed(() => rows.value.filter((app) => app.updateAvailable).length);

  async function load() {
    const version = ++loadVersion;
    loading.value = true;
    error.value = '';
    try {
      const result = await request<(ManagedApp | AppOffer)[]>(`/apps/${endpoint.value}`);
      if (version === loadVersion) {
        data.value = result;
        if (!selected.value) {
          selectedId.value = null;
        }
      }
    } catch (reason) {
      if (version === loadVersion) {
        error.value = failureMessage(reason);
      }
    } finally {
      if (version === loadVersion) {
        loading.value = false;
      }
    }
  }

  function select(id: string | null) {
    if (busy.value || loading.value) {
      return;
    }
    selectedId.value = rows.value.some(
      (app) => app.id === id && resolveApp(app.id).source.kind === 'external',
    )
      ? id
      : null;
  }

  function setView(value: ManagerView) {
    view.value = value;
  }

  function setShowSystem(value: boolean) {
    showSystem.value = value;
  }

  async function setEndpoint(value: ManagerEndpoint) {
    if (busy.value || endpoint.value === value) {
      return;
    }
    endpoint.value = value;
    selectedId.value = null;
    data.value = [];
    await load();
  }

  async function loadCredentials() {
    try {
      forge.value = await request<ForgeCredentials>('/apps/credentials');
    } catch (reason) {
      error.value = failureMessage(reason);
    }
  }

  async function check() {
    if (checking.value || busy.value || !forge.value?.packagesEnabled) {
      return;
    }
    checking.value = true;
    checkError.value = '';
    try {
      const found = await request<AppOffer[]>('/apps/updates', { method: 'POST', body: '{}' });
      checked.value = Object.fromEntries(found.map((offer) => [offer.id, offer]));
      await load();
      void useNotificationsStore().refresh();
    } catch (reason) {
      checkError.value = failureMessage(reason);
    } finally {
      checking.value = false;
    }
  }

  function followLater() {
    clearTimeout(followTimer);
    followTimer = installing.value.length ? setTimeout(follow, FOLLOW_MS) : undefined;
  }

  /**
   * Reads how the installations are going. One that ended refreshes the lists, the menu and the
   * desktop included, and one that failed says why.
   */
  async function follow() {
    let tasks: UserTask[];
    try {
      tasks = (await request<UserTask[]>('/tasks')).filter(installedBy);
    } catch {
      followLater();
      return;
    }
    const idOf = (task: UserTask) => task.id.slice(INSTALL_TASK.length);
    const running = tasks
      .filter((task) => task.state === 'running' || task.state === 'paused')
      .map(idOf);
    const failed = tasks.filter(
      (task) => task.state === 'failed' && installing.value.includes(idOf(task)),
    );
    const ended = installing.value.some((id) => !running.includes(id));
    installing.value = running;
    if (ended) {
      await Promise.all([load(), apps.fetchInstalled()]);
    }
    if (failed.length) {
      error.value = failed
        .map((task) => t('installFailed', { name: task.name, reason: task.detail }))
        .join(' ');
    }
    followLater();
  }

  async function change(action: ManageAction, options: { removeData?: boolean } = {}) {
    const app = selected.value;
    if (!app || busy.value || loading.value) {
      return false;
    }
    busy.value = true;
    error.value = '';
    try {
      const query = action === 'uninstall' && options.removeData ? '?removeData=true' : '';
      await request(`/apps/${encodeURIComponent(app.id)}/${action}${query}`, {
        method: 'POST',
        body: undefined,
      });
      if (action === 'install') {
        installing.value = [...new Set([...installing.value, app.id])];
        followLater();
      }
      await load();
      if (action === 'install' || action === 'uninstall') {
        selectedId.value = null;
        await apps.fetchInstalled();
      }
      return true;
    } catch (reason) {
      error.value = failureMessage(reason);
      return false;
    } finally {
      busy.value = false;
    }
  }

  async function waitForInstall(id: string) {
    const taskId = `${INSTALL_TASK}${id}`;
    for (;;) {
      const tasks = await request<UserTask[]>('/tasks');
      const task = tasks.find((entry) => entry.id === taskId && entry.source === SELF);
      if (!task || task.state === 'done' || task.state === 'cancelled') {
        return;
      }
      if (task.state === 'failed') {
        throw new Error(task.detail || t('updateFailed', { name: id }));
      }
      await new Promise((resolve) => setTimeout(resolve, FOLLOW_MS));
    }
  }

  async function updateApps(ids?: string[]) {
    if (busy.value || loading.value || !forge.value?.packagesEnabled || !forge.value.configured) {
      return false;
    }
    const wanted = ids ?? rows.value.filter((app) => app.updateAvailable).map((app) => app.id);
    const queue = [...new Set(wanted)].filter((id) =>
      rows.value.some(
        (app) =>
          app.id === id && app.updateAvailable && resolveApp(app.id).source.kind === 'external',
      ),
    );
    if (!queue.length) {
      return false;
    }
    busy.value = true;
    error.value = '';
    try {
      for (const id of queue) {
        installing.value = [...new Set([...installing.value, id])];
        await request(`/apps/${encodeURIComponent(id)}/update`, {
          method: 'POST',
          body: undefined,
        });
        await waitForInstall(id);
        installing.value = installing.value.filter((entry) => entry !== id);
        await Promise.all([load(), apps.fetchInstalled()]);
      }
      selectedId.value = null;
      return true;
    } catch (reason) {
      error.value = failureMessage(reason);
      return false;
    } finally {
      installing.value = installing.value.filter((id) => !queue.includes(id));
      busy.value = false;
    }
  }

  async function checkOnOpen() {
    let wanted: ManagerSettings;
    try {
      wanted = await request<ManagerSettings>('/settings/apps');
    } catch {
      return;
    }
    if (!wanted.checkOnOpen) {
      return;
    }
    await check();
    if (wanted.updateOnOpen && updates.value) {
      await updateApps();
    }
  }

  async function initialize() {
    endpoint.value = 'installed';
    view.value = 'preview';
    showSystem.value = true;
    selectedId.value = null;
    data.value = [];
    checked.value = {};
    forge.value = null;
    checkError.value = '';
    await Promise.all([load(), loadCredentials(), follow()]);
    await checkOnOpen();
  }

  return {
    data,
    checked,
    selectedId,
    endpoint,
    view,
    showSystem,
    rows,
    listed,
    selected,
    forge,
    updates,
    loading,
    busy,
    checking,
    error,
    checkError,
    installing,
    initialize,
    follow,
    load,
    select,
    setEndpoint,
    setView,
    setShowSystem,
    loadCredentials,
    saveToken,
    check,
    change,
    updateApps,
  };
});
