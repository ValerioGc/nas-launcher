import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWidgetsStore } from '@/stores/widgetsStore';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset();
});

describe('widget settings store', () => {
  it('falls back to ten seconds until the settings arrive', () => {
    expect(useWidgetsStore().pollingMs).toBe(10_000);
  });

  it('turns the saved seconds into the interval the widgets poll at', async () => {
    vi.mocked(request).mockResolvedValue({ pollingSeconds: 30 });
    const store = useWidgetsStore();
    await store.load();

    expect(store.pollingMs).toBe(30_000);
    expect(store.error).toBe('');
  });

  it('keeps the previous interval and reports a failed read', async () => {
    vi.mocked(request).mockRejectedValue(new Error('down'));
    const store = useWidgetsStore();
    await store.load();

    expect(store.settings).toBeNull();
    expect(store.pollingMs).toBe(10_000);
    expect(store.error).not.toBe('');
  });

  it('saves the new interval and says it worked', async () => {
    vi.mocked(request).mockResolvedValue({ pollingSeconds: 5 });
    const store = useWidgetsStore();

    await expect(store.save({ pollingSeconds: 5 })).resolves.toBe(true);
    expect(request).toHaveBeenCalledWith('/settings/widgets', {
      method: 'PUT',
      body: '{"pollingSeconds":5}',
    });
    expect(store.pollingMs).toBe(5000);
    expect(store.saving).toBe(false);
  });

  it('says a refused save did not work and stops being busy', async () => {
    vi.mocked(request).mockRejectedValue(new Error('refused'));
    const store = useWidgetsStore();

    await expect(store.save({ pollingSeconds: 5 })).resolves.toBe(false);
    expect(store.error).not.toBe('');
    expect(store.saving).toBe(false);
  });
});
