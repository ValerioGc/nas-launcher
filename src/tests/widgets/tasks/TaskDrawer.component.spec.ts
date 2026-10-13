import { createPinia, setActivePinia } from 'pinia';
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, afterEach, expect, it, vi } from 'vitest';
import TaskDrawer from '@/widgets/tasks/TaskDrawer.vue';
import { request } from '@/api/client';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => setActivePinia(createPinia()));

afterEach(() => {
  document.body.innerHTML = '';
});

it('loads only when opened and closes from its own header', async () => {
  vi.mocked(request).mockReset().mockResolvedValue([]);

  const wrapper = mount(TaskDrawer, { props: { modelValue: false }, attachTo: document.body });
  expect(request).not.toHaveBeenCalled();
  await wrapper.setProps({ modelValue: true });
  await flushPromises();
  expect(request).toHaveBeenCalledWith('/tasks');
  expect(wrapper.get('aside').attributes('id')).toBe('task_panel');
  await wrapper.get('[data-testid="widget_close"]').trigger('click');
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);

  wrapper.unmount();
});
