import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { request } from '@/api/client';
import { useSetupStore } from '@/stores/setupStore';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const step = { id: 'app:photos', kind: 'app', name: 'Photos', state: 'running', detail: '' };

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(request).mockReset();
});

describe('setup store', () => {
  it('says which page the first configuration needs', async () => {
    const setup = useSetupStore();

    vi.mocked(request).mockResolvedValueOnce({ completed: false, ready: false, steps: [] });
    expect(await setup.check()).toBe('unconfigured');
    vi.mocked(request).mockResolvedValueOnce({ completed: true, ready: false, steps: [step] });
    expect(await setup.check()).toBe('preparing');
    vi.mocked(request).mockResolvedValueOnce({ completed: true, ready: true, steps: [] });
    expect(await setup.check()).toBe('ready');
  });

  it('stops asking once everything is done', async () => {
    vi.mocked(request).mockResolvedValue({ completed: true, ready: true, steps: [] });
    const setup = useSetupStore();

    await setup.check();
    await setup.check();

    expect(request).toHaveBeenCalledOnce();
  });

  it('lets the sign-in take over when the launcher does not answer', async () => {
    vi.mocked(request).mockRejectedValue(new Error('down'));

    expect(await useSetupStore().check()).toBe('ready');
    expect(useSetupStore().state).toBeNull();
  });
});
