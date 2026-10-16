import { onBeforeUnmount, reactive, ref, watch, type InjectionKey } from 'vue';
import { locale } from '@/i18n';
import { randomUuid } from '@/utils/randomUuid';
import { request } from '@/api/client';
import { useSettingsStore } from '@/apps/nas-system-settings/store';
import { useShellStore } from '@/stores/shellStore';
import { useAppsStore } from '@/stores/appsStore';
import {
  APP_PROTOCOL,
  type AppContext,
  type AppMessage,
  type DescribedNotification,
  type AccessReply,
  type AccessRequest,
  type DiagnosticReply,
  type DiagnosticRequest,
  type DiskReply,
  type DiskRequest,
  type SystemMonitorReply,
  type SystemMonitorRequest,
  type ShellMessage,
  type ThemeRole,
  type NetworkDisk,
  type PickedFile,
} from '@/types/types';
import { noticeMessage, noticeTitle, sourceLabel } from '@/apps/nas-notification-manager/text';
import { accessCall } from '@/api/access';
import { httpsSettings } from '@/api/securityCertificates';
import { gateway } from '@/api/securityDevices';
import { diskCall, waitForTask } from '@/api/disks';
import { SYSTEM_MONITOR, monitorCall } from '@/api/systemMonitor';
import type { AppNotification } from '@/apps/nas-notification-manager/types';
import type { PowerSettings } from '@/apps/nas-system-settings/types/power';

/**
 * The conversation between the shell and the applications it shows in a frame.
 *
 * An external application is served under `/apps/<id>/`, on the origin of the console. This is the
 * one door the shell opens to it, and it is narrow on purpose: the shell tells an application how
 * it is being displayed, and the application may ask for a notification, for the notifications to
 * be described or for a window to be opened.
 *
 * **The origin is checked in both directions.** Going out, every message is addressed to the
 * application's own origin and never to `*`. Coming in, a message counts only if it arrived from
 * the origin of an application that is open and from that frame's own window: an application
 * cannot speak for another one, and neither can anything else that happens to post into this page.
 */

/** The only application that reads, opens and removes the notifications of the launcher. */
const LOG_MANAGER = 'nas-log-manager';
/** The only application that initializes, renames, ejects and reads the health of the disks. */
const DISK_MANAGER = 'nas-disk-manager';
/** The only application that manages who reaches the NAS. */
const SECURITY_CENTER = 'nas-security-center';
const NOTIFICATION_ID = /^\d{1,20}$/;

/**
 * The frame of the widget nas-system-monitor draws in the drawer of the processes. It is kept
 * apart from the frame of its window, which may be open at the same time, and speaks for the
 * same application.
 */
export const MONITOR_WIDGET = `${SYSTEM_MONITOR}#widget`;

/** The variable of the launcher behind each colour of the theme an application receives. */
const PALETTE: Record<ThemeRole, string> = {
  background: '--theme-background',
  foreground: '--theme-foreground',
  surface: '--surface',
  sunken: '--surface-sunken',
  hover: '--surface-hover',
  border: '--surface-border',
  muted: '--surface-muted',
};

/** The application a frame belongs to: its window, or its widget. */
function appOf(frame: string) {
  return frame.split('#')[0] ?? frame;
}

/** The message of one type an application sends. */
type MessageOf<T extends AppMessage['type']> = Extract<AppMessage, { type: T }>;

/** Opens the console on the gateway's HTTPS port: the app cannot navigate the top frame. */
function navigateHttps(port: number) {
  const page = globalThis.location;
  const target = new URL(page.href);
  target.protocol = 'https:';
  target.port = port === 443 ? '' : String(port);
  if (target.href !== page.href) {
    page.assign(target.href);
  }
}

async function openHttps(id: string) {
  if (id !== SECURITY_CENTER) {
    return;
  }
  try {
    const settings = await gateway('/auth/https');
    if (httpsSettings(settings)) {
      navigateHttps(settings.port);
    }
  } catch (error) {
    console.error('HTTPS settings unavailable', error);
  }
}

function validRequest(requestId: unknown): requestId is string {
  return typeof requestId === 'string' && requestId.length > 0 && requestId.length <= 100;
}

/**
 * One file of the picker's answer, or null when it is not one the request may receive: a folder for
 * a folder request, otherwise a file with the extensions asked for, downloaded from the API of the
 * picker itself (`base`).
 */
