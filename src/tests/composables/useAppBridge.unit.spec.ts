import { defineComponent } from 'vue';
import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MONITOR_WIDGET, useAppBridge } from '@/composables/useAppBridge';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { request } from '@/api/client';
import { APP_PROTOCOL } from '@/types/types';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const ORIGIN = location.origin;

function frame() {
  const contentWindow = { postMessage: vi.fn() };
  return {
    element: { contentWindow, src: `${ORIGIN}/apps/ext/` } as unknown as HTMLIFrameElement,
    sent: contentWindow,
  };
}

function bridge() {
  let api!: ReturnType<typeof useAppBridge>;
  const wrapper = mount(
    defineComponent({
      setup() {
        api = useAppBridge();
        return () => null;
      },
    }),
  );
  return { api, wrapper };
}

function arrive(data: unknown, origin: string, source: unknown) {
  const event = new MessageEvent('message', { data, origin });
  Object.defineProperty(event, 'source', { value: source });
  window.dispatchEvent(event);
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(request).mockReset().mockResolvedValue(undefined);
  document.documentElement.dataset.theme = 'dark';
});

afterEach(() => vi.unstubAllGlobals());

describe('the bridge between the shell and an application', () => {
  it('lets only nas-disk-manager request initialization and returns its task', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce({ taskId: 'initialize-storage' })
      .mockResolvedValueOnce({ state: 'done', detail: 'ready for use' });
    const { api, wrapper } = bridge();
    const disks = frame();
    const other = frame();
    api.attach('nas-disk-manager', disks.element);
    api.attach('ext', other.element);
    const message = {
      nas: APP_PROTOCOL,
      type: 'initialize-disk',
      requestId: 'initialize-1',
      device: 'sdb',
      label: 'Data-1',
    } as const;

    arrive(message, ORIGIN, other.element.contentWindow);
    expect(request).not.toHaveBeenCalled();
    arrive(message, ORIGIN, disks.element.contentWindow);
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/storage/disks/initialize', {
      method: 'POST',
      body: JSON.stringify({ device: 'sdb', label: 'Data-1' }),
    });
    expect(request).toHaveBeenCalledWith('/tasks/initialize-storage');
    expect(disks.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'initialize-disk-result',
        requestId: 'initialize-1',
        success: true,
        taskId: 'initialize-storage',
      },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('lets only nas-disk-manager rename a data volume', async () => {
    const { api, wrapper } = bridge();
    const disks = frame();
    api.attach('nas-disk-manager', disks.element);
    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'rename-disk',
        requestId: 'rename-1',
        device: 'sdb1',
        label: 'Archive',
      },
      ORIGIN,
      disks.element.contentWindow,
    );
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/storage/disks/rename', {
      method: 'POST',
      body: JSON.stringify({ device: 'sdb1', label: 'Archive' }),
    });
    expect(disks.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'rename-disk-result',
        requestId: 'rename-1',
        success: true,
      },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('reads the diagnostics for nas-log-manager only, and only the warnings and errors', async () => {
    vi.mocked(request).mockResolvedValue([
      { id: '1', tone: 'error' },
      { id: '2', tone: 'info' },
      { id: '3', tone: 'warning' },
    ]);
    const { api, wrapper } = bridge();
    const logs = frame();
    const other = frame();
    api.attach('nas-log-manager', logs.element);
    api.attach('ext', other.element);
    const message = { nas: APP_PROTOCOL, type: 'diagnostics', requestId: 'read-1' };

    arrive(message, ORIGIN, other.element.contentWindow);
    await flushPromises();
    expect(request).not.toHaveBeenCalled();

    arrive(message, ORIGIN, logs.element.contentWindow);
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/notifications?limit=500');
    expect(logs.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'diagnostics-result',
        requestId: 'read-1',
        success: true,
        data: [
          { id: '1', tone: 'error' },
          { id: '3', tone: 'warning' },
        ],
      },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('opens and removes a notification for nas-log-manager, and says why it could not', async () => {
    const { api, wrapper } = bridge();
    const logs = frame();
    api.attach('nas-log-manager', logs.element);
    const ask = (type: string, notification: string, requestId: string) =>
      arrive(
        { nas: APP_PROTOCOL, type, requestId, notification },
        ORIGIN,
        logs.element.contentWindow,
      );

    ask('remove-diagnostic', '../settings', 'bad-1');
    await flushPromises();
    expect(request).not.toHaveBeenCalled();

    vi.mocked(request).mockRejectedValueOnce(new Error('no log for this notification'));
    ask('diagnostic-log', '12', 'log-1');
    ask('remove-diagnostic', '12', 'remove-1');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/notifications/12/log');
    expect(request).toHaveBeenCalledWith('/notifications/12', { method: 'DELETE' });
    expect(logs.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'diagnostic-log-result',
        requestId: 'log-1',
        success: false,
        error: 'no log for this notification',
      },
      ORIGIN,
    );
    expect(logs.sent.postMessage).toHaveBeenCalledWith(
      { nas: APP_PROTOCOL, type: 'remove-diagnostic-result', requestId: 'remove-1', success: true },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('returns S.M.A.R.T. data only to nas-disk-manager', async () => {
    vi.mocked(request).mockResolvedValueOnce({ smart_status: { passed: true } });
    const { api, wrapper } = bridge();
    const disks = frame();
    api.attach('nas-disk-manager', disks.element);
    arrive(
      { nas: APP_PROTOCOL, type: 'smart-disk', requestId: 'smart-1', device: 'sdb' },
      ORIGIN,
      disks.element.contentWindow,
    );
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/storage/disks/smart', {
      method: 'POST',
      body: JSON.stringify({ device: 'sdb' }),
    });
    expect(disks.sent.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'smart-disk-result',
        requestId: 'smart-1',
        success: true,
        data: { smart_status: { passed: true } },
      }),
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('ejects and marks the external disks only for nas-disk-manager', async () => {
    vi.mocked(request).mockResolvedValue(['ab12-cd34']);
    const { api, wrapper } = bridge();
    const disks = frame();
    const other = frame();
    api.attach('nas-disk-manager', disks.element);
    api.attach('nas-file-manager', other.element);

    arrive(
      { nas: APP_PROTOCOL, type: 'eject-disk', requestId: 'eject-1', device: 'sdb' },
      ORIGIN,
      other.element.contentWindow,
    );
    arrive(
      { nas: APP_PROTOCOL, type: 'eject-disk', requestId: 'eject-2', device: '../sda' },
      ORIGIN,
      disks.element.contentWindow,
    );
    await flushPromises();
    expect(request).not.toHaveBeenCalled();

    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'lock-disk',
        requestId: 'lock-1',
        uuid: 'ab12-cd34',
        locked: true,
      },
      ORIGIN,
      disks.element.contentWindow,
    );
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/storage/locks', {
      method: 'POST',
      body: JSON.stringify({ uuid: 'ab12-cd34', locked: true }),
    });
    expect(disks.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'lock-disk-result',
        requestId: 'lock-1',
        success: true,
        data: ['ab12-cd34'],
      },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('opens the first declared file picker and returns a provider download URL to the requesting app', async () => {
    const apps = useAppsStore();
    apps.installed = [
      { id: 'ext', name: 'Requester', info: '', system: false },
      { id: 'nas-file-manager', name: 'Explorer', info: '', system: false },
    ];
    vi.mocked(request).mockResolvedValueOnce([
      { app: 'nas-file-manager', endpoint: '/', version: 1 },
    ]);
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('ok')));
    const { api, wrapper } = bridge();
    const caller = frame();
    api.attach('ext', caller.element);
    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'pick-files',
        requestId: 'pick-1',
        extensions: ['.pdf'],
        directory: '/docs',
      },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/capabilities/file-picker');
    expect(api.picker.value?.provider).toBe('nas-file-manager');
    expect(api.picker.value?.url).toContain('/apps/nas-file-manager/?nasPicker=');
    const pickerUrl = new URL(api.picker.value!.url);
    expect(pickerUrl.searchParams.get('extensions')).toBe('.pdf');
    expect(pickerUrl.searchParams.get('directory')).toBe('/docs');

    const picker = frame();
    api.attachPicker(picker.element);
    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'files-picked',
        token: api.picker.value?.token,
        files: [
          {
            name: 'report.pdf',
            path: '/docs/report.pdf',
            download: 'api/downloads?path=docs%2Freport.pdf',
          },
        ],
      },
      ORIGIN,
      picker.element.contentWindow,
    );
    expect(caller.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'pick-files-result',
        requestId: 'pick-1',
        files: [
          {
            name: 'report.pdf',
            path: '/docs/report.pdf',
            url: `${ORIGIN}/apps/nas-file-manager/api/downloads?path=docs%2Freport.pdf`,
          },
        ],
        cancelled: false,
      },
      ORIGIN,
    );
    expect(api.picker.value).toBeNull();
    wrapper.unmount();
    vi.unstubAllGlobals();
  });

  it('reports when no file picker provider is installed', async () => {
    vi.mocked(request).mockResolvedValueOnce([]);
    const { api, wrapper } = bridge();
    const caller = frame();
    api.attach('ext', caller.element);
    arrive(
      { nas: APP_PROTOCOL, type: 'pick-files', requestId: 'pick-2' },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();
    expect(caller.sent.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'pick-files-result',
        requestId: 'pick-2',
        files: [],
        cancelled: false,
        error: 'No file picker provider is installed',
      }),
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('rejects malformed filters and files outside the requested extensions', async () => {
    const apps = useAppsStore();
    apps.installed = [
      { id: 'ext', name: 'Requester', info: '', system: false },
      { id: 'nas-file-manager', name: 'Explorer', info: '', system: false },
    ];
    const { api, wrapper } = bridge();
    const caller = frame();
    api.attach('ext', caller.element);
    arrive(
      { nas: APP_PROTOCOL, type: 'pick-files', requestId: 'invalid', extensions: ['png'] },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();
    expect(api.picker.value).toBeNull();
    vi.mocked(request).mockResolvedValueOnce([
      { app: 'nas-file-manager', endpoint: '/', version: 1 },
    ]);
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('ok')));
    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'pick-files',
        requestId: 'filtered',
        multiple: true,
        extensions: ['.png'],
      },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();
    expect(new URL(api.picker.value!.url).searchParams.get('multiple')).toBe('1');
    const picker = frame();
    api.attachPicker(picker.element);
    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'files-picked',
        token: api.picker.value?.token,
        files: [
          { name: 'report.pdf', path: '/report.pdf', download: 'api/downloads?path=report.pdf' },
        ],
      },
      ORIGIN,
      picker.element.contentWindow,
    );
    expect(caller.sent.postMessage).toHaveBeenLastCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'pick-files-result',
        requestId: 'filtered',
        files: [],
        cancelled: false,
        error: 'Invalid file selection',
      },
      ORIGIN,
    );
    wrapper.unmount();
    vi.unstubAllGlobals();
  });

  it('opens the picker on folders and returns the chosen folder without a download URL', async () => {
    const apps = useAppsStore();
    apps.installed = [
      { id: 'ext', name: 'Requester', info: '', system: false },
      { id: 'nas-file-manager', name: 'Explorer', info: '', system: false },
    ];
    vi.mocked(request).mockResolvedValueOnce([
      { app: 'nas-file-manager', endpoint: '/', version: 1 },
    ]);
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('ok')));
    const { api, wrapper } = bridge();
    const caller = frame();
    api.attach('ext', caller.element);
    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'pick-files',
        requestId: 'folder-1',
        folder: true,
        multiple: true,
        extensions: ['.pdf'],
      },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();
    const pickerUrl = new URL(api.picker.value!.url);
    expect(pickerUrl.searchParams.get('folder')).toBe('1');
    expect(pickerUrl.searchParams.has('multiple')).toBe(false);
    expect(pickerUrl.searchParams.has('extensions')).toBe(false);

    const picker = frame();
    api.attachPicker(picker.element);
    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'files-picked',
        token: api.picker.value?.token,
        files: [{ name: 'Photos', path: '/Photos' }],
      },
      ORIGIN,
      picker.element.contentWindow,
    );
    expect(caller.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'pick-files-result',
        requestId: 'folder-1',
        files: [{ name: 'Photos', path: '/Photos' }],
        cancelled: false,
      },
      ORIGIN,
    );
    wrapper.unmount();
    vi.unstubAllGlobals();
  });

  it('shares the active power rules with any application', async () => {
    const apps = useAppsStore();
    apps.installed = [{ id: 'ext', name: 'Requester', info: '', system: false }];
    vi.mocked(request).mockResolvedValueOnce({
      enabled: true,
      rules: [
        { day: 'everyday', time: '23:00', action: 'shutdown', active: true },
        { day: 'monday', time: '07:00', action: 'start', active: false },
      ],
    });
    const { api, wrapper } = bridge();
    const caller = frame();
    api.attach('ext', caller.element);
    arrive(
      { nas: APP_PROTOCOL, type: 'power-schedule', requestId: 'power-1' },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/settings/power');
    expect(caller.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'power-schedule-result',
        requestId: 'power-1',
        success: true,
        data: { enabled: true, rules: [{ day: 'everyday', time: '23:00', action: 'shutdown' }] },
      },
      ORIGIN,
    );

    vi.mocked(request).mockClear();
    arrive({ nas: APP_PROTOCOL, type: 'power-schedule' }, ORIGIN, caller.element.contentWindow);
    await flushPromises();
    expect(request).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('reads the network disks from their provider for any application', async () => {
    const apps = useAppsStore();
    apps.installed = [
      { id: 'ext', name: 'Requester', info: '', system: false },
      { id: 'nas-disk-manager', name: 'Disks', info: '', system: true },
    ];
    vi.mocked(request).mockResolvedValueOnce([
      { app: 'nas-disk-manager', endpoint: '/api/network-disks', version: 1 },
    ]);
    const shares = [
      { name: 'backup', mount: '/mnt/backup', source: '//nas/backup', read_only: false, size: 0 },
    ];
    const fetcher = vi.fn().mockImplementation(() => Promise.resolve(Response.json(shares)));
    vi.stubGlobal('fetch', fetcher);
    const { api, wrapper } = bridge();
    const caller = frame();
    api.attach('ext', caller.element);
    arrive(
      { nas: APP_PROTOCOL, type: 'network-disks', requestId: 'shares-1' },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/capabilities/network-disks');
    expect(String(fetcher.mock.lastCall?.[0])).toBe(
      `${ORIGIN}/apps/nas-disk-manager/api/network-disks`,
    );
    expect(caller.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'network-disks-result',
        requestId: 'shares-1',
        success: true,
        data: [{ name: 'backup', mount: '/mnt/backup', source: '//nas/backup', readOnly: false }],
      },
      ORIGIN,
    );

    vi.mocked(request).mockResolvedValueOnce([]);
    arrive(
      { nas: APP_PROTOCOL, type: 'network-disks', requestId: 'shares-2' },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();
    expect(caller.sent.postMessage).toHaveBeenLastCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'network-disks-result',
        requestId: 'shares-2',
        success: false,
        error: 'No network disk provider is installed',
      },
      ORIGIN,
    );

    vi.mocked(request).mockResolvedValueOnce([
      { app: 'nas-disk-manager', endpoint: '/api/network-disks', version: 1 },
    ]);
    fetcher.mockImplementation(() => Promise.resolve(Response.json([{ name: 'x', mount: 'x' }])));
    arrive(
      { nas: APP_PROTOCOL, type: 'network-disks', requestId: 'shares-3' },
      ORIGIN,
      caller.element.contentWindow,
    );
    await flushPromises();
    expect(caller.sent.postMessage).toHaveBeenLastCalledWith(
      expect.objectContaining({ requestId: 'shares-3', error: 'Invalid network disk list' }),
      ORIGIN,
    );
    wrapper.unmount();
    vi.unstubAllGlobals();
  });

  it('persists a folder shortcut only for Explorer and answers after the API succeeds', async () => {
    const { api, wrapper } = bridge();
    const { element, sent } = frame();
    api.attach('nas-file-manager', element);
    vi.mocked(request).mockResolvedValueOnce([
      { kind: 'folder', target: '/Documents', name: 'Documents', position: 0 },
    ]);
    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'shortcut',
        requestId: 'one',
        path: '/Documents',
        name: 'Documents',
      },
      ORIGIN,
      element.contentWindow,
    );
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/desktop/folders', {
      method: 'POST',
      body: JSON.stringify({ path: '/Documents', name: 'Documents' }),
    });
    expect(sent.postMessage).toHaveBeenCalledWith(
      { nas: APP_PROTOCOL, type: 'shortcut-result', requestId: 'one', success: true },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('does not accept folder requests from another application or another frame', async () => {
    const { api, wrapper } = bridge();
    const { element } = frame();
    api.attach('ext', element);
    const payload = {
      nas: APP_PROTOCOL,
      type: 'shortcut',
      requestId: 'two',
      path: '/Documents',
      name: 'Documents',
    };
    arrive(payload, ORIGIN, element.contentWindow);
    api.attach('nas-file-manager', element);
    arrive(payload, ORIGIN, { postMessage: vi.fn() });
    await flushPromises();
    expect(request).not.toHaveBeenCalled();
    wrapper.unmount();
  });
  it('tells an application how it is being shown, on its own origin and never on a wildcard', () => {
    const { api, wrapper } = bridge();
    const { element, sent } = frame();
    api.attach('ext', element);

    api.announce('ext');

    expect(sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'context',
        context: expect.objectContaining({
          app: 'ext',
          nasName: 'nas',
          theme: 'dark',
          accent: '',
          locale: 'it',
        }),
      },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('answers an application that says it is ready', () => {
    const { api, wrapper } = bridge();
    const { element, sent } = frame();
    api.attach('ext', element);

    arrive({ nas: APP_PROTOCOL, type: 'ready' }, ORIGIN, element.contentWindow);

    expect(sent.postMessage).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });

  it('describes the notifications an application read, in the language of the page', () => {
    setLocale('it');
    const { api, wrapper } = bridge();
    const { element, sent } = frame();
    api.attach('ext', element);

    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'describe',
        notifications: [
          {
            id: '7',
            title: 'Launcher updated',
            message: 'x',
            source: 'nas-system-settings',
            key: 'launcherUpdated',
            params: { version: '0.2.0' },
          },
          'not a notification',
        ],
      },
      ORIGIN,
      element.contentWindow,
    );

    expect(sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'described',
        notifications: [
          {
            id: '7',
            title: 'Launcher aggiornato',
            message: 'Launcher aggiornato alla versione 0.2.0.',
            source: 'Impostazioni',
          },
        ],
      },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it.each([
    ['another origin', 'http://elsewhere.example', true],
    ['a window that is not the frame', ORIGIN, false],
  ])('ignores a message from %s', (_case, origin, ownWindow) => {
    const { api, wrapper } = bridge();
    const { element, sent } = frame();
    api.attach('ext', element);

    arrive(
      { nas: APP_PROTOCOL, type: 'ready' },
      origin,
      ownWindow ? element.contentWindow : { postMessage: vi.fn() },
    );

    expect(sent.postMessage).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('ignores whatever is posted into the page without the mark of this protocol', () => {
    const { api, wrapper } = bridge();
    const { element, sent } = frame();
    api.attach('ext', element);

    arrive({ type: 'ready' }, ORIGIN, element.contentWindow);
    arrive('vite:hmr', ORIGIN, element.contentWindow);

    expect(sent.postMessage).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('records a notification in the name of the application that asked for it', async () => {
    const { api, wrapper } = bridge();
    const { element } = frame();
    api.attach('ext', element);

    arrive(
      {
        nas: APP_PROTOCOL,
        type: 'notify',
        title: 'Copy finished',
        message: '30 GB',
        tone: 'success',
        source: 'nas-app-manager',
        log: '/var/lib/nas-app/ext/copy.log',
      },
      ORIGIN,
      element.contentWindow,
    );
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/notifications', {
      method: 'POST',
      body: JSON.stringify({
        title: 'Copy finished',
        message: '30 GB',
        source: 'ext',
        tone: 'success',
        log: '/var/lib/nas-app/ext/copy.log',
      }),
    });
    wrapper.unmount();
  });

  it('opens a window for an application that is installed, and no window for one that is not', () => {
    const apps = useAppsStore();
    const shell = useShellStore();
    apps.installed = [{ id: 'other', name: 'Other', info: '', system: false }];
    const { api, wrapper } = bridge();
    const { element } = frame();
    api.attach('ext', element);

    arrive({ nas: APP_PROTOCOL, type: 'open', app: 'nothing' }, ORIGIN, element.contentWindow);
    expect(shell.activeId).toBeNull();

    arrive({ nas: APP_PROTOCOL, type: 'open', app: 'other' }, ORIGIN, element.contentWindow);
    expect(shell.activeId).toBe('other');
    arrive(
      { nas: APP_PROTOCOL, type: 'open', app: 'other', location: '?path=%2Fphoto.png' },
      ORIGIN,
      element.contentWindow,
    );
    expect(shell.locations.other).toBe('?path=%2Fphoto.png');
    arrive(
      { nas: APP_PROTOCOL, type: 'open', app: 'other', location: '/api/other' },
      ORIGIN,
      element.contentWindow,
    );
    expect(shell.locations.other).toBe('?path=%2Fphoto.png');
    wrapper.unmount();
  });

  it('loads the console again over HTTPS for the security center alone, and only from HTTP', () => {
    const assign = vi.fn();
    vi.stubGlobal('location', {
      protocol: 'http:',
      hostname: '192.168.1.20',
      pathname: '/',
      search: '?app=x',
      assign,
    });
    const { api, wrapper } = bridge();
    const other = frame();
    const security = frame();
    api.attach('ext', other.element);
    api.attach('nas-security-center', security.element);

    arrive({ nas: APP_PROTOCOL, type: 'open-https' }, ORIGIN, other.element.contentWindow);
    expect(assign).not.toHaveBeenCalled();
    arrive({ nas: APP_PROTOCOL, type: 'open-https' }, ORIGIN, security.element.contentWindow);
    expect(assign).toHaveBeenCalledWith('https://192.168.1.20/?app=x');

    vi.stubGlobal('location', { protocol: 'https:', hostname: 'nas', pathname: '/', assign });
    arrive({ nas: APP_PROTOCOL, type: 'open-https' }, ORIGIN, security.element.contentWindow);
    expect(assign).toHaveBeenCalledTimes(1);
    vi.unstubAllGlobals();
    wrapper.unmount();
  });

  it('stops listening once the shell is gone', () => {
    const { api, wrapper } = bridge();
    const { element, sent } = frame();
    api.attach('ext', element);
    wrapper.unmount();

    arrive({ nas: APP_PROTOCOL, type: 'ready' }, ORIGIN, element.contentWindow);

    expect(sent.postMessage).not.toHaveBeenCalled();
  });

  it('tells an application the colours of the theme drawn now', () => {
    const root = document.documentElement;
    root.style.setProperty('--theme-background', '#f6f1e7');
    root.style.setProperty('--surface-muted', '#6f6150');
    const { api, wrapper } = bridge();
    const app = frame();
    api.attach('nas-file-manager', app.element);

    api.announce('nas-file-manager');
    expect(app.sent.postMessage).toHaveBeenLastCalledWith(
      expect.objectContaining({
        context: expect.objectContaining({
          palette: expect.objectContaining({ background: '#f6f1e7', muted: '#6f6150' }),
        }),
      }),
      ORIGIN,
    );
    root.style.removeProperty('--theme-background');
    root.style.removeProperty('--surface-muted');
    wrapper.unmount();
  });

  it('answers the system monitor and its widget about the processes, and nobody else', async () => {
    vi.mocked(request).mockResolvedValue({ processes: [] });
    const { api, wrapper } = bridge();
    const window_ = frame();
    const widget = frame();
    const other = frame();
    api.attach('nas-system-monitor', window_.element);
    api.attach(MONITOR_WIDGET, widget.element);
    api.attach('nas-log-manager', other.element);
    const telemetry = { nas: APP_PROTOCOL, type: 'system-telemetry', requestId: 'monitor-1' };

    arrive(telemetry, ORIGIN, other.element.contentWindow);
    await flushPromises();
    expect(request).not.toHaveBeenCalled();

    arrive(telemetry, ORIGIN, widget.element.contentWindow);
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/system/telemetry');
    expect(widget.sent.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'system-telemetry-result', success: true }),
      ORIGIN,
    );

    // The widget is told it belongs to the application, like its window.
    api.announce(MONITOR_WIDGET);
    expect(widget.sent.postMessage).toHaveBeenLastCalledWith(
      expect.objectContaining({
        type: 'context',
        context: expect.objectContaining({ app: 'nas-system-monitor' }),
      }),
      ORIGIN,
    );

    vi.mocked(request).mockClear();
    const act = (requestId: string, payload: unknown) =>
      arrive(
        { nas: APP_PROTOCOL, type: 'process-action', requestId, payload },
        ORIGIN,
        window_.element.contentWindow,
      );
    act('bad-1', { id: 0, started: 1, action: 'terminate' });
    act('bad-2', { id: 4, started: 1, action: 'kill' });
    await flushPromises();
    expect(request).not.toHaveBeenCalled();

    act('act-1', { id: 4, started: 900, action: 'restart' });
    await flushPromises();
    expect(request).toHaveBeenCalledWith(
      '/system/processes/4',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ action: 'restart', started: 900 }),
      }),
    );
    wrapper.unmount();
  });

  it.each(['nas-system-monitor', MONITOR_WIDGET])(
    'closes an application tab only after its service is stopped through %s',
    async (source) => {
      vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response()));
      const apps = useAppsStore();
      apps.installed = [
        { id: 'nas-app-manager', name: 'Apps', info: '', system: true },
        { id: 'nas-picture-manager', name: 'Pictures', info: '', system: false },
      ];
      const shell = useShellStore();
      shell.open('nas-app-manager');
      shell.open('nas-picture-manager', { location: '?image=test' });
      await flushPromises();
      const { api, wrapper } = bridge();
      const monitor = frame();
      api.attach(source, monitor.element);
      let finish!: (value: unknown) => void;
      vi.mocked(request)
        .mockClear()
        .mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
      arrive(
        {
          nas: APP_PROTOCOL,
          type: 'process-action',
          requestId: 'terminate-picture',
          payload: { id: 400, started: 900, action: 'terminate' },
        },
        ORIGIN,
        monitor.element.contentWindow,
      );
      await flushPromises();
      expect(shell.openApps.map((app) => app.id)).toContain('nas-picture-manager');
      finish({ appId: 'nas-picture-manager' });
      await flushPromises();
      expect(shell.openApps.map((app) => app.id)).toEqual(['nas-app-manager']);
      expect(shell.activeId).toBe('nas-app-manager');
      expect(shell.locations['nas-picture-manager']).toBeUndefined();
      expect(sessionStorage.getItem('nas-shell-open-apps-v1')).not.toContain('nas-picture-manager');
      expect(request).toHaveBeenCalledTimes(1);
      expect(monitor.sent.postMessage).toHaveBeenCalledWith(
        expect.objectContaining({ type: 'process-action-result', success: true }),
        ORIGIN,
      );
      wrapper.unmount();
    },
  );

  it.each(['restart', 'unrelated', 'failed'])(
    'keeps application tabs for a %s process action',
    async (result) => {
      vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response()));
      useAppsStore().installed = [
        { id: 'nas-picture-manager', name: 'Pictures', info: '', system: false },
      ];
      const shell = useShellStore();
      shell.open('nas-picture-manager');
      await flushPromises();
      const { api, wrapper } = bridge();
      const monitor = frame();
      api.attach(MONITOR_WIDGET, monitor.element);
      if (result === 'failed') {
        vi.mocked(request).mockRejectedValueOnce(new Error('service stop failed'));
      } else {
        vi.mocked(request).mockResolvedValueOnce({ appId: null });
      }
      arrive(
        {
          nas: APP_PROTOCOL,
          type: 'process-action',
          requestId: 'act-picture',
          payload: {
            id: 400,
            started: 900,
            action: result === 'restart' ? 'restart' : 'terminate',
            appId: 'nas-picture-manager',
          },
        },
        ORIGIN,
        monitor.element.contentWindow,
      );
      await flushPromises();
      expect(shell.openApps.map((app) => app.id)).toEqual(['nas-picture-manager']);
      expect(monitor.sent.postMessage).toHaveBeenCalledWith(
        expect.objectContaining({ type: 'process-action-result', success: result !== 'failed' }),
        ORIGIN,
      );
      wrapper.unmount();
    },
  );

  it('answers the security center about the access, and nobody else', async () => {
    vi.mocked(request).mockResolvedValue({ notifyFailures: true });
    const { api, wrapper } = bridge();
    const security = frame();
    const other = frame();
    api.attach('nas-security-center', security.element);
    api.attach('nas-log-manager', other.element);
    const message = { nas: APP_PROTOCOL, type: 'access-settings', requestId: 'access-1' };

    arrive(message, ORIGIN, other.element.contentWindow);
    await flushPromises();
    expect(request).not.toHaveBeenCalled();

    arrive(message, ORIGIN, security.element.contentWindow);
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/settings/access');
    expect(security.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'access-settings-result',
        requestId: 'access-1',
        success: true,
        data: { notifyFailures: true },
      },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('lets only the security center switch mTLS, after password confirmation, and returns gateway refusals', async () => {
    const state = { enabled: true, available: true, current: { id: 1 } };
    const fetcher = vi
      .fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({ code: 'confirmed' })))
      .mockResolvedValueOnce(new Response(JSON.stringify(state)))
      .mockResolvedValueOnce(new Response(JSON.stringify({ code: 'confirmed-again' })))
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({ error: 'this device has no certificate of the NAS: install one first' }),
          { status: 409 },
        ),
      );
    vi.stubGlobal('fetch', fetcher);
    const { api, wrapper } = bridge();
    const security = frame();
    const other = frame();
    api.attach('nas-security-center', security.element);
    api.attach('ext', other.element);
    const message = {
      nas: APP_PROTOCOL,
      type: 'save-mtls',
      requestId: 'mtls-1',
      payload: { enabled: true, password: 'long enough' },
    };

    arrive(message, ORIGIN, other.element.contentWindow);
    arrive(message, 'https://elsewhere.example', security.element.contentWindow);
    await flushPromises();
    expect(fetcher).not.toHaveBeenCalled();

    arrive(message, ORIGIN, security.element.contentWindow);
    await flushPromises();
    expect(fetcher).toHaveBeenNthCalledWith(
      1,
      '/auth/confirm',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({ password: 'long enough' }),
      }),
    );
    expect(fetcher).toHaveBeenNthCalledWith(
      2,
      '/auth/mtls',
      expect.objectContaining({
        method: 'PUT',
        credentials: 'include',
        body: JSON.stringify({ enabled: true }),
        headers: { 'Content-Type': 'application/json', 'X-NAS-Confirmation': 'confirmed' },
      }),
    );
    expect(security.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'save-mtls-result',
        requestId: 'mtls-1',
        success: true,
        data: state,
      },
      ORIGIN,
    );

    arrive({ ...message, requestId: 'mtls-2' }, ORIGIN, security.element.contentWindow);
    await flushPromises();
    expect(security.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'save-mtls-result',
        requestId: 'mtls-2',
        success: false,
        error: 'this device has no certificate of the NAS: install one first',
      },
      ORIGIN,
    );
    wrapper.unmount();
  });

  it('ignores a malformed access request and says why a refused one failed', async () => {
    const { api, wrapper } = bridge();
    const security = frame();
    api.attach('nas-security-center', security.element);
    const ask = (requestId: string, payload: unknown) =>
      arrive(
        { nas: APP_PROTOCOL, type: 'remove-access-rule', requestId, payload },
        ORIGIN,
        security.element.contentWindow,
      );

    ask('bad-1', { id: '../settings' });
    await flushPromises();
    expect(request).not.toHaveBeenCalled();
    expect(security.sent.postMessage).not.toHaveBeenCalledWith(
      expect.objectContaining({ requestId: 'bad-1' }),
      ORIGIN,
    );

    vi.mocked(request).mockRejectedValueOnce(new Error('access rule'));
    ask('remove-1', { id: '7' });
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/access/rules/7', { method: 'DELETE' });
    expect(security.sent.postMessage).toHaveBeenCalledWith(
      {
        nas: APP_PROTOCOL,
        type: 'remove-access-rule-result',
        requestId: 'remove-1',
        success: false,
        error: 'access rule',
      },
      ORIGIN,
    );
    wrapper.unmount();
  });
});

it('accepts antivirus requests only from the registered Security Center frame', async () => {
  const { api, wrapper } = bridge();
  const security = frame();
  const other = frame();
  api.attach('nas-security-center', security.element);
  api.attach('nas-log-manager', other.element);
  const message = {
    nas: APP_PROTOCOL,
    type: 'antivirus-scan',
    requestId: 'antivirus-1',
    payload: { kind: 'quick', paths: [] },
  };
  arrive(message, ORIGIN, other.element.contentWindow);
  await flushPromises();
  expect(request).not.toHaveBeenCalled();
  arrive(message, ORIGIN, security.element.contentWindow);
  await flushPromises();
  expect(request).toHaveBeenCalledWith('/antivirus/scan', {
    method: 'POST',
    body: '{"kind":"quick","paths":[]}',
  });
  expect(security.sent.postMessage).toHaveBeenCalledWith(
    expect.objectContaining({ type: 'antivirus-scan-result', success: true }),
    ORIGIN,
  );
  wrapper.unmount();
});
