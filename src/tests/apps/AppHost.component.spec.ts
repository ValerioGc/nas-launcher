import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import AppHost from '@/apps/AppHost.vue';
import AppTabs from '@/apps/AppTabs.vue';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import { request } from '@/api/client';
import { APP_BRIDGE, type useAppBridge } from '@/composables/useAppBridge';
import { ref } from 'vue';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

afterEach(() => vi.unstubAllGlobals());

vi.mock('@/apps/registry', async () => {
  const { default: component } = await import('@tests/fixtures/StatefulApp.vue');
  return {
    appLabel: (_id: string, fallback: string) => fallback,
    appIcon: (_id: string, packaged?: string) => packaged || '/icon.svg',
    resolveApp: (id: string) => ({
      presentation: 'fullscreen',
      icon: '/icon.svg',
      source:
        id === 'ext' ? { kind: 'external', url: '/apps/ext/' } : { kind: 'internal', component },
    }),
  };
});

const probe = vi.fn<typeof fetch>();
const resize = { observe: vi.fn(), disconnect: vi.fn() };
let onResize: ResizeObserverCallback;

beforeEach(() => {
  resize.observe.mockClear();
  resize.disconnect.mockClear();
  vi.stubGlobal(
    'ResizeObserver',
    class {
      constructor(callback: ResizeObserverCallback) {
        onResize = callback;
      }
      observe = resize.observe;
      disconnect = resize.disconnect;
    },
  );
  sessionStorage.clear();
  document.documentElement.dataset.theme = 'light';
  document.documentElement.lang = 'it';
  setActivePinia(createPinia());
  vi.mocked(request).mockReset();
  vi.mocked(request).mockResolvedValue(undefined as never);
  probe.mockReset().mockResolvedValue(new Response());
  vi.stubGlobal('fetch', probe);
});