function pickedFile(
  item: unknown,
  request: { folder: boolean; extensions?: string[] },
  base: URL,
): PickedFile | null {
  if (!item || typeof item !== 'object') {
    return null;
  }
  const { name, path, download } = item as Record<string, unknown>;
  if (typeof name !== 'string' || !name || typeof path !== 'string') {
    return null;
  }
  if (!path.startsWith('/') || path.length > 2048) {
    return null;
  }
  if (request.folder) {
    return { name, path };
  }
  const wanted =
    !request.extensions?.length ||
    request.extensions.some((extension) => name.toLowerCase().endsWith(extension.toLowerCase()));
  if (!wanted || typeof download !== 'string') {
    return null;
  }
  if (!download.startsWith('api/') || download.length > 2048) {
    return null;
  }
  const url = new URL(download, base);
  if (url.origin !== base.origin || !url.pathname.startsWith(`${base.pathname}api/`)) {
    return null;
  }
  return { name, path, url: url.toString() };
}

function describe(received: unknown): DescribedNotification[] {
  const text = (value: unknown) => (typeof value === 'string' ? value : '');
  return (Array.isArray(received) ? received.slice(0, 500) : [])
    .filter((item): item is Record<string, unknown> => Boolean(item) && typeof item === 'object')
    .map((item) => {
      const params =
        item.params && typeof item.params === 'object'
          ? Object.fromEntries(
              Object.entries(item.params).map(([name, value]) => [name, text(value)]),
            )
          : {};
      const notification: AppNotification = {
        id: text(item.id),
        title: text(item.title),
        message: text(item.message),
        source: text(item.source),
        tone: 'info',
        at: 0,
        read: true,
        key: text(item.key) || null,
        params,
      };
      return {
        id: notification.id,
        title: noticeTitle(notification),
        message: noticeMessage(notification),
        source: sourceLabel(notification.source),
      };
    });
}

function notify(id: string, message: MessageOf<'notify'>) {
  return request('/notifications', {
    method: 'POST',
    body: JSON.stringify({
      title: String(message.title ?? '').slice(0, 160),
      message: String(message.message ?? '').slice(0, 2000),
      source: appOf(id),
      tone: message.tone ?? 'info',
      ...(typeof message.log === 'string' ? { log: message.log } : {}),
    }),
  }).catch(() => {});
}

/** Whether a request about a disk comes from nas-disk-manager and names a device. */
function fromDisks(id: string, message: { requestId: unknown; device: unknown }) {
  return (
    id === DISK_MANAGER && validRequest(message.requestId) && typeof message.device === 'string'
  );
}

/** The network shares a provider reported, refused whole when one of them is malformed. */
function networkDiskList(received: unknown): NetworkDisk[] {
  if (!Array.isArray(received) || received.length > 64) {
    throw new Error('Invalid network disk list');
  }
  return received.map((item: unknown) => {
    const disk = item && typeof item === 'object' ? (item as Record<string, unknown>) : {};
    if (
      typeof disk.name !== 'string' ||
      !disk.name ||
      typeof disk.mount !== 'string' ||
      !disk.mount.startsWith('/') ||
      disk.mount.length > 2048 ||
      (disk.source !== null && typeof disk.source !== 'string') ||
      typeof disk.read_only !== 'boolean'
    ) {
      throw new Error('Invalid network disk list');
    }
    return { name: disk.name, mount: disk.mount, source: disk.source, readOnly: disk.read_only };
  });
}

