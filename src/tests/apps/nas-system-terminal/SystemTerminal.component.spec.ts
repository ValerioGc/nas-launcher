import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SystemTerminal from '@/apps/nas-system-terminal/SystemTerminal.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import { FakeEventSource, settings } from './fixtures';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.stubGlobal('EventSource', FakeEventSource);
  FakeEventSource.opened.clear();
  vi.mocked(request).mockReset().mockResolvedValue(settings);
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

describe('terminal', () => {
  it('opens a shell while open, runs what is typed, and closes it when closed', async () => {
    const wrapper = mount(SystemTerminal, { attachTo: document.body });
    await flushPromises();
    const shell = FakeEventSource.last;
    shell?.open();
    await flushPromises();

    expect(wrapper.get('[data-testid="terminal_connection"]').attributes('title')).toBe(
      'Collegato',
    );
    expect(
      wrapper.get('[data-testid="terminal_connection"] [role="img"]').attributes('aria-label'),
    ).toBe('Collegato');
    const clearButton = wrapper.get('[data-testid="terminal_clear"]');
    expect(clearButton.attributes('title')).toBe('Pulisci');
    expect(clearButton.attributes('aria-label')).toBe('Pulisci');
    expect(clearButton.text()).toBe('');
    expect(document.activeElement).toBe(wrapper.get('[data-testid="terminal_command"]').element);

    await wrapper.get('[data-testid="terminal_command"]').setValue('echo ciao');
    await wrapper.get('[data-testid="terminal_prompt"]').trigger('submit');
    shell?.print('ciao');
    shell?.done(0, '/');
    await flushPromises();

    expect(wrapper.get('[data-testid="terminal_output"]').text()).toContain('/ $ echo ciao');
    expect(wrapper.get('[data-testid="terminal_output"]').text()).toContain('ciao');

    wrapper.unmount();
    expect(shell?.closed).toBe(true);
  });

  it('draws the prompt and the output with the look of the settings', async () => {
    const wrapper = mount(SystemTerminal, { attachTo: document.body });
    await flushPromises();
    const style = wrapper.get('[data-testid="terminal_app"]').attributes('style');

    expect(style).toContain('--terminal-text: #d6e2ea');
    expect(style).toContain('--terminal-font-size: 14px');
    expect(style).toContain('color-mix(in srgb, #0e1922 80%, transparent)');
    expect(wrapper.get('section').attributes('style')).toContain(
      'color-mix(in srgb, var(--surface) 80%, transparent)',
    );
    wrapper.unmount();
  });

  it('clears the screen, and offers to reconnect a shell the launcher closed', async () => {
    const wrapper = mount(SystemTerminal, { attachTo: document.body });
    await flushPromises();
    const shell = FakeEventSource.last;
    shell?.open();
    shell?.print('old output');
    await flushPromises();

    await wrapper.get('[data-testid="terminal_clear"]').trigger('click');
    expect(wrapper.get('[data-testid="terminal_output"]').text()).not.toContain('old output');

    shell?.fail(true);
    await flushPromises();
    await wrapper.get('[data-testid="terminal_reconnect"]').trigger('click');
    expect(FakeEventSource.last).not.toBe(shell);
    wrapper.unmount();
  });

  it('opens the settings from the icon button', async () => {
    const wrapper = mount(SystemTerminal, { attachTo: document.body });
    await flushPromises();

    const button = wrapper.get('[data-testid="terminal_settings_open"]');
    expect(button.attributes('aria-label')).toBe('Impostazioni del terminale');
    await button.trigger('click');

    expect(document.querySelector('[data-testid="terminal_settings"]')).not.toBeNull();
    wrapper.unmount();
  });

  it('follows the system log in its own tab, only while the tab is open', async () => {
    const wrapper = mount(SystemTerminal, { attachTo: document.body });
    await flushPromises();
    FakeEventSource.last?.open();
    await flushPromises();
    expect(FakeEventSource.opened.has('/api/terminal/log')).toBe(false);

    await wrapper.get('[data-testid="terminal_view_log"]').trigger('click');
    const log = FakeEventSource.opened.get('/api/terminal/log');
    log?.connect();
    log?.print('2026-09-26T10:00:00 nas systemd[1]: Started nas-os.service.');
    await flushPromises();

    expect(wrapper.find('[data-testid="terminal_prompt"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="terminal_log"]').text()).toContain('Started nas-os.service.');
    expect(wrapper.get('[data-testid="terminal_connection"]').attributes('title')).toBe(
      'Collegato',
    );

    await wrapper.get('[data-testid="terminal_view_console"]').trigger('click');
    expect(log?.closed).toBe(true);
    expect(wrapper.find('[data-testid="terminal_prompt"]').exists()).toBe(true);
    wrapper.unmount();
  });
});
