import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LauncherUpdateModal from '@/apps/nas-system-settings/components/LauncherUpdateModal.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

const task = (detail: string, state = 'running', progress: number | null = null) => ({
  id: 'nas-update',
  detail,
  state,
  progress,
});

let respond: (path: string) => unknown;

function mountModal() {
  return mount(LauncherUpdateModal, {
    attachTo: document.body,
    props: {
      modelValue: true,
      components: [
        {
          component: 'launcher' as const,
          currentVersion: '0.1.6',
          availableVersion: '0.2.0',
          repository: 'ValerioGc/nas-launcher',
        },
        {
          component: 'wizard' as const,
          currentVersion: '0.1.3',
          availableVersion: '0.1.4',
          repository: 'ValerioGc/nas-wizard',
        },
      ],
    },
  });
}

const text = () => document.body.textContent ?? '';
const byTestId = (id: string) => document.querySelector<HTMLElement>(`[data-testid="${id}"]`);

beforeEach(() => {
  setLocale('it');
  vi.useFakeTimers();
  vi.mocked(request)
    .mockReset()
    .mockImplementation(async (path: string) => {
      const answer = respond(path);
      if (answer instanceof Error) {
        throw answer;
      }
      return answer as never;
    });
});

afterEach(() => {
  vi.useRealTimers();
  document.body.innerHTML = '';
});

async function tick() {
  await vi.advanceTimersByTimeAsync(2000);
  await flushPromises();
}

describe('LauncherUpdateModal', () => {
  it('asks first, then follows the download and the check of the package', async () => {
    respond = (path) =>
      path === '/launcher/updates' ? { accepted: true, task: 'nas-update' } : [];
    const wrapper = mountModal();
    expect(text()).toContain('riavviarsi');
    expect(byTestId('launcher_update_plan')?.textContent).toContain('Wizard da 0.1.3 a 0.1.4');
    expect(request).not.toHaveBeenCalled();

    byTestId('launcher_update_start')?.click();
    await flushPromises();
    expect(request).toHaveBeenCalledWith('/launcher/updates', { method: 'POST', body: '{}' });
    expect(byTestId('launcher_update_step')?.textContent).toContain('Scaricamento in corso');
    expect(byTestId('common_modal_close')).toBeNull();

    respond = () => [task('downloading-wizard', 'running', 45)];
    await tick();
    expect(byTestId('launcher_update_step')?.textContent).toContain(
      'Scaricamento e verifica di Wizard',
    );
    respond = () => [task('verifying', 'running', 80)];
    await tick();
    expect(byTestId('launcher_update_step')?.textContent).toContain('Verifica');
    expect(byTestId('progress_bar')?.getAttribute('value')).toBe('80');
    wrapper.unmount();
  });

  it('waits for the restart and reloads the page on the new version', async () => {
    const reload = vi.fn();
    vi.stubGlobal('location', { reload });
    respond = (path) =>
      path === '/launcher/updates' ? { accepted: true, task: 'nas-update' } : [];
    const wrapper = mountModal();
    byTestId('launcher_update_start')?.click();
    await flushPromises();

    respond = () => [task('installing', 'done', 100)];
    await tick();
    respond = () => new Error('down');
    await tick();
    expect(byTestId('launcher_update_step')?.textContent).toContain('Riavvio del NAS in corso');

    respond = () => ({
      components: [
        { component: 'launcher', currentVersion: '0.2.0' },
        { component: 'wizard', currentVersion: '0.1.4' },
      ],
    });
    await tick();
    expect(reload).toHaveBeenCalled();
    vi.unstubAllGlobals();
    wrapper.unmount();
  });

  it('says the update was rolled back when a component came back on its old version', async () => {
    respond = (path) =>
      path === '/launcher/updates' ? { accepted: true, task: 'nas-update' } : [];
    const wrapper = mountModal();
    byTestId('launcher_update_start')?.click();
    await flushPromises();

    respond = () => new Error('down');
    await tick();
    respond = () => ({
      components: [
        { component: 'launcher', currentVersion: '0.2.0' },
        { component: 'wizard', currentVersion: '0.1.3' },
      ],
    });
    await tick();

    expect(byTestId('launcher_update_failed')?.textContent).toContain('annullato');
    wrapper.unmount();
  });

  it('shows why the download or the check failed, and can be closed', async () => {
    respond = (path) =>
      path === '/launcher/updates' ? { accepted: true, task: 'nas-update' } : [];
    const wrapper = mountModal();
    byTestId('launcher_update_start')?.click();
    await flushPromises();

    respond = () => [task('the release published no checksum', 'failed')];
    await tick();

    expect(byTestId('launcher_update_failed')?.textContent).toContain('no checksum');
    expect(byTestId('common_modal_close')).not.toBeNull();
    wrapper.unmount();
  });

  it('shows a refused start at once', async () => {
    respond = () => new Error('no launcher update available');
    const wrapper = mountModal();
    byTestId('launcher_update_start')?.click();
    await flushPromises();

    expect(byTestId('launcher_update_failed')?.textContent).toContain('no launcher update');
    wrapper.unmount();
  });
});
