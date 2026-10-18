import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useNotificationsStore } from '@/apps/nas-notification-manager/store';
import { request } from '@/api/client';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(request).mockReset();
});

describe('notification summary', () => {
  it('shares pending refreshes and counts the unread items', async () => {
    vi.mocked(request).mockResolvedValue([
      { id: '1', read: false },
      { id: '2', read: false },
    ]);

    const store = useNotificationsStore();
    await Promise.all([store.refresh(), store.refresh()]);
    expect(request).toHaveBeenCalledExactlyOnceWith('/notifications?read=false&limit=500');
    expect(store.unread).toBe(2);
  });

  it('preserves the last successful data and reports failures', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce([{ id: '1', read: false }])
      .mockRejectedValueOnce(new Error());

    const store = useNotificationsStore();
    await store.refresh();
    await store.refresh();
    expect(store.items).toHaveLength(1);
    expect(store.error).not.toBe('');
  });
});

describe('toasts', () => {
  const entry = (id: string, read = false) => ({
    id,
    title: `Title ${id}`,
    message: 'Message',
    tone: 'info' as const,
    at: 0,
    read,
    source: 'system',
  });

  it('announces only the notifications that arrive after the first refresh', async () => {
    const store = useNotificationsStore();
    vi.mocked(request).mockResolvedValueOnce([entry('1')]);
    await store.refresh();
    expect(store.toasts).toEqual([]);

    vi.mocked(request).mockResolvedValueOnce([entry('2'), entry('1')]);
    await store.refresh();
    expect(store.toasts.map((toast) => toast.id)).toEqual(['2']);
    expect(store.toasts[0]).toMatchObject({
      title: 'Title 2',
      message: 'Message',
      tone: 'info',
      duration: 6000,
    });
  });

  it('opens the notification application from a toast', async () => {
    const { useShellStore } = await import('@/stores/shellStore');
    const open = vi.spyOn(useShellStore(), 'open').mockImplementation(() => undefined);
    const store = useNotificationsStore();
    vi.mocked(request)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([entry('7')]);
    await store.refresh();
    await store.refresh();

    store.toasts[0]?.action?.run();
    expect(open).toHaveBeenCalledWith('nas-notification-manager');
  });

  it('shows at the first refresh, until closed, only what was written for the sign-in', async () => {
    const store = useNotificationsStore();
    vi.mocked(request)
      .mockResolvedValueOnce([{ ...entry('3'), key: 'discoveryOff' }, entry('2')])
      .mockResolvedValue(undefined);
    await store.refresh();

    expect(store.toasts.map((toast) => toast.id)).toEqual(['3']);
    expect(store.toasts[0]?.duration).toBeUndefined();
    expect(store.toasts[0]?.notification?.key).toBe('discoveryOff');

    expect(request).toHaveBeenCalledWith('/notifications/3', {
      method: 'PATCH',
      body: JSON.stringify({ read: true }),
    });
  });

  it('keeps the toast of the sign-in even when it cannot be marked as read', async () => {
    const store = useNotificationsStore();
    vi.mocked(request)
      .mockResolvedValueOnce([{ ...entry('3'), key: 'discoveryOff' }])
      .mockRejectedValue(new Error('offline'));
    await store.refresh();
    await Promise.resolve();

    expect(store.toasts.map((toast) => toast.id)).toEqual(['3']);
  });

  it('replaces a toast with the same id and keeps only the latest four', () => {
    const store = useNotificationsStore();
    for (const id of ['a', 'b', 'c', 'd', 'e']) {
      store.toast({ id, tone: 'info', title: id });
    }
    store.toast({ id: 'c', tone: 'error', title: 'again' });

    expect(store.toasts.map((toast) => toast.id)).toEqual(['b', 'd', 'e', 'c']);
    expect(store.toasts.at(-1)?.tone).toBe('error');

    store.dismiss('d');
    expect(store.toasts.map((toast) => toast.id)).toEqual(['b', 'e', 'c']);
  });
});

describe('toasts after an action of the page', () => {
  it('shows nothing for what a refresh after an action finds, and remembers it', async () => {
    const entry = { id: '9', title: 'Saved', message: '', tone: 'success', at: 0, read: false };
    const store = useNotificationsStore();
    vi.mocked(request)
      .mockResolvedValueOnce([])
      .mockResolvedValue([{ ...entry, source: 'x' }]);
    await store.refresh();
    await store.refresh(false);
    await store.refresh();

    expect(store.toasts).toEqual([]);
    expect(store.unread).toBe(1);
  });
});
