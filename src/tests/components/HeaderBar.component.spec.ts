import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import HeaderBar from '@/components/HeaderBar.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

function open(props = {}) {
  return mount(HeaderBar, {
    props: { processesAvailable: true, ...props },
    global: { stubs: { teleport: true } },
  });
}

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset().mockResolvedValue([]);
});

describe('top bar', () => {
  it.each([
    ['header_menu_toggle', 'toggleMenu'],
    ['header_process_toggle', 'toggleProcesses'],
    ['header_task_toggle', 'toggleTasks'],
  ])('asks for %s through %s', async (testid, event) => {
    const wrapper = open();
    await wrapper.get(`[data-testid="${testid}"]`).trigger('click');

    expect(wrapper.emitted(event)).toHaveLength(1);
    wrapper.unmount();
  });

  it.each([
    ['header_menu_toggle', { menuOpen: true }],
    ['header_process_toggle', { processesOpen: true }],
    ['header_task_toggle', { tasksOpen: true }],
  ])('says whether the panel behind %s is open', (testid, props) => {
    const closed = open();
    expect(closed.get(`[data-testid="${testid}"]`).attributes('aria-expanded')).toBe('false');
    closed.unmount();

    const opened = open(props);
    expect(opened.get(`[data-testid="${testid}"]`).attributes('aria-expanded')).toBe('true');
    opened.unmount();
  });

  it('offers the processes only with the application that draws them', () => {
    const wrapper = open({ processesAvailable: false });

    expect(wrapper.find('[data-testid="header_process_toggle"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="header_task_toggle"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('offers the way back to the desktop only when an application is open', async () => {
    const closed = open();
    expect(closed.find('[data-testid="header_desktop"]').exists()).toBe(false);
    closed.unmount();

    const wrapper = open({ appOpen: true });
    await wrapper.get('[data-testid="header_desktop"]').trigger('click');
    expect(wrapper.emitted('showDesktop')).toHaveLength(1);
    wrapper.unmount();
  });

  it('passes on what was searched for', async () => {
    const wrapper = open();
    await wrapper.get('[data-testid="header_search_input"]').setValue('foto');
    await wrapper.get('form').trigger('submit');

    expect(wrapper.emitted('search')?.[0]).toEqual(['foto']);
    wrapper.unmount();
  });
});
