import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SystemUpdatesTab from '@/apps/nas-system-settings/components/tabs/SystemUpdatesTab.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { SystemUpdates } from '@/apps/nas-system-settings/types/update';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const bash = { id: 'bash', current: '5.2.37-1', version: '5.2.37-2', level: 'stable' as const };
const idle: SystemUpdates = { packages: [bash], running: null, manageable: true };

function answer(...states: SystemUpdates[]) {
  const queue = [...states];
  vi.mocked(request)
    .mockReset()
    .mockImplementation(async () => (queue.length > 1 ? queue.shift() : queue[0]) as never);
}

function mountTab() {
  return mount(SystemUpdatesTab, { global: { stubs: { LauncherUpdates: true } } });
}

beforeEach(() => {
  setLocale('it');
  vi.useFakeTimers();
});

afterEach(() => vi.useRealTimers());

describe('system updates', () => {
  it('lists each package with the installed and the new version', async () => {
    answer(idle);
    const wrapper = mountTab();
    await flushPromises();

    const row = wrapper.get('[data-testid="system_packages_row_bash"]');
    expect(row.text()).toContain('5.2.37-1');
    expect(row.text()).toContain('5.2.37-2');
    expect(wrapper.find('[data-testid="system_updates_readonly"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('installs only the packages that were chosen', async () => {
    answer(idle, { ...idle, running: 'upgrade' });
    const wrapper = mountTab();
    await flushPromises();
    expect(
      wrapper.get('[data-testid="system_updates_apply"]').attributes('disabled'),
    ).toBeDefined();

    await wrapper.get('[data-testid="system_packages_row_bash"] input').setValue(true);
    await wrapper.get('[data-testid="system_updates_apply"]').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/system/updates', {
      method: 'POST',
      body: '{"ids":["bash"]}',
    });
  });

  it('shows the job in progress on the list and reads it again until it is over', async () => {
    answer(
      { ...idle, running: 'refresh' },
      { ...idle, running: 'refresh' },
      { packages: [], running: null, manageable: true },
    );
    const wrapper = mountTab();
    await flushPromises();

    expect(wrapper.get('[data-testid="system_updates_running"]').text()).toContain(
      'Aggiornamento dell’elenco',
    );
    expect(wrapper.get('.system_updates_list').attributes('aria-busy')).toBe('true');
    expect(
      wrapper.get('[data-testid="system_packages_row_bash"] input').attributes('disabled'),
    ).toBeDefined();
    expect(wrapper.find('[data-testid="system_updates_refresh"]').exists()).toBe(false);
    expect(
      wrapper.get('[data-testid="system_updates_recheck"]').attributes('disabled'),
    ).toBeDefined();
    expect(
      wrapper.get('[data-testid="system_updates_apply"]').attributes('disabled'),
    ).toBeDefined();

    await vi.advanceTimersByTimeAsync(3000);
    await vi.advanceTimersByTimeAsync(3000);
    await flushPromises();

    expect(wrapper.find('[data-testid="system_updates_running"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="system_updates_none"]').text()).toContain('aggiornato');
    wrapper.unmount();
  });

  it('refreshes the package list on request', async () => {
    answer({ ...idle, packages: [] });
    const wrapper = mountTab();
    await flushPromises();

    await wrapper.get('[data-testid="system_updates_refresh"]').trigger('click');
    expect(request).toHaveBeenCalledWith('/system/updates/refresh', {
      method: 'POST',
      body: '{}',
    });
    wrapper.unmount();
  });

  it('checks again with pending packages without installing them', async () => {
    const newer = { ...bash, version: '5.2.37-3' };
    const refreshing = { ...idle, running: 'refresh' as const };
    answer(idle, refreshing, refreshing, { ...idle, packages: [newer] });
    const wrapper = mountTab();
    await flushPromises();

    const button = wrapper.get('[data-testid="system_updates_recheck"]');
    expect(button.attributes('aria-label')).toBe('Controlla aggiornamenti');
    expect(button.attributes('disabled')).toBeUndefined();
    await button.trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/system/updates/refresh', {
      method: 'POST',
      body: '{}',
    });
    expect(request).not.toHaveBeenCalledWith(
      '/system/updates',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(button.attributes('disabled')).toBeDefined();

    await vi.advanceTimersByTimeAsync(3000);
    await flushPromises();
    expect(wrapper.get('[data-testid="system_packages_row_bash"]').text()).toContain(newer.version);
    expect(button.attributes('disabled')).toBeUndefined();
    wrapper.unmount();
  });

  it('says why nothing can be installed where the mode is not real', async () => {
    answer({ ...idle, manageable: false, packages: [] });
    const wrapper = mountTab();
    await flushPromises();

    expect(wrapper.get('[data-testid="system_updates_readonly"]').text()).toContain(
      'non disponibile in questa installazione',
    );
    expect(
      wrapper.get('[data-testid="system_updates_refresh"]').attributes('disabled'),
    ).toBeDefined();
    wrapper.unmount();
  });

  it('offers to install only when apt has something to install', async () => {
    answer({ ...idle, packages: [] });
    const wrapper = mountTab();
    await flushPromises();

    expect(wrapper.find('[data-testid="system_updates_apply"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="system_updates_refresh"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="system_updates_recheck"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
