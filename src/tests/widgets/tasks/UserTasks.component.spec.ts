import { createPinia, setActivePinia } from 'pinia';
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import UserTasks from '@/widgets/tasks/UserTasks.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { UserTask } from '@/types/widgets/tasks';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

function task(over: Partial<UserTask> = {}): UserTask {
  return {
    id: 'copy',
    source: 'nas-backup-manager',
    name: 'Copia',
    detail: '',
    progress: 40,
    state: 'running',
    pausable: true,
    cancellable: true,
    createdAt: 0,
    updatedAt: 0,
    ...over,
  };
}

const names = (wrapper: VueWrapper) => wrapper.findAll('.task_row strong').map((row) => row.text());

function answer(tasks: UserTask[]) {
  vi.mocked(request)
    .mockReset()
    .mockResolvedValue(tasks as never);
}

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  answer([]);
});

describe('ongoing operations', () => {
  it('offers pause and stop on what is still going, and sends what was clicked', async () => {
    answer([task(), task({ id: 'scan', name: 'Scansione', state: 'paused', progress: 10 })]);
    const wrapper = mount(UserTasks);
    await flushPromises();

    const actions = wrapper.findAll('.task_row .app_toolbar button');
    expect(actions).toHaveLength(4);
    for (const button of actions) {
      expect(button.text()).toBe('');
      expect(button.attributes('title')).toBeTruthy();
      expect(button.attributes('aria-label')).toBeTruthy();
      expect(button.find('[data-testid="common_icon"]').exists()).toBe(true);
    }

    await actions[0]?.trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/tasks/copy/pause', { method: 'POST', body: undefined });

    await actions[2]?.trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/tasks/scan/resume', { method: 'POST', body: undefined });

    await actions[1]?.trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/tasks/copy/stop', { method: 'POST', body: undefined });
    wrapper.unmount();
  });

  it.each([
    [{ pausable: false, cancellable: true }, ['Termina Copia']],
    [{ pausable: true, cancellable: false }, ['Pausa Copia']],
    [{ pausable: false, cancellable: false }, []],
  ])('offers only what the owner agreed to be asked for (%j)', async (allowed, labels) => {
    answer([task(allowed)]);
    const wrapper = mount(UserTasks);
    await flushPromises();

    expect(
      wrapper.findAll('.task_row .app_toolbar button').map((b) => b.attributes('aria-label')),
    ).toEqual(labels);
    wrapper.unmount();
  });

  it('draws a bar with no position when the progress is unknown', async () => {
    answer([task({ progress: null })]);
    const wrapper = mount(UserTasks);
    await flushPromises();

    const bar = wrapper.get('[data-testid="progress_bar"]');
    expect(bar.attributes('value')).toBeUndefined();
    expect(bar.classes()).toContain('progress_bar_waiting');
    wrapper.unmount();
  });

  it.each([
    ['done', 'Conclusa'],
    ['failed', 'Non riuscita'],
    ['cancelled', 'Interrotta'],
  ] as const)(
    'shows a %s operation as history, with only a way to remove it',
    async (state, label) => {
      answer([task({ state, progress: state === 'failed' ? null : 100 })]);
      const wrapper = mount(UserTasks);
      await flushPromises();
      expect(wrapper.find('[data-testid="task_copy"]').exists()).toBe(false);

      await wrapper.get('[data-testid="task_view_finished"]').trigger('click');

      expect(wrapper.get('[data-testid="task_state_copy"]').text()).toBe(label);
      expect(wrapper.find('[data-testid="progress_bar"]').exists()).toBe(false);
      expect(wrapper.get('[data-testid="task_copy"]').classes()).toContain('task_over');

      const actions = wrapper.findAll('.task_row .app_toolbar button');
      expect(actions).toHaveLength(1);
      await actions[0]?.trigger('click');
      await flushPromises();
      expect(request).toHaveBeenCalledWith('/tasks/copy', { method: 'DELETE', body: undefined });
      wrapper.unmount();
    },
  );

  it('sweeps up everything that has finished, and only offers to when there is something', async () => {
    answer([task()]);
    const running = mount(UserTasks);
    await flushPromises();
    expect(running.find('[data-testid="task_clear_finished"]').exists()).toBe(false);
    running.unmount();

    answer([task(), task({ id: 'old', state: 'done' })]);
    const wrapper = mount(UserTasks);
    await flushPromises();
    expect(wrapper.get('[data-testid="task_count"]').text()).toContain('1 operazioni');
    expect(wrapper.find('[data-testid="task_clear_finished"]').exists()).toBe(false);

    await wrapper.get('[data-testid="task_view_finished"]').trigger('click');
    await wrapper.get('[data-testid="task_clear_finished"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/tasks/finished', { method: 'POST', body: undefined });
    wrapper.unmount();
  });

  it('opens on what is running, and keeps what has ended in a tab of its own', async () => {
    answer([task(), task({ id: 'old', name: 'Vecchia', state: 'done' })]);
    const wrapper = mount(UserTasks);
    await flushPromises();

    expect(wrapper.get('[data-testid="task_view_active"]').attributes('aria-pressed')).toBe('true');
    expect(names(wrapper)).toEqual(['Copia']);

    await wrapper.get('[data-testid="task_view_finished"]').trigger('click');
    expect(names(wrapper)).toEqual(['Vecchia']);
    wrapper.unmount();
  });

  it('lists both tabs from the most recent operation to the oldest', async () => {
    answer([
      task({ id: 'a', name: 'Prima', createdAt: 1 }),
      task({ id: 'b', name: 'Terza', createdAt: 3, state: 'paused' }),
      task({ id: 'c', name: 'Seconda', createdAt: 2 }),
      task({ id: 'd', name: 'Vecchia', createdAt: 4, state: 'failed' }),
      task({ id: 'e', name: 'Recente', createdAt: 6, state: 'done' }),
      task({ id: 'f', name: 'Media', createdAt: 5, state: 'cancelled' }),
    ]);
    const wrapper = mount(UserTasks);
    await flushPromises();

    expect(names(wrapper)).toEqual(['Terza', 'Seconda', 'Prima']);
    await wrapper.get('[data-testid="task_view_finished"]').trigger('click');
    expect(names(wrapper)).toEqual(['Recente', 'Media', 'Vecchia']);
    wrapper.unmount();
  });

  it('says when nothing has finished yet', async () => {
    answer([task()]);
    const wrapper = mount(UserTasks);
    await flushPromises();

    await wrapper.get('[data-testid="task_view_finished"]').trigger('click');
    expect(wrapper.text()).toContain('Nessuna operazione conclusa.');
    wrapper.unmount();
  });

  it('shows who is working and what it is doing', async () => {
    answer([task({ detail: '12 di 30 GB' })]);
    const wrapper = mount(UserTasks);
    await flushPromises();

    const row = wrapper.get('[data-testid="task_copy"]');
    expect(row.text()).toContain('12 di 30 GB');
    expect(row.text()).toContain('nas-backup-manager');
    wrapper.unmount();
  });

  it('asks to close itself from the header of the widget', async () => {
    const wrapper = mount(UserTasks);
    await flushPromises();

    expect(wrapper.text()).toContain('Nessuna operazione in corso.');
    await wrapper.get('[data-testid="widget_close"]').trigger('click');
    expect(wrapper.emitted('close')).toHaveLength(1);
    wrapper.unmount();
  });
});
