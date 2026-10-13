import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import ConfigTransferModal from '@/apps/nas-system-settings/components/ConfigTransferModal.vue';
import { request } from '@/api/client';
import { confirmPassword, PasswordRefused, saveSignInPolicy, signInPolicy } from '@/api/gateway';
import {
  isEncrypted,
  readConfig,
  saveConfigFile,
  writeConfig,
} from '@/apps/nas-system-settings/configFile';
import { setLocale } from '@/i18n';
import { useAppsStore } from '@/stores/appsStore';
import type { ConfigContent, LauncherConfig } from '@/apps/nas-system-settings/types/config';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
vi.mock('@/api/gateway', async (original) => ({
  ...(await original<typeof import('@/api/gateway')>()),
  confirmPassword: vi.fn(),
  signInPolicy: vi.fn(),
  saveSignInPolicy: vi.fn(),
}));
vi.mock('@/apps/nas-system-settings/configFile', async (original) => ({
  ...(await original<typeof import('@/apps/nas-system-settings/configFile')>()),
  saveConfigFile: vi.fn(() => 'nas-os-config-2026-10-02.json'),
}));

const config = {
  format: 'nas-os-config',
  version: 1,
  accessRules: [],
} as unknown as LauncherConfig;
const signIn = { enabled: true, attempts: 3, lockMinutes: 5 };

const byTestId = (id: string) => document.querySelector<HTMLElement>(`[data-testid="${id}"]`);

function mountModal(mode: 'export' | 'import') {
  return mount(ConfigTransferModal, { attachTo: document.body, props: { modelValue: true, mode } });
}

function type(id: string, value: string) {
  const field = byTestId(id) as HTMLInputElement;
  field.value = value;
  field.dispatchEvent(new Event('input'));
}

async function choose(file: File) {
  const input = byTestId('config_file') as HTMLInputElement;
  Object.defineProperty(input, 'files', { value: [file] });
  input.dispatchEvent(new Event('change'));
  await flushPromises();
}

async function confirm(password: string, filePassword = '') {
  type('config_password', password);
  if (filePassword) {
    type('config_file_password', filePassword);
  }
  // Enabled once the passwords are in and, for an import, the file has been read.
  await vi.waitFor(() =>
    expect(byTestId('config_transfer_confirm')?.hasAttribute('disabled')).toBe(false),
  );
  byTestId('config_transfer_confirm')?.click();
  await flushPromises();
}

async function exported(content: ConfigContent, password = '') {
  return new File([await writeConfig(content, password)], 'nas-os-config.json');
}

/** The launcher: the applications of the NAS, the ones with settings to export, the configuration. */
function launcher(path: string) {
  if (path === '/apps') {
    return Promise.resolve([
      { id: 'nas-system-terminal', name: 'Terminal', info: '', system: true },
      { id: 'nas-file-manager', name: 'Explorer', info: '', system: false },
    ]);
  }
  if (path === '/desktop/shortcuts') {
    return Promise.resolve([]);
  }
  if (path === '/system/config/apps') {
    return Promise.resolve(['nas-system-terminal', 'nas-app-manager']);
  }
  return Promise.resolve(config);
}

const configCalls = () =>
  vi
    .mocked(request)
    .mock.calls.filter(([path]) => path.startsWith('/system/config?') || path === '/system/config');

beforeEach(async () => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request)
    .mockReset()
    .mockImplementation(launcher as typeof request);
  await useAppsStore().fetchInstalled();
  vi.mocked(confirmPassword).mockReset().mockResolvedValue('c0de');
  vi.mocked(signInPolicy).mockReset().mockResolvedValue(signIn);
  vi.mocked(saveSignInPolicy).mockReset().mockResolvedValue(signIn);
  vi.mocked(saveConfigFile).mockClear();
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

