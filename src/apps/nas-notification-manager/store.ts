import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';
import { request } from '@/api/client';
import { t } from '@/i18n';
import { useShellStore } from '@/stores/shellStore';

import type { AppNotification, NotificationTone, Toast } from './types';
import { failureMessage } from '@/utils/failureMessage';
import { noticeMessage, noticeTitle, SIGN_IN_NOTICES, sourceLabel } from './text';

const MAX_TOASTS = 4;

export const TOAST_MS: Record<NotificationTone, number> = {
  info: 6000,
  success: 6000,
  warning: 12000,
  error: 12000,
};

export const useNotificationsStore = defineStore('notifications', () => {
  const items = ref<AppNotification[]>([]);
  const error = ref('');
  const removing = ref(false);
  const unread = computed(() => items.value.length);
  let pending: Promise<void> | undefined;
  const tones = ['info', 'success', 'warning', 'error'] as const;
  const tone = ref('');
  const source = ref('');
  const unreadOnly = ref(false);
  const query = ref('');
  const data = ref<AppNotification[]>([]);
  const sources = ref<string[]>([]);
  const loading = ref(false);
  const busy = ref(false);
  const managerError = ref('');
  let managerVersion = 0;
  let seen: Set<string> | undefined;
  const toasts = ref<Toast[]>([]);

  const managerEndpoint = computed(() => {
    const params = new URLSearchParams({ limit: '500' });
    if (tone.value) {
      params.set('tone', tone.value);
    }
    if (source.value) {
      params.set('source', source.value);
    }
    if (unreadOnly.value) {
      params.set('read', 'false');
    }
    return `/notifications?${params}`;
  });

  const visible = computed(() => {
    const needle = query.value.trim().toLocaleLowerCase();
    if (!needle) {
      return data.value;
    }

    return data.value.filter((item) =>
      [noticeTitle(item), noticeMessage(item), sourceLabel(item.source)]
        .join(' ')
        .toLocaleLowerCase()
        .includes(needle),
    );
  });

  // Sources cover the full log, so a filter with no matches remains editable.
  const hasNotifications = computed(
    () => data.value.length > 0 || sources.value.length > 0 || items.value.length > 0,
  );

  const managerUnread = computed(() => data.value.filter((item) => !item.read).length);

  const filtered = computed(() =>
    Boolean(tone.value || source.value || unreadOnly.value || query.value),
  );

  const sourceOptions = computed(() => {
    if (source.value && !sources.value.includes(source.value)) {
      return [source.value, ...sources.value];
    }
    return sources.value;
  });

  async function load() {
    const current = ++managerVersion;
    loading.value = true;
    managerError.value = '';
    try {
      const result = await request<AppNotification[]>(managerEndpoint.value);
      if (current === managerVersion) {
        data.value = result;
      }
    } catch {
      if (current === managerVersion) {
        managerError.value = t('requestFailed');
      }
    } finally {
      if (current === managerVersion) {
        loading.value = false;
      }
    }
  }

  async function loadSources() {
    try {
      sources.value = await request<string[]>('/notifications/sources');
    } catch {
      // The existing value remains usable when the optional source list cannot refresh.
    }
  }

  async function change(path: string, method = 'POST', body?: unknown) {
    if (busy.value) {
      return;
    }
    busy.value = true;
    managerError.value = '';
    try {
      await request(path, { method, body: body === undefined ? undefined : JSON.stringify(body) });
      await Promise.all([load(), loadSources(), refresh(false)]);
    } catch (error_) {
      managerError.value = failureMessage(error_);
    } finally {
      busy.value = false;
    }
  }

  function setTone(value: string) {
    tone.value = value;
  }
  function setSource(value: string) {
    source.value = value;
  }
  function setUnreadOnly(value: boolean) {
    unreadOnly.value = value;
  }
  function setQuery(value: string) {
    query.value = value;
  }
  function reset() {
    tone.value = '';
    source.value = '';
    unreadOnly.value = false;
    query.value = '';
  }

  watch([tone, source, unreadOnly], load);

  /**
   * Shows a toast, replacing the one with the same id, and keeps only the latest few.
   * @param item - The toast to show.
   */
  function toast(item: Toast) {
    toasts.value = [...toasts.value.filter((entry) => entry.id !== item.id), item].slice(
      -MAX_TOASTS,
    );
  }

  function dismiss(id: string) {
    toasts.value = toasts.value.filter((entry) => entry.id !== id);
  }

  function show(item: AppNotification, duration?: number) {
    toast({
      id: item.id,
      tone: item.tone,
      title: noticeTitle(item),
      message: noticeMessage(item),
      duration,
      notification: item,
      action: {
        label: t('viewNotification'),
        run: () => useShellStore().open('nas-notification-manager'),
      },
    });
  }

  /**
   * Turns the unread notifications that arrived since the previous refresh into toasts. The first
   * refresh only takes note of what is there: the log of a whole day is not news. Only what was
   * written for the first sign-in after it is shown then, until closed, and marked as read so that
   * it is not shown again.
   * @param result - The notifications just read, newest first.
   * @param news - False when the refresh follows an action of this page: what it just caused is
   * already in front of whoever clicked.
   */
  function announce(result: AppNotification[], news: boolean) {
    if (seen && news) {
      const known = seen;
      for (const item of result.filter((entry) => !known.has(entry.id)).reverse()) {
        show(item, TOAST_MS[item.tone]);
      }
    } else if (!seen) {
      for (const item of result.filter((entry) => SIGN_IN_NOTICES.includes(entry.key ?? ''))) {
        show(item);
        void request(`/notifications/${encodeURIComponent(item.id)}`, {
          method: 'PATCH',
          body: JSON.stringify({ read: true }),
        }).catch(() => undefined);
      }
    }
    seen = new Set(result.map((item) => item.id));
  }

  /**
   * Refreshes the unread notifications from the server.
   * @param news - Whether what arrived is shown as toasts; see `announce`.
   * @returns A promise that resolves when the notifications have been refreshed.
   */
  function refresh(news = true) {
    if (pending) {
      return pending;
    }

    pending = request<AppNotification[]>('/notifications?read=false&limit=500')
      .then((result) => {
        announce(result, news);
        items.value = result;
        error.value = '';
      })
      .catch(() => {
        error.value = t('requestFailed');
      })
      .finally(() => {
        pending = undefined;
      });

    return pending;
  }

  /**
   * Removes a notification by its ID.
   * @param id - The ID of the notification to remove.
   * @returns A promise that resolves when the notification has been removed.
   */
  async function remove(id: string) {
    if (removing.value) {
      return;
    }

    removing.value = true;
    error.value = '';

    try {
      await request(`/notifications/${encodeURIComponent(id)}`, { method: 'DELETE' });
      await pending;
      items.value = items.value.filter((item) => item.id !== id);
    } catch {
      error.value = t('requestFailed');
    } finally {
      removing.value = false;
    }
  }

  return {
    items,
    error,
    unread,
    removing,
    refresh,
    remove,
    toasts,
    toast,
    dismiss,
    tones,
    tone,
    source,
    unreadOnly,
    query,
    data,
    visible,
    sources,
    loading,
    busy,
    managerError,
    managerUnread,
    hasNotifications,
    filtered,
    sourceOptions,
    load,
    loadSources,
    change,
    setTone,
    setSource,
    setUnreadOnly,
    setQuery,
    reset,
  };
});
