import { mount, flushPromises, enableAutoUnmount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SystemActionDropdown from '@/components/header/SystemActionDropdown.vue';
import { request } from '@/api/client';
import { logout } from '@/api/gateway';
import { setLocale } from '@/i18n';
import type { PowerBusy } from '@/types/types';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
vi.mock('@/api/gateway', () => ({ logout: vi.fn() }));

enableAutoUnmount(afterEach);

const BUSY = '/system/power/busy';
let work: PowerBusy | Error;

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.useFakeTimers();
  work = { tasks: [], jobs: [] };
  vi.mocked(request)
    .mockReset()
    .mockImplementation(async (path: string) => {
      if (path !== BUSY) {
        return { accepted: true, simulated: true };
      }
      if (work instanceof Error) {
        throw work;
      }
      return work;
    });
  vi.mocked(logout).mockReset().mockResolvedValue();
});

afterEach(() => {
  vi.useRealTimers();
  document.body.innerHTML = '';
});

function button(id: string) {
  return document.querySelector<HTMLButtonElement>(`[data-testid="${id}"]`)!;
}

function sent() {
  return vi.mocked(request).mock.calls.filter(([path]) => path !== BUSY);
}

async function choose(command = 'shutdown') {
  const wrapper = mount(SystemActionDropdown, { attachTo: document.body });
  await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
  await wrapper.get(`[data-testid="system_action_${command}"]`).trigger('click');
  await flushPromises();
  return wrapper;
}

