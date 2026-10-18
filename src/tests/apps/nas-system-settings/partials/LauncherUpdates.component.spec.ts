import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import LauncherUpdates from '@/apps/nas-system-settings/components/LauncherUpdates.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { NasUpdates } from '@/apps/nas-system-settings/types/update';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

function updates(
  launcher: string | null = null,
  gateway: string | null = null,
  options: { repository?: string; tokenConfigured?: boolean } = {},
): NasUpdates {
  return {
    components: [
      {
        component: 'launcher',
        currentVersion: '1.0.0',
        availableVersion: launcher,
        repository: options.repository ?? 'ValerioGc/nas-launcher',
      },
      {
        component: 'gateway',
        currentVersion: null,
        availableVersion: gateway,
        repository: 'ValerioGc/nas-gateway',
      },
    ],
    tokenConfigured: options.tokenConfigured ?? true,
  };
}

const configured = updates();

const versions = (wrapper: { findAll: (selector: string) => { element: Element }[] }) =>
  wrapper
    .findAll('[data-testid^="nas_component_"]')
    .map(({ element }) =>
      [...element.querySelectorAll('.component_name, .component_versions > span:not(.update_dot)')]
        .map((part) => part.textContent?.trim())
        .join(' '),
    );
const tokenLabel = (wrapper: {
  get: (selector: string) => { attributes: (name: string) => string | undefined };
}) => wrapper.get('[data-testid="launcher_token"] [role="img"]').attributes('aria-label');

beforeEach(() => {
  setLocale('it');
  vi.mocked(request).mockReset();
});

describe('launcher updates', () => {
  it('lists the components with their version, and does not reach the repository on its own', async () => {
    vi.mocked(request).mockResolvedValue(configured);
    const wrapper = mount(LauncherUpdates);
    await flushPromises();

    expect(wrapper.find('[data-testid="launcher_repository"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="launcher_token"]').text()).toContain('Token repository');
    expect(tokenLabel(wrapper)).toBe('Token configurato');
    expect(wrapper.find('input[type="password"]').exists()).toBe(false);
    expect(versions(wrapper)).toEqual(['Launcher e interfaccia 1.0.0', 'Gateway -']);
    expect(wrapper.find('.update_dot').exists()).toBe(false);
    expect(vi.mocked(request).mock.calls).toEqual([['/launcher/updates']]);
    wrapper.unmount();
  });

  it('keeps successful check feedback out of the settings page', async () => {
    vi.mocked(request).mockResolvedValue(configured);
    const wrapper = mount(LauncherUpdates);
    await flushPromises();

    await wrapper.get('[data-testid="launcher_check"]').trigger('click');
    await flushPromises();
    expect(wrapper.find('[data-testid="launcher_up_to_date"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('marks a missing token with its own icon', async () => {
    vi.mocked(request).mockResolvedValue(updates(null, null, { tokenConfigured: false }));
    const wrapper = mount(LauncherUpdates);
    await flushPromises();

    expect(tokenLabel(wrapper)).toBe('Token non configurato');
    wrapper.unmount();
  });

  it('says so when nobody configured a repository', async () => {
    vi.mocked(request).mockResolvedValue(updates(null, null, { repository: '' }));
    const wrapper = mount(LauncherUpdates);
    await flushPromises();

    expect(wrapper.get('[data-testid="launcher_no_source"]').text()).toContain(
      'Nessun repository configurato',
    );
    expect(wrapper.get('[data-testid="launcher_check"]').attributes('disabled')).toBeDefined();
    wrapper.unmount();
  });

  it('asks the repository only when the button is pressed, and shows what it found', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce(configured)
      .mockResolvedValueOnce(updates('1.2.0', '0.1.15'))
      .mockResolvedValueOnce(updates('1.2.0', '0.1.15'));
    const wrapper = mount(LauncherUpdates);
    await flushPromises();

    await wrapper.get('[data-testid="launcher_check"]').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/launcher/updates/check', {
      method: 'POST',
      body: '{}',
    });

    expect(versions(wrapper)).toEqual(['Launcher e interfaccia 1.0.0 1.2.0', 'Gateway - 0.1.15']);
    expect(wrapper.findAll('.update_dot')).toHaveLength(2);
    expect(wrapper.get('[data-testid="launcher_update"]').text()).toBe('Aggiorna tutti');
    expect(wrapper.find('[data-testid="launcher_check"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('opens the update window, asking before anything starts', async () => {
    vi.mocked(request).mockResolvedValueOnce(updates('1.2.0', '0.1.15'));
    const wrapper = mount(LauncherUpdates, { attachTo: document.body });
    await flushPromises();

    await wrapper.get('[data-testid="launcher_update"]').trigger('click');
    await flushPromises();

    expect(document.body.textContent).toContain('Launcher e interfaccia da 1.0.0 a 1.2.0');
    expect(document.body.textContent).toContain('Gateway da - a 0.1.15');
    expect(request).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });

  it('says why a check failed and permits retrying it', async () => {
    vi.mocked(request)
      .mockResolvedValueOnce(configured)
      .mockRejectedValueOnce(new Error('the repository refused the token'));
    const wrapper = mount(LauncherUpdates);
    await flushPromises();

    await wrapper.get('[data-testid="launcher_check"]').trigger('click');
    await flushPromises();

    expect(wrapper.get('[role="alert"]').text()).toContain('refused the token');
    expect(versions(wrapper)[0]).toBe('Launcher e interfaccia 1.0.0');
    expect(wrapper.get('[data-testid="launcher_check"]').attributes('disabled')).toBeUndefined();
    wrapper.unmount();
  });

  it('offers to update only when a newer release is there', async () => {
    vi.mocked(request).mockResolvedValue(configured);
    const current = mount(LauncherUpdates);
    await flushPromises();
    expect(current.find('[data-testid="launcher_update"]').exists()).toBe(false);
    current.unmount();

    vi.mocked(request).mockResolvedValue(updates(null, '0.1.15'));
    const newer = mount(LauncherUpdates);
    await flushPromises();
    expect(newer.get('[data-testid="launcher_update"]').text()).toBe('Aggiorna tutti');
    newer.unmount();
  });
});