describe('ConfigTransferModal', () => {
  it('exports the configuration in clear when no password is given for the file', async () => {
    const wrapper = mountModal('export');
    await flushPromises();

    await confirm('long enough');
    await vi.waitFor(() => expect(byTestId('config_exported')).not.toBeNull());

    expect(confirmPassword).toHaveBeenCalledWith('long enough');
    expect(request).toHaveBeenCalledWith('/system/config', {
      headers: { 'X-NAS-Confirmation': 'c0de' },
    });
    const text = vi.mocked(saveConfigFile).mock.calls[0]![0];
    expect(isEncrypted(text)).toBe(false);
    expect(JSON.parse(text)).toEqual({ config, signIn });
    expect(byTestId('config_exported')?.textContent).toContain('nas-os-config-2026-10-02.json');
    wrapper.unmount();
  });

  it('encrypts the file with the password chosen for it', async () => {
    const wrapper = mountModal('export');
    await flushPromises();

    await confirm('long enough', 'for the file');
    await vi.waitFor(() => expect(byTestId('config_exported')).not.toBeNull());

    const text = vi.mocked(saveConfigFile).mock.calls[0]![0];
    expect(isEncrypted(text)).toBe(true);
    await expect(readConfig(text, 'for the file')).resolves.toEqual({ config, signIn });
    wrapper.unmount();
  });

  it('exports nothing with a wrong password of the account', async () => {
    vi.mocked(confirmPassword).mockRejectedValue(new PasswordRefused('wrong password'));
    const wrapper = mountModal('export');
    await flushPromises();

    await confirm('not the one');
    await vi.waitFor(() => expect(byTestId('config_failed')).not.toBeNull());

    expect(byTestId('config_failed')?.textContent).toContain('La password non è corretta.');
    expect(configCalls()).toEqual([]);
    expect(saveConfigFile).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it('imports a file in clear without asking a password for it, then reloads', async () => {
    const reload = vi.fn();
    vi.stubGlobal('location', { reload });
    const wrapper = mountModal('import');
    await flushPromises();
    expect(document.body.textContent).toContain('Nessun file selezionato');
    expect(document.querySelectorAll('li')).toHaveLength(3);

    await choose(await exported({ config, signIn }));
    expect(document.body.textContent).toContain('nas-os-config.json');
    expect(byTestId('config_file_password')).toBeNull();
    await confirm('long enough');
    await vi.waitFor(() => expect(reload).toHaveBeenCalled());

    expect(request).toHaveBeenCalledWith('/system/config', {
      method: 'POST',
      body: JSON.stringify(config),
      headers: { 'X-NAS-Confirmation': 'c0de' },
    });
    expect(saveSignInPolicy).toHaveBeenCalledWith(signIn);
    wrapper.unmount();
  });

  it('imports an encrypted file with its own password', async () => {
    const reload = vi.fn();
    vi.stubGlobal('location', { reload });
    const wrapper = mountModal('import');
    await flushPromises();

    await choose(await exported({ config, signIn }, 'for the file'));
    expect(byTestId('config_transfer_confirm')?.hasAttribute('disabled')).toBe(true);
    await confirm('the account one', 'for the file');
    await vi.waitFor(() => expect(reload).toHaveBeenCalled());

    expect(confirmPassword).toHaveBeenCalledWith('the account one');
    wrapper.unmount();
  });

  it('imports nothing from a file it cannot open with the password given', async () => {
    const wrapper = mountModal('import');
    await flushPromises();

    await choose(await exported({ config, signIn }, 'for the file'));
    await confirm('long enough', 'another one');
    await vi.waitFor(() => expect(byTestId('config_failed')).not.toBeNull());

    expect(byTestId('config_failed')?.textContent).toContain('È richiesta la password scelta');
    expect(confirmPassword).not.toHaveBeenCalled();
    expect(configCalls()).toEqual([]);
    wrapper.unmount();
  });

  it('says at once when the file is not a configuration', async () => {
    const wrapper = mountModal('import');
    await flushPromises();

    await choose(new File(['{"hello":"world"}'], 'other.json'));
    await vi.waitFor(() => expect(byTestId('config_failed')).not.toBeNull());

    expect(byTestId('config_failed')?.textContent).toContain('non è una configurazione');
    expect(byTestId('config_transfer_confirm')?.hasAttribute('disabled')).toBe(true);
    wrapper.unmount();
  });

  it('puts in the file the settings of the applications chosen, and offers only those that have some', async () => {
    const wrapper = mountModal('export');
    await flushPromises();

    const terminal = byTestId('config_app_nas-system-terminal') as HTMLInputElement;
    const explorer = byTestId('config_app_nas-file-manager') as HTMLInputElement;
    expect(explorer.disabled).toBe(true);
    expect(explorer.closest('label')?.textContent).toContain('Nessuna impostazione da esportare');
    terminal.click();
    await flushPromises();
    await confirm('long enough');
    await vi.waitFor(() => expect(byTestId('config_exported')).not.toBeNull());

    expect(request).toHaveBeenCalledWith('/system/config?apps=nas-system-terminal', {
      headers: { 'X-NAS-Confirmation': 'c0de' },
    });

    wrapper.unmount();
  });

  it('takes an application out of the file when it is unchecked', async () => {
    const wrapper = mountModal('export');
    await flushPromises();

    const terminal = byTestId('config_app_nas-system-terminal') as HTMLInputElement;
    terminal.click();
    await flushPromises();
    terminal.click();
    await flushPromises();
    await confirm('long enough');
    await vi.waitFor(() => expect(byTestId('config_exported')).not.toBeNull());

    expect(configCalls()[0]?.[0]).toBe('/system/config');
    wrapper.unmount();
  });

  it('lists no application to choose when importing', async () => {
    const wrapper = mountModal('import');
    await flushPromises();

    expect(byTestId('config_apps')).toBeNull();
    expect(request).not.toHaveBeenCalledWith('/system/config/apps');
    wrapper.unmount();
  });
});