describe('system action confirmation', () => {
  it('counts down from 60 seconds and executes exactly once at the deadline', async () => {
    await choose();
    expect(document.querySelector('[role="timer"]')?.textContent).toContain('60');
    expect(sent()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(59000);
    expect(document.querySelector('[role="timer"]')?.textContent).toContain('1 secondi');
    expect(sent()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1000);
    expect(sent()).toEqual([['/system/actions/shutdown', { method: 'POST' }]]);
    expect(document.querySelector('dialog')).toBeNull();
    await vi.advanceTimersByTimeAsync(60000);
    expect(sent()).toHaveLength(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it.each(['restart', 'shutdown'])(
    'confirms %s immediately and clears the countdown',
    async (command) => {
      await choose(command);
      button('system_action_confirm').click();
      button('system_action_confirm').click();
      await flushPromises();
      expect(sent()).toEqual([[`/system/actions/${command}`, { method: 'POST' }]]);
      await vi.advanceTimersByTimeAsync(60000);
      expect(sent()).toHaveLength(1);
    },
  );

  it('closes the windows of the session and asks the gateway to close the session', async () => {
    sessionStorage.setItem('nas-shell-open-apps-v1', JSON.stringify({ openedIds: ['a'] }));
    await choose('logout');
    button('system_action_confirm').click();
    await flushPromises();
    expect(logout).toHaveBeenCalledTimes(1);
    expect(sent()).toEqual([['/apps/close', expect.objectContaining({ method: 'POST' })]]);
    const saved = JSON.parse(sessionStorage.getItem('nas-shell-open-apps-v1') ?? '{}');
    expect(saved.openedIds ?? []).toEqual([]);
  });

  it('closes the session after shutting down the machine', async () => {
    await choose('shutdown');
    button('system_action_confirm').click();
    await flushPromises();
    expect(sent()).toEqual([['/system/actions/shutdown', { method: 'POST' }]]);
    expect(logout).toHaveBeenCalledTimes(1);
  });

  it('blocks the interface until a real restart goes down and comes back', async () => {
    const reload = vi.fn();
    vi.stubGlobal('location', { reload });
    await choose('restart');
    vi.mocked(request).mockResolvedValueOnce({ accepted: true, simulated: false });

    button('system_action_confirm').click();
    await flushPromises();

    expect(document.querySelector('[data-testid="system_restart_overlay"]')?.textContent).toContain(
      'Riavvio del NAS in corso',
    );
    expect(button('common_modal_close')).toBeNull();
    expect(logout).not.toHaveBeenCalled();

    vi.mocked(request).mockResolvedValueOnce({ online: true });
    await vi.advanceTimersByTimeAsync(2000);
    expect(reload).not.toHaveBeenCalled();

    vi.mocked(request).mockRejectedValueOnce(new Error('offline'));
    await vi.advanceTimersByTimeAsync(2000);
    vi.mocked(request).mockResolvedValueOnce({ online: true });
    await vi.advanceTimersByTimeAsync(2000);

    expect(request).toHaveBeenCalledWith('/status');
    expect(reload).toHaveBeenCalledTimes(1);
    vi.unstubAllGlobals();
  });

  it('does not blame the machine when only the session fails to close', async () => {
    vi.mocked(logout).mockRejectedValueOnce(new Error('gateway offline'));
    await choose('shutdown');
    button('system_action_confirm').click();
    await flushPromises();
    expect(sent()).toHaveLength(1);
    expect(document.querySelector('[role="alert"]')).toBeNull();
    expect(document.querySelector('dialog')).toBeNull();
  });

  it('says nothing at all once the action has been sent', async () => {
    const wrapper = await choose('restart');
    button('system_action_confirm').click();
    await flushPromises();
    await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
    expect(wrapper.findAll('output')).toHaveLength(0);
    expect(wrapper.text()).not.toContain('simulat');
  });

  it('cancels and starts a fresh 60 second countdown when reopened', async () => {
    const wrapper = await choose();
    await vi.advanceTimersByTimeAsync(30000);
    button('system_action_cancel').click();
    await flushPromises();
    expect(document.activeElement).toBe(
      wrapper.get('[data-testid="common_dropdown_trigger"]').element,
    );
    await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
    await wrapper.get('[data-testid="system_action_restart"]').trigger('click');
    await vi.advanceTimersByTimeAsync(59000);
    expect(sent()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1000);
    expect(sent()).toEqual([['/system/actions/restart', { method: 'POST' }]]);
  });

  it.each(['escape', 'close', 'backdrop'])(
    'cancels on %s without executing later',
    async (method) => {
      await choose();
      if (method === 'escape') {
        document
          .querySelector('dialog')!
          .dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      } else {
        button(method === 'close' ? 'common_modal_close' : 'common_modal_backdrop').click();
      }
      await flushPromises();
      expect(document.querySelector('dialog')).toBeNull();
      await vi.advanceTimersByTimeAsync(60000);
      expect(sent()).toHaveLength(0);
      expect(vi.getTimerCount()).toBe(0);
    },
  );

  it('clears the pending action when unmounted', async () => {
    const wrapper = await choose();
    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(60000);
    expect(sent()).toHaveLength(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('shows a failed request without automatically retrying and allows manual retry', async () => {
    await choose();
    vi.mocked(request).mockRejectedValueOnce(new Error('offline'));
    await vi.advanceTimersByTimeAsync(60000);
    expect(document.querySelector('[role="alert"]')?.textContent).toContain(
      'Operazione non riuscita',
    );
    expect(document.querySelector('[role="timer"]')).toBeNull();
    await vi.advanceTimersByTimeAsync(60000);
    expect(sent()).toHaveLength(1);
    button('system_action_confirm').click();
    await flushPromises();
    expect(sent()).toHaveLength(2);
    expect(document.querySelector('dialog')).toBeNull();
  });
});

describe('work under way', () => {
  it('names the operations a restart interrupts and waits for an explicit confirmation', async () => {
    work = { tasks: ['Backup', 'Scansione antivirus'], jobs: [] };
    await choose('restart');
    const tasks = document.querySelector('[data-testid="system_action_tasks"]');
    expect(tasks?.textContent).toContain('Backup');
    expect(tasks?.textContent).toContain('Scansione antivirus');
    expect(document.querySelector('dialog')?.textContent).toContain('verranno interrotte');
    expect(document.querySelector('[role="timer"]')).toBeNull();
    await vi.advanceTimersByTimeAsync(120000);
    expect(sent()).toHaveLength(0);
    button('system_action_confirm').click();
    await flushPromises();
    expect(sent()).toEqual([['/system/actions/restart', { method: 'POST' }]]);
  });

  it('holds the shutdown back while a job of root runs', async () => {
    work = { tasks: ['Backup'], jobs: ['update', 'eject'] };
    await choose('shutdown');
    const jobs = document.querySelector('[data-testid="system_action_jobs"]');
    expect(jobs?.textContent).toContain('Aggiornamento del NAS');
    expect(jobs?.textContent).toContain('Espulsione di un disco');
    expect(document.querySelector('[data-testid="system_action_tasks"]')).toBeNull();
    expect(button('system_action_confirm').disabled).toBe(true);
    await vi.advanceTimersByTimeAsync(120000);
    expect(sent()).toHaveLength(0);
  });

  it('keeps the usual countdown when the work under way cannot be read', async () => {
    work = new Error('offline');
    await choose();
    expect(document.querySelector('[role="timer"]')?.textContent).toContain('60');
    await vi.advanceTimersByTimeAsync(60000);
    expect(sent()).toEqual([['/system/actions/shutdown', { method: 'POST' }]]);
  });

  it('does not ask before signing out', async () => {
    await choose('logout');
    expect(request).not.toHaveBeenCalled();
  });

  it('starts no countdown when cancelled before the answer arrives', async () => {
    let answer: (found: PowerBusy) => void = () => {};
    vi.mocked(request).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    );
    const wrapper = mount(SystemActionDropdown, { attachTo: document.body });
    await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
    await wrapper.get('[data-testid="system_action_shutdown"]').trigger('click');
    button('system_action_cancel').click();
    await flushPromises();
    answer({ tasks: [], jobs: [] });
    await flushPromises();
    await vi.advanceTimersByTimeAsync(60000);
    expect(sent()).toHaveLength(0);
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe('dropdown icon actions', () => {
  it('shows both icons and text for system actions', async () => {
    const wrapper = mount(SystemActionDropdown);
    await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
    for (const button of wrapper.findAll('[role="menuitem"]')) {
      expect(button.text()).toBe(button.attributes('aria-label'));
      expect(button.attributes('title')).toBe(button.attributes('aria-label'));
      expect(button.find('[data-testid="common_icon"]').exists()).toBe(true);
    }
  });
});
