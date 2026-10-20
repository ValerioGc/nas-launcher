import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';
import { useAppsStore } from '@/stores/appsStore';
import { request } from '@/api/client';
import { failureMessage } from '@/utils/failureMessage';

const SESSION_KEY = 'nas-shell-open-apps-v1';

interface SavedShell {
  openedIds: string[];
  activeId: string | null;
}

function readSavedShell(): SavedShell {
  try {
    const saved: unknown = JSON.parse(sessionStorage.getItem(SESSION_KEY) ?? 'null');
    if (!saved || typeof saved !== 'object') {
      return { openedIds: [], activeId: null };
    }

    const record = saved as Record<string, unknown>;
    const openedIds = Array.isArray(record.openedIds)
      ? [...new Set(record.openedIds.filter((id): id is string => typeof id === 'string'))]
      : [];

    const activeId = typeof record.activeId === 'string' ? record.activeId : null;

    return { openedIds, activeId: openedIds.includes(activeId ?? '') ? activeId : null };
  } catch {
    sessionStorage.removeItem(SESSION_KEY);
    return { openedIds: [], activeId: null };
  }
}

/**
 * Manages the state of the shell, including open applications, the active application, and the desktop view.
 * It reads the registry from `appsStore` because a window without its application makes no sense.
 */
export const useShellStore = defineStore('shell', () => {
  const appsStore = useAppsStore();
  const saved = readSavedShell();
  const openedIds = ref<string[]>(saved.openedIds);
  const activeId = ref<string | null>(saved.activeId);
  const modalId = ref<string | null>(null);
  const launchQuery = ref('');
  const locations = ref<Record<string, string>>({});
  const starting = ref<string[]>([]);
  const closing = ref<string[]>([]);
  const startErrors = ref<Record<string, string>>({});
  const closeError = ref('');
  const failedCloseId = ref<string | null>(null);
  const starts = new Map<string, Promise<void>>();
  const closes = new Map<string, Promise<void>>();
  let restoring = openedIds.value.length > 0;

  const openApps = computed(() =>
    openedIds.value.flatMap((id) => appsStore.apps.filter((app) => app.id === id)),
  );

  const openApp = computed(
    () => appsStore.apps.find((app) => app.id === (modalId.value ?? activeId.value)) ?? null,
  );

  watch(
    () => appsStore.apps.map((app) => app.id),
    (ids) => {
      openedIds.value = openedIds.value.filter((id) => ids.includes(id));
      if (!openedIds.value.includes(activeId.value ?? '')) {
        activeId.value = openedIds.value.at(-1) ?? null;
      }
      if (restoring) {
        restoring = false;
        for (const id of openedIds.value) {
          const app = appsStore.apps.find((item) => item.id === id);
          if (app?.entry.source.kind === 'external') {
            void start(id);
          }
        }
      }
    },
  );

  watch(
    [openedIds, activeId],
    ([ids, active]) => {
      sessionStorage.setItem(SESSION_KEY, JSON.stringify({ openedIds: ids, activeId: active }));
    },
    { deep: true },
  );

  /**
   * Starts an external application, or finds it running. The launcher answers at once for one
   * whose process already answers, so the page of a stopped one is never asked to find out.
   */
  function start(id: string) {
    const pending = starts.get(id);
    if (pending) {
      return pending;
    }
    starting.value = [...starting.value, id];
    delete startErrors.value[id];
    const task = request(`/apps/${encodeURIComponent(id)}/start`, {
      method: 'POST',
      signal: AbortSignal.timeout(35000),
    })
      .then(() => {
        delete startErrors.value[id];
      })
      .catch((reason: unknown) => {
        startErrors.value[id] = failureMessage(reason);
      })
      .finally(() => {
        starting.value = starting.value.filter((item) => item !== id);
        starts.delete(id);
      });
    starts.set(id, task);
    return task;
  }

  /**
   * Opens an application. A modal one takes the foreground without becoming a tab.
   * @param id - The application to open; an unknown one is ignored.
   * @param options - What the application is opened with: a search query, or for an external one
   * the query string of the page it opens on, which replaces the one it had.
   */
  function open(id: string, options: { query?: string; location?: string } = {}) {
    const app = appsStore.apps.find((item) => item.id === id);
    if (!app) {
      return;
    }
    const stopping = closes.get(id);
    if (stopping) {
      void stopping.then(() => open(id, options));
      return;
    }
    launchQuery.value = options.query ?? '';
    if (options.location !== undefined) {
      locations.value = { ...locations.value, [id]: options.location };
    }
    if (app.entry.presentation === 'modal') {
      modalId.value = id;
      return;
    }
    modalId.value = null;
    const firstOpen = !openedIds.value.includes(id);
    if (firstOpen) {
      openedIds.value.push(id);
    }
    activeId.value = id;
    if (app.entry.source.kind === 'external' && (firstOpen || startErrors.value[id])) {
      void start(id);
    }
  }

  /**
   * Closes an application. Closing the active tab moves to the one on its left, so the workspace
   * never goes blank while other applications are still open.
   * @param id - The application to close; by default the one in the foreground.
   */
  async function close(id = modalId.value ?? activeId.value) {
    if (id === modalId.value) {
      modalId.value = null;
      launchQuery.value = '';
      return;
    }

    if (!id || !openedIds.value.includes(id)) {
      return;
    }
    const app = appsStore.apps.find((item) => item.id === id);
    if (app?.entry.source.kind === 'external') {
      const pending = closes.get(id);
      if (pending) {
        return pending;
      }
      closing.value = [...closing.value, id];
      closeError.value = '';
      failedCloseId.value = null;
      const task = (async () => {
        await starts.get(id);
        try {
          await request(`/apps/${encodeURIComponent(id)}/close`, {
            method: 'POST',
            signal: AbortSignal.timeout(35000),
          });
          remove(id);
        } catch (reason) {
          closeError.value = failureMessage(reason);
          failedCloseId.value = id;
        } finally {
          closing.value = closing.value.filter((item) => item !== id);
          closes.delete(id);
        }
      })();
      closes.set(id, task);
      return task;
    }
    remove(id);
  }

  function remove(id: string) {
    if (id === modalId.value) {
      modalId.value = null;
      launchQuery.value = '';
    }
    const index = openedIds.value.indexOf(id);
    openedIds.value = openedIds.value.filter((openId) => openId !== id);
    locations.value = Object.fromEntries(
      Object.entries(locations.value).filter(([appId]) => appId !== id),
    );
    if (id === activeId.value) {
      activeId.value = openedIds.value[Math.max(0, index - 1)] ?? null;
    }
    delete startErrors.value[id];
  }

  function showDesktop() {
    activeId.value = null;
    modalId.value = null;
  }

  /**
   * Closes every window of a session, when it begins or ends
   */
  async function endSession() {
    openedIds.value = [];
    activeId.value = null;
    modalId.value = null;
    launchQuery.value = '';
    locations.value = {};
    startErrors.value = {};
    restoring = false;
    sessionStorage.removeItem(SESSION_KEY);
    await request('/apps/close', { method: 'POST', signal: AbortSignal.timeout(35000) }).catch(
      () => {},
    );
  }

  return {
    activeId,
    modalId,
    launchQuery,
    locations,
    starting,
    closing,
    startErrors,
    closeError,
    failedCloseId,
    openApp,
    openApps,
    open,
    start,
    close,
    remove,
    showDesktop,
    endSession,
  };
});
