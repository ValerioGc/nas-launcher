import { defineComponent, h, nextTick, provide, reactive } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import MonitorDrawer from '@/widgets/monitor/MonitorDrawer.vue';
import { APP_BRIDGE, MONITOR_WIDGET, type useAppBridge } from '@/composables/useAppBridge';
import { useShellStore } from '@/stores/shellStore';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

type Bridge = ReturnType<typeof useAppBridge>;

function drawer(open = true, withBridge = true) {
  const bridge = {
    attach: vi.fn(),
    announce: vi.fn(),
    ready: reactive(new Set<string>()),
  } as unknown as Bridge;
  const wrapper = mount(
    defineComponent({
      setup() {
        if (withBridge) {
          provide(APP_BRIDGE, bridge);
        }
        return () => h(MonitorDrawer, { modelValue: open });
      },
    }),
    { attachTo: document.body },
  );
  return { wrapper, bridge };
}

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset().mockResolvedValue(undefined);
  vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('stopped')));
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

describe('the drawer of the processes', () => {
  it('starts the system monitor and shows its widget in a frame the bridge follows', async () => {
    const { wrapper, bridge } = drawer();
    expect(wrapper.find('[data-testid="process_widget_loading"]').exists()).toBe(true);
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/apps/nas-system-monitor/start', expect.anything());
    const frame = wrapper.get('[data-testid="process_widget_frame"]');
    const address = new URL(frame.attributes('src')!, window.location.origin);
    expect(address.pathname).toBe('/apps/nas-system-monitor/');
    expect(address.searchParams.get('view')).toBe('widget');
    expect(address.searchParams.get('theme')).toBe(
      document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light',
    );
    expect(address.searchParams.get('lang')).toBe(
      document.documentElement.lang === 'en' ? 'en' : 'it',
    );
    expect(bridge.attach).toHaveBeenCalledWith(MONITOR_WIDGET, frame.element);

    await frame.trigger('load');
    expect(bridge.announce).toHaveBeenCalledWith(MONITOR_WIDGET);
    expect(wrapper.find('[data-testid="process_widget_loading"]').exists()).toBe(true);
    bridge.ready.add(MONITOR_WIDGET);
    await nextTick();
    expect(wrapper.find('[data-testid="process_widget_loading"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('keeps the loading indicator until the frame loads when used without a bridge', async () => {
    const { wrapper } = drawer(true, false);
    await flushPromises();
    expect(wrapper.find('[data-testid="process_widget_loading"]').exists()).toBe(true);
    await wrapper.get('[data-testid="process_widget_frame"]').trigger('load');
    expect(wrapper.find('[data-testid="process_widget_loading"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('says why the application did not start', async () => {
    vi.mocked(request).mockRejectedValue(new Error('the unit failed'));
    const { wrapper } = drawer();
    await flushPromises();

    expect(wrapper.get('[role="alert"]').text()).toContain('the unit failed');
    expect(wrapper.find('[data-testid="process_widget_frame"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="process_widget_loading"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('opens the whole application and closes itself', async () => {
    const { wrapper } = drawer();
    await flushPromises();
    const shell = useShellStore();
    const opened = vi.spyOn(shell, 'open').mockImplementation(() => undefined);

    await wrapper.get('[data-testid="process_widget_open_app"]').trigger('click');
    expect(opened).toHaveBeenCalledWith('nas-system-monitor');
    wrapper.unmount();
  });

  it('starts nothing while closed', async () => {
    const { wrapper } = drawer(false);
    await flushPromises();

    expect(request).not.toHaveBeenCalled();
    wrapper.unmount();
  });
});
