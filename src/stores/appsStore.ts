import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { appIcon, appLabel, resolveApp } from '@/apps/registry';
import type { DesktopItem, DesktopShortcut, InstalledApp, ShellApp } from '@/types/types';
import folderIcon from '@/assets/icons/folder.svg';
import { request } from '@/api/client';
import { t } from '@/i18n';

/**
 * Applications store: what is installed on the machine and what sits on the desktop.
 */
export const useAppsStore = defineStore('apps', () => {
  const installed = ref<InstalledApp[]>([]);
  const loading = ref(false);
  const errorMessage = ref('');
  const shortcutList = ref<DesktopShortcut[]>([]);

  // The installed applications, with the localized registry label
  const apps = computed<ShellApp[]>(() =>
    installed.value.map((app) => ({
      ...app,
      name: appLabel(app.id, app.name),
      entry: { ...resolveApp(app.id), icon: appIcon(app.id, app.icon) },
    })),
  );

  // The desktop icons. Folders are saved too
  const shortcuts = computed(() =>
    shortcutList.value
      .filter((item) => item.kind === 'app')
      .flatMap((item) => apps.value.filter((app) => app.id === item.target)),
  );

  const desktopItems = computed<DesktopItem[]>(() =>
    shortcutList.value.flatMap<DesktopItem>((shortcut) => {
      if (shortcut.kind === 'folder') {
        return [
          {
            key: `folder:${shortcut.target}`,
            kind: 'folder',
            target: shortcut.target,
            name: shortcut.name ?? shortcut.target.split('/').at(-1) ?? shortcut.target,
            icon: folderIcon,
            available: apps.value.some((app) => app.id === 'nas-file-manager'),
          },
        ];
      }
      const app = apps.value.find((entry) => entry.id === shortcut.target);
      return app
        ? [
            {
              key: `app:${app.id}`,
              kind: 'app',
              target: app.id,
              name: app.name,
              icon: app.entry.icon,
              available: true,
            },
          ]
        : [];
    }),
  );

  // Reads the installed applications and the saved desktop shortcuts, in one round trip.
  async function fetchInstalled() {
    loading.value = true;
    errorMessage.value = '';
    try {
      const [list, saved] = await Promise.all([
        request<InstalledApp[]>('/apps'),
        request<DesktopShortcut[]>('/desktop/shortcuts'),
      ]);
      installed.value = list;
      shortcutList.value = saved;
    } catch {
      errorMessage.value = t('requestFailed');
    } finally {
      loading.value = false;
    }
  }

  /**
   * Adds or removes a desktop shortcut. The backend answers with the whole list, so a failed call
   * leaves the desktop exactly as it was.
   * @param id - The application the shortcut points to.
   * @param remove - Whether to remove the shortcut instead of adding it.
   */
  async function shortcut(id: string, remove = false) {
    errorMessage.value = '';
    try {
      shortcutList.value = await request<DesktopShortcut[]>(
        `/desktop/shortcuts/${encodeURIComponent(id)}`,
        { method: remove ? 'DELETE' : 'POST' },
      );
    } catch {
      errorMessage.value = t('requestFailed');
    }
  }

  async function folderShortcut(path: string, name: string, remove = false): Promise<boolean> {
    errorMessage.value = '';
    if (!remove && `?${new URLSearchParams({ path })}`.length > 2048) {
      errorMessage.value = t('requestFailed');
      return false;
    }
    try {
      shortcutList.value = await request<DesktopShortcut[]>('/desktop/folders', {
        method: remove ? 'DELETE' : 'POST',
        body: JSON.stringify(remove ? { path } : { path, name }),
      });
      return true;
    } catch {
      errorMessage.value = t('requestFailed');
      return false;
    }
  }

  /**
   * Puts the desktop icons in a new order at once, and back as they were if the launcher refuses it.
   * @param ids - Every application on the desktop, first to last.
   */
  async function reorder(ids: Array<string | { kind: 'app' | 'folder'; target: string }>) {
    const previous = shortcutList.value;
    const rank = (shortcut: DesktopShortcut) => {
      const index = ids.findIndex((id) =>
        typeof id === 'string'
          ? shortcut.kind === 'app' && id === shortcut.target
          : id.kind === shortcut.kind && id.target === shortcut.target,
      );
      return index === -1 ? ids.length : index;
    };
    shortcutList.value = [...previous].sort((a, b) => rank(a) - rank(b));
    errorMessage.value = '';
    try {
      shortcutList.value = await request<DesktopShortcut[]>('/desktop/shortcuts/order', {
        method: 'PUT',
        body: JSON.stringify({ order: ids }),
      });
    } catch {
      shortcutList.value = previous;
      errorMessage.value = t('requestFailed');
    }
  }

  return {
    installed,
    loading,
    errorMessage,
    apps,
    shortcuts,
    desktopItems,
    fetchInstalled,
    shortcut,
    folderShortcut,
    reorder,
  };
});