describe('multiple app views', () => {
  it('keeps draft content mounted when switching tabs, and removes closed views', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce([
        { id: 'a', name: 'A', info: '' },
        { id: 'b', name: 'B', info: '' },
      ])
      .mockResolvedValueOnce([]);

    const apps = useAppsStore();
    const store = useShellStore();
    await apps.fetchInstalled();
    store.open('a');
    const host = mount(AppHost);
    const tabs = mount(AppTabs);
    await host.get('input').setValue('unsaved');
    store.open('b');
    await flushPromises();
    expect(host.findAll('input')).toHaveLength(2);
    store.open('a');
    await flushPromises();
    expect(host.findAll<HTMLInputElement>('input')[0]?.element.value).toBe('unsaved');
    const close = tabs.findAll('button').find((b) => b.attributes('aria-label') === 'Chiudi A');
    await close?.trigger('click');
    expect(store.activeId).toBe('b');
    expect(host.findAll('input')).toHaveLength(1);
    host.unmount();
    tabs.unmount();
  });

  it('opens an external application in a frame under its path, with no sandbox', async () => {
    vi.stubGlobal('crypto', { getRandomValues: crypto.getRandomValues.bind(crypto) });
    document.documentElement.dataset.theme = 'dark';
    document.documentElement.lang = 'en';
    const apps = useAppsStore();
    const store = useShellStore();
    apps.installed = [{ id: 'ext', name: 'Esplora file', info: '', system: false }];
    store.open('ext');
    const host = mount(AppHost);
    await flushPromises();

    const frame = host.get('[data-testid="app_frame_ext"]');
    const frameUrl = new URL(frame.attributes('src')!, window.location.origin);
    expect(frameUrl.pathname).toBe('/apps/ext/');
    expect(frameUrl.searchParams.get('launch')).toBeTruthy();
    expect(frameUrl.searchParams.get('theme')).toBe('dark');
    expect(frameUrl.searchParams.get('lang')).toBe('en');
    expect(frame.attributes('title')).toBe('Esplora file');

    expect(frame.attributes('sandbox')).toBeUndefined();
    expect(frame.attributes('referrerpolicy')).toBe('no-referrer');
    expect(host.find('input').exists()).toBe(false);
    host.unmount();
  });

  it('waits for the external process to start and offers a retry when it fails', async () => {
    const apps = useAppsStore();
    const store = useShellStore();
    apps.installed = [{ id: 'ext', name: 'Esplora file', info: '', system: false }];
    probe.mockRejectedValueOnce(new Error('not running'));
    vi.mocked(request).mockRejectedValueOnce(new Error('Could not start'));
    store.open('ext');
    const host = mount(AppHost);
    await flushPromises();

    expect(host.find('[data-testid="app_frame_ext"]').exists()).toBe(false);
    const error = host.get('[data-testid="app_start_error_ext"]');
    expect(error.text()).toContain('Could not start');
    await error.get('button').trigger('click');
    await flushPromises();
    expect(host.find('[data-testid="app_frame_ext"]').exists()).toBe(true);
    host.unmount();
  });

  it('opens an external application on the page it was asked for', async () => {
    const apps = useAppsStore();
    const store = useShellStore();
    apps.installed = [{ id: 'ext', name: 'Log', info: '', system: false }];
    store.open('ext', { location: '?notification=7' });
    const host = mount(AppHost);
    await flushPromises();

    const firstUrl = new URL(
      host.get('[data-testid="app_frame_ext"]').attributes('src')!,
      window.location.origin,
    );
    expect(firstUrl.searchParams.get('notification')).toBe('7');
    const firstLaunch = firstUrl.searchParams.get('launch');
    store.open('ext', { location: '?path=%2FDocuments' });
    await flushPromises();
    const nextUrl = new URL(
      host.get('[data-testid="app_frame_ext"]').attributes('src')!,
      window.location.origin,
    );
    expect(nextUrl.searchParams.get('path')).toBe('/Documents');
    expect(nextUrl.searchParams.get('launch')).toBe(firstLaunch);
    expect(store.openApps.filter((app) => app.id === 'ext')).toHaveLength(1);

    await store.close('ext');
    store.open('ext');
    await flushPromises();
    const reopenedUrl = new URL(
      host.get('[data-testid="app_frame_ext"]').attributes('src')!,
      window.location.origin,
    );
    expect(reopenedUrl.searchParams.get('launch')).not.toBe(firstLaunch);
    host.unmount();
  });

  it('covers an external application with a spinner until it says it is ready', async () => {
    const apps = useAppsStore();
    const store = useShellStore();
    apps.installed = [{ id: 'ext', name: 'Esplora file', info: '', system: false }];
    store.open('ext');
    const host = mount(AppHost, { attachTo: document.body });
    await flushPromises();

    const cover = host.get('[data-testid="app_loading_ext"]');
    expect(cover.text()).toBe('Apertura di Esplora file in corso');
    const frame = host.get<HTMLIFrameElement>('[data-testid="app_frame_ext"]');
    await frame.trigger('load');
    expect(host.find('[data-testid="app_loading_ext"]').exists()).toBe(true);

    const ready = new MessageEvent('message', {
      data: { nas: 1, type: 'ready' },
      origin: window.location.origin,
    });
    Object.defineProperty(ready, 'source', { value: frame.element.contentWindow });
    window.dispatchEvent(ready);
    await flushPromises();
    expect(host.find('[data-testid="app_loading_ext"]').exists()).toBe(false);
    host.unmount();
  });

  it('shows an application that never says it is ready a few seconds after its page loaded', async () => {
    vi.useFakeTimers();
    const apps = useAppsStore();
    const store = useShellStore();
    apps.installed = [{ id: 'ext', name: 'Esplora file', info: '', system: false }];
    store.open('ext');
    const host = mount(AppHost);
    await flushPromises();

    await host.get('[data-testid="app_frame_ext"]').trigger('load');
    await vi.advanceTimersByTimeAsync(3999);
    expect(host.find('[data-testid="app_loading_ext"]').exists()).toBe(true);
    await vi.advanceTimersByTimeAsync(1);
    expect(host.find('[data-testid="app_loading_ext"]').exists()).toBe(false);

    store.close('ext');
    host.unmount();
    expect(vi.getTimerCount()).toBe(0);
    vi.useRealTimers();
  });

  it('says so when an application does not answer', async () => {
    const apps = useAppsStore();
    const store = useShellStore();
    apps.installed = [{ id: 'ext', name: 'Esplora file', info: '', system: false }];
    store.open('ext');
    probe.mockResolvedValueOnce(new Response('{"error":"not answering"}', { status: 503 }));
    const host = mount(AppHost);
    await flushPromises();

    await host.get('[data-testid="app_frame_ext"]').trigger('load');
    await flushPromises();

    expect(probe).toHaveBeenCalledExactlyOnceWith(
      expect.stringMatching(/^\/apps\/ext\/\?launch=/),
      expect.objectContaining({ cache: 'no-store' }),
    );
    const notice = host.get('[data-testid="app_unreachable_ext"]');
    expect(notice.text()).toContain('Gestore applicazioni');

    await notice.get('button').trigger('click');
    expect(host.find('[data-testid="app_unreachable_ext"]').exists()).toBe(false);
    await host.get('[data-testid="app_frame_ext"]').trigger('load');
    await flushPromises();
    expect(probe).toHaveBeenCalledTimes(2);
    expect(host.find('[data-testid="app_unreachable_ext"]').exists()).toBe(false);
    host.unmount();
  });

  it('shows the tab bar only while app tabs are open', async () => {
    const apps = useAppsStore();
    const store = useShellStore();
    apps.installed = [{ id: 'a', name: 'A', info: '', system: false }];
    const tabs = mount(AppTabs);
    expect(tabs.find('nav').exists()).toBe(false);
    store.open('a');
    await flushPromises();
    expect(tabs.find('nav').exists()).toBe(true);
    expect(tabs.findAll('button').some((button) => button.text() === 'Desktop')).toBe(false);
    store.showDesktop();
    await flushPromises();
    expect(tabs.get('[data-testid="app_tab_a"]').attributes('aria-pressed')).toBe('false');
    store.close('a');
    await flushPromises();
    expect(tabs.find('nav').exists()).toBe(false);
    tabs.unmount();
  });

  it('stops an external application when its tab close button is clicked', async () => {
    const apps = useAppsStore();
    const store = useShellStore();
    apps.installed = [{ id: 'ext', name: 'Explorer', info: '', system: false }];
    store.open('ext');
    const tabs = mount(AppTabs);
    await flushPromises();

    await tabs.get('[data-testid="app_tab_close_ext"]').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith(
      '/apps/ext/close',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(tabs.find('[data-testid="app_tab_ext"]').exists()).toBe(false);
    tabs.unmount();
  });

  it('indicates tabs hidden to the right and updates after scrolling and resizing', async () => {
    const apps = useAppsStore();
    apps.installed = [{ id: 'a', name: 'A', info: '', system: false }];
    useShellStore().open('a');
    const tabs = mount(AppTabs);
    await flushPromises();
    const viewport = tabs.get('.app_tabs_scroll');
    Object.defineProperties(viewport.element, {
      scrollWidth: { value: 700, configurable: true },
      clientWidth: { value: 300, configurable: true },
      scrollLeft: { value: 0, writable: true },
    });
    onResize([], {} as ResizeObserver);
    await flushPromises();
    expect(tabs.find('[data-testid="app_tabs_overflow"]').exists()).toBe(true);
    viewport.element.scrollLeft = 400;
    await viewport.trigger('scroll');
    expect(tabs.find('[data-testid="app_tabs_overflow"]').exists()).toBe(false);
    viewport.element.scrollLeft = 0;
    Object.defineProperty(viewport.element, 'clientWidth', { value: 800 });
    onResize([], {} as ResizeObserver);
    await flushPromises();
    expect(tabs.find('[data-testid="app_tabs_overflow"]').exists()).toBe(false);
    tabs.unmount();
    expect(resize.disconnect).toHaveBeenCalled();
  });
});

describe('file picker', () => {
  it('leaves the cross to the picker, and cancels it on Escape or a click beside it', async () => {
    const bridge = {
      picker: ref({ provider: 'files', requester: 'ext', url: '/apps/files/?nasPicker=t' }),
      cancelPicker: vi.fn(),
      attachPicker: vi.fn(),
      announcePicker: vi.fn(),
      attach: vi.fn(),
      ready: vi.fn(),
      announce: vi.fn(),
    } as unknown as ReturnType<typeof useAppBridge>;
    const host = mount(AppHost, { global: { provide: { [APP_BRIDGE as symbol]: bridge } } });
    await flushPromises();

    const overlay = host.get('[data-testid="app_file_picker"]');
    expect(overlay.find('button').exists()).toBe(false);
    await overlay.get('.file_picker_window').trigger('click');
    expect(bridge.cancelPicker).not.toHaveBeenCalled();
    await overlay.trigger('click');
    expect(bridge.cancelPicker).toHaveBeenCalledTimes(1);
    await overlay.trigger('keydown', { key: 'Escape' });
    expect(bridge.cancelPicker).toHaveBeenCalledTimes(2);
    host.unmount();
  });
});
