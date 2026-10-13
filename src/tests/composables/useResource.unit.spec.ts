import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ResourceProbe from '@tests/fixtures/ResourceProbe.vue';
import { request } from '@/api/client';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => vi.mocked(request).mockReset());

describe('resource lifecycle', () => {
  it('ignores late replies from earlier requests', async () => {
    let resolveOld: (value: string[]) => void = () => {};
    vi.mocked(request)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveOld = resolve;
          }),
      )
      .mockResolvedValueOnce(['new']);

    const wrapper = mount(ResourceProbe);
    await wrapper.get('button').trigger('click');
    await flushPromises();
    resolveOld(['old']);
    await flushPromises();
    expect(wrapper.get('p').text()).toBe('new');

    wrapper.unmount();
  });

  it('keeps existing data on failure and allows retry', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce(['saved'])
      .mockRejectedValueOnce(new Error())
      .mockResolvedValueOnce(['latest']);

    const wrapper = mount(ResourceProbe);
    await flushPromises();
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(wrapper.get('p').text()).toBe('saved');
    expect(wrapper.get('span').text()).toContain('non riuscita');
    await wrapper.get('button').trigger('click');
    await flushPromises();
    expect(wrapper.get('p').text()).toBe('latest');

    wrapper.unmount();
  });
});
