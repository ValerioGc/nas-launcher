import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import FactoryResetModal from '@/apps/nas-system-settings/components/FactoryResetModal.vue';
import { request } from '@/api/client';
import { confirmPassword, LoginThrottled, PasswordRefused } from '@/api/gateway';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
vi.mock('@/api/gateway', async (original) => ({
  ...(await original<typeof import('@/api/gateway')>()),
  confirmPassword: vi.fn(),
}));

const text = () => document.body.textContent ?? '';
const byTestId = (id: string) => document.querySelector<HTMLElement>(`[data-testid="${id}"]`);

function mountModal() {
  return mount(FactoryResetModal, { attachTo: document.body, props: { modelValue: true } });
}

async function confirmWith(password: string) {
  const input = byTestId('factory_reset_password') as HTMLInputElement;
  input.value = password;
  input.dispatchEvent(new Event('input'));
  await flushPromises();
  byTestId('factory_reset_confirm')?.click();
  await flushPromises();
}

beforeEach(() => {
  setLocale('it');
  vi.useFakeTimers();
  vi.mocked(request).mockReset();
  vi.mocked(confirmPassword).mockReset().mockResolvedValue('c0de');
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

describe('FactoryResetModal', () => {
  it('lists what goes and what stays, and needs the password', async () => {
    const wrapper = mountModal();
    await flushPromises();

    expect([...document.querySelectorAll('li')].map((item) => item.textContent?.trim())).toEqual([
      'Account, sessioni e regole di accesso',
      'Applicazioni installate, con i relativi dati e impostazioni',
      'Utente del terminale e relativa cartella home',
      'Impostazioni del launcher, catalogo delle applicazioni e token delle release',
    ]);
    expect(text()).toContain('Dati eliminati definitivamente');
    expect(text()).toContain('I dischi dati e i relativi file restano invariati');
    expect(byTestId('factory_reset_confirm')?.hasAttribute('disabled')).toBe(true);
    expect(request).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('sends the password, then waits for the wizard and goes to it', async () => {
    const assign = vi.fn();
    vi.stubGlobal('location', { assign });
    const fetch = vi.fn().mockRejectedValue(new Error('down'));
    vi.stubGlobal('fetch', fetch);
    vi.mocked(request).mockResolvedValue({ available: true, reason: null });
    const wrapper = mountModal();
    await flushPromises();

    await confirmWith('long enough');
    expect(confirmPassword).toHaveBeenCalledWith('long enough');
    expect(request).toHaveBeenCalledWith('/system/reset', {
      method: 'POST',
      body: '{}',
      headers: { 'X-NAS-Confirmation': 'c0de' },
    });
    expect(byTestId('factory_reset_running')?.textContent).toContain('Ripristino in corso');
    expect(byTestId('common_modal_close')).toBeNull();

    await vi.advanceTimersByTimeAsync(2000);
    expect(assign).not.toHaveBeenCalled();

    fetch.mockResolvedValue({ ok: true });
    await vi.advanceTimersByTimeAsync(2000);
    await flushPromises();
    expect(fetch).toHaveBeenCalledWith('/api/wizard', expect.anything());
    expect(assign).toHaveBeenCalledWith('/');
    wrapper.unmount();
  });

  it.each([
    [new PasswordRefused('wrong password'), 'La password non è corretta.'],
    [new LoginThrottled('too many'), 'Troppi tentativi non riusciti'],
  ])('stays open on a refused password and says why: %s', async (refusal, reason) => {
    vi.mocked(confirmPassword).mockRejectedValue(refusal);
    const wrapper = mountModal();
    await flushPromises();

    await confirmWith('not the one');

    expect(byTestId('factory_reset_failed')?.textContent).toContain(reason);
    expect(request).not.toHaveBeenCalled();
    expect((byTestId('factory_reset_password') as HTMLInputElement).value).toBe('');
    expect(byTestId('common_modal_close')).not.toBeNull();
    wrapper.unmount();
  });

  it('shows why the launcher refused the reset', async () => {
    vi.mocked(request).mockRejectedValue(new Error('the reset did not start'));
    const wrapper = mountModal();
    await flushPromises();

    await confirmWith('long enough');

    expect(byTestId('factory_reset_failed')?.textContent).toContain('the reset did not start');
    wrapper.unmount();
  });
});
