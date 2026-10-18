import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LoginPage from '@/views/LoginPage.vue';
import { login as gatewayLogin, LoginRefused } from '@/api/gateway';
import { request } from '@/api/client';
import { recoveryOffered, requestPasswordReset, resetPassword } from '@/api/account';
import { t } from '@/i18n';

const push = vi.fn();
vi.mock('vue-router', () => ({ useRouter: () => ({ push }) }));
vi.mock('@/api/client', () => ({ request: vi.fn() }));
vi.mock('@/api/account', () => ({
  recoveryOffered: vi.fn().mockResolvedValue(false),
  requestPasswordReset: vi.fn(),
  resetPassword: vi.fn(),
  CodeRefused: class extends Error {},
}));
vi.mock('@/api/gateway', () => ({
  login: vi.fn(),
  hasSession: vi.fn().mockResolvedValue(true),
  LoginRefused: class extends Error {},
  LoginThrottled: class extends Error {},
}));

function mountLogin() {
  return mount(LoginPage, { attachTo: document.body });
}

async function fillLogin(wrapper: ReturnType<typeof mountLogin>) {
  await wrapper.get('[name="username"]').setValue('  admin  ');
  await wrapper.get('[name="password"]').setValue('long enough');
}

describe('LoginPage', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.mocked(request).mockReset().mockResolvedValue(undefined);
    vi.mocked(gatewayLogin).mockReset().mockResolvedValue({ username: 'mock-user', device: null });
    push.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('validates fields on blur and focuses the first invalid field', async () => {
    const wrapper = mountLogin();
    await wrapper.get('[name="username"]').trigger('blur');
    expect(wrapper.text()).toContain('È necessario inserire il nome utente.');
    await wrapper.get('form').trigger('submit');
    expect(document.activeElement).toBe(wrapper.get('[name="username"]').element);
    expect(gatewayLogin).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('toggles password visibility without submitting', async () => {
    const wrapper = mountLogin();
    await fillLogin(wrapper);
    await wrapper.get('[aria-controls="login_password"]').trigger('click');
    expect(wrapper.get('[name="password"]').attributes('type')).toBe('text');
    expect(gatewayLogin).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('submits once, clears the password and navigates after gateway success', async () => {
    let resolve!: (value: { username: string; device: string | null }) => void;
    vi.mocked(gatewayLogin).mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const wrapper = mountLogin();
    await fillLogin(wrapper);
    await wrapper.get('form').trigger('submit');
    await wrapper.get('form').trigger('submit');
    expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined();
    resolve({ username: 'mock-user', device: null });
    await flushPromises();
    expect(gatewayLogin).toHaveBeenCalledTimes(1);
    expect(wrapper.get<HTMLInputElement>('[name="password"]').element.value).toBe('');
    expect(push).toHaveBeenCalledWith({ name: 'main' });
    wrapper.unmount();
  });

  it('shows a gateway failure and stays on login', async () => {
    vi.mocked(gatewayLogin).mockRejectedValue(new Error('offline'));
    const wrapper = mountLogin();
    await fillLogin(wrapper);
    await wrapper.get('form').trigger('submit');
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain('Accesso non riuscito');
    expect(push).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('says the credentials are wrong when the gateway refuses them', async () => {
    vi.mocked(gatewayLogin).mockRejectedValue(new LoginRefused('refused'));
    const wrapper = mountLogin();
    await fillLogin(wrapper);
    await wrapper.get('form').trigger('submit');
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toBe('Nome utente o password non corretti.');
    expect(push).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('offers the reset only when the account turned it on', async () => {
    vi.mocked(recoveryOffered).mockResolvedValueOnce(false);
    const wrapper = mountLogin();
    await flushPromises();
    expect(wrapper.find('[data-testid="login-forgot"]').exists()).toBe(false);
  });

  it('resets a forgotten password with the code of the mail and comes back to the sign-in', async () => {
    vi.mocked(recoveryOffered).mockResolvedValueOnce(true);
    const wrapper = mountLogin();
    await flushPromises();
    await wrapper.get('[name="username"]').setValue('admin');
    await wrapper.get('[data-testid="login-forgot"]').trigger('click');
    expect(wrapper.find('[data-testid="login-form"]').exists()).toBe(false);
    const recovery = wrapper.get('[data-testid="password-recovery"]');
    expect(recovery.get('h2').text()).toBe(t('passwordRecoveryTitle'));
    expect(
      (wrapper.get('[data-testid="recovery-username"]').element as HTMLInputElement).value,
    ).toBe('admin');

    await wrapper.get('[data-testid="recovery-send"]').trigger('submit');
    await flushPromises();
    expect(requestPasswordReset).toHaveBeenCalledWith('admin');
    expect(wrapper.text()).toContain(t('passwordRecoverySent'));

    await wrapper.get('[data-testid="recovery-code"]').setValue('123456');
    await wrapper.get('[data-testid="recovery-password"]').setValue('a new password');
    await wrapper.get('[data-testid="recovery-repeat"]').setValue('another');
    expect(wrapper.text()).toContain(t('passwordRecoveryMismatch'));
    expect(wrapper.get('[data-testid="recovery-reset"]').attributes('disabled')).toBeDefined();
    await wrapper.get('[data-testid="recovery-repeat"]').setValue('a new password');
    await wrapper.get('[data-testid="recovery-reset"]').trigger('submit');
    await flushPromises();
    expect(resetPassword).toHaveBeenCalledWith('admin', '123456', 'a new password');
    expect(wrapper.get('[data-testid="recovery-done"]').text()).toBe(t('passwordRecoveryDone'));

    await wrapper.get('[data-testid="recovery-back"]').trigger('click');
    expect(wrapper.find('[data-testid="login-form"]').exists()).toBe(true);
    expect((wrapper.get('[name="username"]').element as HTMLInputElement).value).toBe('admin');
  });
});
