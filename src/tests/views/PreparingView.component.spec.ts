import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import PreparingView from '@/views/PreparingView.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

const push = vi.fn();

vi.mock('@/api/client', () => ({ request: vi.fn() }));
vi.mock('vue-router', () => ({ useRouter: () => ({ push }) }));

const files = { id: 'app:files', kind: 'app', name: 'Explorer', state: 'done', detail: '' };
const photos = { id: 'app:photos', kind: 'app', name: 'Photos', state: 'running', detail: '' };

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal('matchMedia', () => ({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
  setActivePinia(createPinia());
  setLocale('it');
  push.mockReset();
  vi.mocked(request).mockReset();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('preparation', () => {
  it('lists the steps while they run and goes on to the sign-in once all succeeded', async () => {
    vi.mocked(request).mockResolvedValueOnce({
      completed: true,
      ready: false,
      steps: [files, photos],
    });
    const wrapper = mount(PreparingView);
    await flushPromises();

    expect(wrapper.get('[data-testid="preparing_spinner"]').text()).toBe('Configurazione in corso');
    expect(wrapper.get('[data-testid="preparing_step_app:files"]').text()).toContain('Explorer');
    expect(wrapper.get('[data-testid="preparing_step_app:photos"]').text()).toContain('Photos');
    expect(push).not.toHaveBeenCalled();

    vi.mocked(request).mockResolvedValueOnce({
      completed: true,
      ready: true,
      steps: [files, { ...photos, state: 'done' }],
    });
    await vi.advanceTimersByTimeAsync(2000);

    expect(push).toHaveBeenCalledWith({ name: 'login' });
    wrapper.unmount();
  });

  it('says the wizard has not configured the NAS yet, and keeps waiting for it', async () => {
    vi.mocked(request).mockResolvedValueOnce({ completed: false, ready: false, steps: [] });
    const wrapper = mount(PreparingView);
    await flushPromises();

    expect(wrapper.get('h1').text()).toBe('NAS non configurato');
    expect(wrapper.text()).toContain('configurazione con il wizard');
    expect(wrapper.find('[data-testid="preparing_spinner"]').exists()).toBe(false);

    vi.mocked(request).mockResolvedValueOnce({ completed: true, ready: true, steps: [files] });
    await vi.advanceTimersByTimeAsync(2000);

    expect(push).toHaveBeenCalledWith({ name: 'login' });
    wrapper.unmount();
  });

  it('says the launcher is restarting while it does not answer', async () => {
    vi.mocked(request).mockRejectedValueOnce(new Error('the launcher is not answering'));
    const wrapper = mount(PreparingView);
    await flushPromises();

    expect(wrapper.get('[data-testid="preparing_spinner"]').text()).toContain('Riavvio in corso');
    wrapper.unmount();
  });

  it('shows what failed and leaves the sign-in to a button', async () => {
    vi.mocked(request).mockResolvedValueOnce({
      completed: true,
      ready: true,
      steps: [files, { ...photos, state: 'failed', detail: 'the release has no asset' }],
    });
    const wrapper = mount(PreparingView);
    await flushPromises();

    expect(push).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="preparing_spinner"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="preparing_step_app:photos"]').text()).toContain(
      'the release has no asset',
    );
    await wrapper.get('[data-testid="preparing_continue"]').trigger('click');
    expect(push).toHaveBeenCalledWith({ name: 'login' });
    wrapper.unmount();
  });
});