export function useAppBridge() {
  const shell = useShellStore();
  const settings = useSettingsStore();
  const apps = useAppsStore();
  const frames = new Map<string, HTMLIFrameElement>();
  /** The applications that said `ready`: their page is drawn, not only loaded. */
  const ready = reactive(new Set<string>());
  const picker = ref<{
    provider: string;
    requester: string;
    requestId: string;
    token: string;
    multiple: boolean;
    extensions?: string[];
    folder: boolean;
    url: string;
  } | null>(null);
  let pickerFrame: HTMLIFrameElement | null = null;
  let pickerStarting = false;

  function originOf(id: string): string {
    const source = frames.get(id)?.src;
    return source ? new URL(source, globalThis.location.href).origin : '';
  }

  /**
   * How the launcher is showing the application: the resolved theme, the accent colour, the
   * colours of the theme chosen for that type and the language. They are read from the document,
   * where `useAppearance` puts them, rather than worked out a second time from the settings.
   */
  function contextOf(id: string): AppContext {
    const root = document.documentElement;
    const accent =
      getComputedStyle(root).getPropertyValue('--accent') ||
      root.style.getPropertyValue('--accent');

    const style = getComputedStyle(root);

    return {
      app: appOf(id),
      nasName: settings.general?.nasName ?? 'nas',
      theme: root.dataset.theme ?? 'light',
      accent: accent.trim(),
      locale: locale.value,
      palette: Object.fromEntries(
        Object.entries(PALETTE).map(([role, variable]) => [
          role,
          style.getPropertyValue(variable).trim(),
        ]),
      ) as Record<ThemeRole, string>,
    };
  }

  function post(id: string, message: ShellMessage) {
    const origin = originOf(id);
    if (origin) {
      frames.get(id)?.contentWindow?.postMessage(message, origin);
    }
  }

  /**
   * Does what the log manager asked with the session of the console, and hands back the outcome
   * under the same request id. Its own process never reaches the launcher.
   */
  async function answer(
    id: string,
    type:
      | DiagnosticReply
      | AccessReply
      | DiskReply
      | SystemMonitorReply
      | 'smart-disk-result'
      | 'network-disks-result'
      | 'power-schedule-result',
    requestId: string,
    run: () => Promise<unknown>,
    unavailable = 'Notifications unavailable',
  ) {
    try {
      post(id, { nas: APP_PROTOCOL, type, requestId, success: true, data: await run() });
    } catch (reason) {
      post(id, {
        nas: APP_PROTOCOL,
        type,
        requestId,
        success: false,
        error: reason instanceof Error ? reason.message : unavailable,
      });
    }
  }

  /** Tells one application where it stands. Sent when its frame loads, and whenever it asks. */
  function announce(id: string) {
    post(id, { nas: APP_PROTOCOL, type: 'context', context: contextOf(id) });
  }

  /** The same, to every open application: the theme changed for all of them at once. */
  function broadcast() {
    for (const id of frames.keys()) {
      announce(id);
    }
  }

  /** Follows the frames Vue creates and removes as tabs open and close. */
  function attach(id: string, element: HTMLIFrameElement | null) {
    if (element) {
      frames.set(id, element);
    } else {
      frames.delete(id);
      ready.delete(id);
      if (picker.value?.requester === id) {
        picker.value = null;
        pickerFrame = null;
      }
    }
  }

  function attachPicker(element: HTMLIFrameElement | null) {
    pickerFrame = element;
  }

  function announcePicker() {
    if (picker.value && pickerFrame?.contentWindow) {
      pickerFrame.contentWindow.postMessage(
        { nas: APP_PROTOCOL, type: 'context', context: contextOf(picker.value.provider) },
        window.location.origin,
      );
    }
  }

  /**
   * Starts the first installed version 1 provider of a capability whose endpoint `accepts`, and
   * gives the URL of that endpoint on the shell's origin.
   */
  async function startProvider(
    capability: string,
    accepts: (endpoint: string) => boolean,
    missing: string,
  ) {
    const providers = await request<{ app: string; endpoint: string; version: number }[]>(
      `/capabilities/${capability}`,
    );
    const provider = providers.find(
      (item) => item.version === 1 && item.endpoint.startsWith('/') && accepts(item.endpoint),
    );
    if (!provider) {
      throw new Error(missing);
    }
    const url = new URL(
      `/apps/${encodeURIComponent(provider.app)}${provider.endpoint}`,
      window.location.origin,
    );
    await shell.start(provider.app);
    if (shell.startErrors[provider.app]) {
      throw new Error(shell.startErrors[provider.app]);
    }
    return { app: provider.app, url };
  }

  function finishPicker(files: PickedFile[], cancelled: boolean, error?: string) {
    const pending = picker.value;
    if (!pending) {
      return;
    }
    post(pending.requester, {
      nas: APP_PROTOCOL,
      type: 'pick-files-result',
      requestId: pending.requestId,
      files,
      cancelled,
      ...(error ? { error } : {}),
    });
    picker.value = null;
    pickerFrame = null;
  }

  async function beginPicker(
    requester: string,
    requestId: string,
    options: { multiple: boolean; extensions?: string[]; directory?: string; folder: boolean },
  ) {
    const { multiple, extensions, directory, folder } = options;
    if (picker.value || pickerStarting) {
      post(requester, {
        nas: APP_PROTOCOL,
        type: 'pick-files-result',
        requestId,
        files: [],
        cancelled: false,
        error: 'A file selection is already open',
      });
      return;
    }
    pickerStarting = true;
    try {
      const { app, url } = await startProvider(
        'file-picker',
        (endpoint) => endpoint.endsWith('/'),
        'No file picker provider is installed',
      );
      if (!frames.has(requester)) {
        return;
      }
      const token = randomUuid();
      url.searchParams.set('nasPicker', token);
      if (folder) {
        url.searchParams.set('folder', '1');
      } else if (multiple) {
        url.searchParams.set('multiple', '1');
      }
      if (!folder && extensions?.length) {
        url.searchParams.set('extensions', extensions.join(','));
      }
      if (directory) {
        url.searchParams.set('directory', directory);
      }
      picker.value = {
        provider: app,
        requester,
        requestId,
        token,
        multiple: multiple && !folder,
        extensions: folder ? undefined : extensions,
        folder,
        url: url.toString(),
      };
    } catch (reason) {
      post(requester, {
        nas: APP_PROTOCOL,
        type: 'pick-files-result',
        requestId,
        files: [],
        cancelled: false,
        error: reason instanceof Error ? reason.message : 'File picker unavailable',
      });
    } finally {
      pickerStarting = false;
    }
  }

  function receivePicker(event: MessageEvent): boolean {
    const pending = picker.value;
    if (
      !pending ||
      !pickerFrame?.contentWindow ||
      event.source !== pickerFrame.contentWindow ||
      event.origin !== window.location.origin
    ) {
      return false;
    }
    const message = event.data as Record<string, unknown> | null;
    if (message?.nas !== APP_PROTOCOL) {
      return true;
    }
    if (message.type === 'ready') {
      announcePicker();
      return true;
    }
    if (message.type !== 'files-picked' || message.token !== pending.token) {
      return true;
    }
    if (message.cancelled === true) {
      finishPicker([], true);
      return true;
    }
    const received = message.files;
    const base = new URL(`/apps/${encodeURIComponent(pending.provider)}/`, window.location.origin);
    if (
      !Array.isArray(received) ||
      !received.length ||
      received.length > (pending.multiple ? 100 : 1)
    ) {
      finishPicker([], false, 'Invalid file selection');
      return true;
    }
    const files = received.map((item) => pickedFile(item, pending, base));
    if (files.includes(null)) {
      finishPicker([], false, 'Invalid file selection');
      return true;
    }
    finishPicker(files as PickedFile[], false);
    return true;
  }

  function openApp(message: MessageOf<'open'>) {
    const location = message.location;
    shell.open(String(message.app ?? ''), {
      location:
        typeof location === 'string' && location.startsWith('?') && location.length <= 2048
          ? location
          : undefined,
    });
  }

  async function shortcut(id: string, message: MessageOf<'shortcut'>) {
    if (
      id !== 'nas-file-manager' ||
      !validRequest(message.requestId) ||
      typeof message.path !== 'string' ||
      typeof message.name !== 'string'
    ) {
      return;
    }
    const success = await apps.folderShortcut(message.path, message.name);
    post(id, {
      nas: APP_PROTOCOL,
      type: 'shortcut-result',
      requestId: message.requestId,
      success,
      ...(success ? {} : { error: apps.errorMessage }),
    });
  }

  async function pickFiles(id: string, message: MessageOf<'pick-files'>) {
    if (
      validRequest(message.requestId) &&
      (message.multiple === undefined || typeof message.multiple === 'boolean') &&
      (message.folder === undefined || typeof message.folder === 'boolean') &&
      (message.extensions === undefined ||
        (Array.isArray(message.extensions) &&
          message.extensions.length <= 32 &&
          message.extensions.every(
            (extension) => typeof extension === 'string' && /^\.[a-z0-9]{1,10}$/i.test(extension),
          ))) &&
      (message.directory === undefined ||
        (typeof message.directory === 'string' &&
          message.directory.startsWith('/') &&
          message.directory.length <= 2048))
    ) {
      await beginPicker(id, message.requestId, {
        multiple: message.multiple === true,
        extensions: message.extensions,
        directory: message.directory,
        folder: message.folder === true,
      });
    }
  }

  /** Any application may read the network shares: the provider is started when it is stopped. */
  async function networkDisks(id: string, message: MessageOf<'network-disks'>) {
    if (!validRequest(message.requestId)) {
      return;
    }
    await answer(
      id,
      'network-disks-result',
      message.requestId,
      async () => {
        const { url } = await startProvider(
          'network-disks',
          (endpoint) => !endpoint.endsWith('/'),
          'No network disk provider is installed',
        );
        const response = await fetch(url, {
          cache: 'no-store',
          signal: AbortSignal.timeout(10000),
        });
        if (!response.ok) {
          throw new Error('Network disks unavailable');
        }
        return networkDiskList(await response.json());
      },
      'Network disks unavailable',
    );
  }

  /**
   * Any application may read when the NAS switches itself off and on, to plan its own work
   * around it: whether the schedule is on, and its active rules.
   */
  async function powerSchedule(id: string, message: MessageOf<'power-schedule'>) {
    if (!validRequest(message.requestId)) {
      return;
    }
    await answer(
      id,
      'power-schedule-result',
      message.requestId,
      async () => {
        const power = await request<PowerSettings>('/settings/power');
        return {
          enabled: power.enabled,
          rules: power.rules
            .filter((rule) => rule.active)
            .map(({ day, time, action }) => ({ day, time, action })),
        };
      },
      'Power schedule unavailable',
    );
  }

  async function initializeDisk(id: string, message: MessageOf<'initialize-disk'>) {
    if (
      !fromDisks(id, message) ||
      typeof message.label !== 'string' ||
      typeof message.identity !== 'string'
    ) {
      return;
    }
    try {
      const result = await request<{ taskId: string }>('/storage/disks/initialize', {
        method: 'POST',
        body: JSON.stringify({
          device: message.device,
          label: message.label,
          identity: message.identity,
        }),
      });
      await waitForTask(result.taskId);
      post(id, {
        nas: APP_PROTOCOL,
        type: 'initialize-disk-result',
        requestId: message.requestId,
        success: true,
        taskId: result.taskId,
      });
    } catch (reason) {
      post(id, {
        nas: APP_PROTOCOL,
        type: 'initialize-disk-result',
        requestId: message.requestId,
        success: false,
        error: reason instanceof Error ? reason.message : 'Disk initialization unavailable',
      });
    }
  }

  async function renameDisk(id: string, message: MessageOf<'rename-disk'>) {
    if (!fromDisks(id, message) || typeof message.label !== 'string') {
      return;
    }
    try {
      await request('/storage/disks/rename', {
        method: 'POST',
        body: JSON.stringify({ device: message.device, label: message.label }),
      });
      post(id, {
        nas: APP_PROTOCOL,
        type: 'rename-disk-result',
        requestId: message.requestId,
        success: true,
      });
    } catch (reason) {
      post(id, {
        nas: APP_PROTOCOL,
        type: 'rename-disk-result',
        requestId: message.requestId,
        success: false,
        error: reason instanceof Error ? reason.message : 'Volume rename unavailable',
      });
    }
  }

  async function smartDisk(id: string, message: MessageOf<'smart-disk'>) {
    if (fromDisks(id, message)) {
      await answer(
        id,
        'smart-disk-result',
        message.requestId,
        () =>
          request<unknown>('/storage/disks/smart', {
            method: 'POST',
            body: JSON.stringify({ device: message.device }),
          }),
        'S.M.A.R.T. unavailable',
      );
    }
  }

  async function diagnostic(id: string, message: Extract<AppMessage, { type: DiagnosticRequest }>) {
    if (id !== LOG_MANAGER || !validRequest(message.requestId)) {
      return;
    }
    if (message.type === 'diagnostics') {
      await answer(id, 'diagnostics-result', message.requestId, async () =>
        (await request<AppNotification[]>('/notifications?limit=500')).filter(
          (item) => item.tone === 'warning' || item.tone === 'error',
        ),
      );
      return;
    }
    const notification = message.notification;
    if (typeof notification !== 'string' || !NOTIFICATION_ID.test(notification)) {
      return;
    }
    await answer(id, `${message.type}-result`, message.requestId, () =>
      message.type === 'diagnostic-log'
        ? request(`/notifications/${notification}/log`)
        : request(`/notifications/${notification}`, { method: 'DELETE' }),
    );
  }

  async function disks(id: string, message: Extract<AppMessage, { type: DiskRequest }>) {
    const call = diskCall(message.type, message);
    if (id === DISK_MANAGER && validRequest(message.requestId) && call) {
      await answer(id, `${message.type}-result`, message.requestId, call, 'Disks unavailable');
    }
  }

  async function access(id: string, message: Extract<AppMessage, { type: AccessRequest }>) {
    const call = accessCall(message.type, message.payload);
    if (id === SECURITY_CENTER && validRequest(message.requestId) && call) {
      let saved: unknown;
      await answer(
        id,
        `${message.type}-result`,
        message.requestId,
        async () => {
          saved = await call();
          return saved;
        },
        'Access settings unavailable',
      );
      if (message.type === 'save-https-settings' && httpsSettings(saved)) {
        const page = globalThis.location;
        const screen =
          ['localhost', '127.0.0.1', '[::1]'].includes(page.hostname) && page.port === '8081';
        if (page.protocol === 'https:' || (saved.force && !screen)) {
          navigateHttps(saved.port);
        }
      }
    }
  }

  async function monitor(id: string, message: Extract<AppMessage, { type: SystemMonitorRequest }>) {
    const call = monitorCall(message.type, message.payload);
    if (appOf(id) === SYSTEM_MONITOR && validRequest(message.requestId) && call) {
      await answer(
        id,
        `${message.type}-result`,
        message.requestId,
        async () => {
          const data = await call();
          if (
            message.type === 'process-action' &&
            typeof data === 'object' &&
            data !== null &&
            'appId' in data &&
            typeof data.appId === 'string'
          ) {
            shell.remove(data.appId);
          }
          return data;
        },
        'Processes unavailable',
      );
    }
  }

  async function handle(event: MessageEvent, id: string) {
    const message = event.data as AppMessage | null;
    if (!message || typeof message !== 'object' || message.nas !== APP_PROTOCOL) {
      return;
    }

    switch (message.type) {
      case 'ready':
        ready.add(id);
        announce(id);
        break;
      case 'notify':
        await notify(id, message);
        break;
      case 'open':
        openApp(message);
        break;
      case 'open-https':
        openHttps(id);
        break;
      case 'describe':
        post(id, {
          nas: APP_PROTOCOL,
          type: 'described',
          notifications: describe(message.notifications),
        });
        break;
      case 'shortcut':
        await shortcut(id, message);
        break;
      case 'pick-files':
        await pickFiles(id, message);
        break;
      case 'network-disks':
        await networkDisks(id, message);
        break;
      case 'power-schedule':
        await powerSchedule(id, message);
        break;
      case 'initialize-disk':
        await initializeDisk(id, message);
        break;
      case 'rename-disk':
        await renameDisk(id, message);
        break;
      case 'smart-disk':
        await smartDisk(id, message);
        break;
      case 'disk-inventory':
      case 'disk-encryption':
      case 'disk-crypt':
      case 'smb-disks':
      case 'smb-disk':
      case 'eject-disk':
      case 'disk-locks':
      case 'lock-disk':
        await disks(id, message);
        break;
      case 'diagnostics':
      case 'diagnostic-log':
      case 'remove-diagnostic':
        await diagnostic(id, message);
        break;
      case 'system-telemetry':
      case 'process-action':
        await monitor(id, message);
        break;
      default:
        await access(id, message);
    }
  }

  function receiveMessage(event: MessageEvent) {
    if (receivePicker(event)) {
      return;
    }
    for (const [id, frame] of frames) {
      const expectedOrigin = originOf(id);
      if (event.origin === expectedOrigin && event.source === frame.contentWindow) {
        void handle(event, id);
        return;
      }
    }
  }

  window.addEventListener('message', receiveMessage);
  const stop = watch([locale, () => settings.general], broadcast, { deep: true });
  const appearance = new MutationObserver(broadcast);
  appearance.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme', 'data-palette', 'style'],
  });

  onBeforeUnmount(() => {
    window.removeEventListener('message', receiveMessage);
    stop();
    appearance.disconnect();
    frames.clear();
    picker.value = null;
    pickerFrame = null;
  });

  return {
    attach,
    announce,
    ready,
    broadcast,
    picker,
    attachPicker,
    announcePicker,
    cancelPicker: () => finishPicker([], true),
  };
}

/** The bridge of the main page, which the application windows and the widgets share. */
export const APP_BRIDGE: InjectionKey<ReturnType<typeof useAppBridge>> = Symbol('appBridge');
