import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import SettingsStatus from '@/apps/nas-system-settings/components/SettingsStatus.vue';
import { setLocale } from '@/i18n';

beforeEach(() => setLocale('en'));

describe('SettingsStatus', () => {
  it('shows nothing when there is nothing to say', () => {
    const wrapper = mount(SettingsStatus);

    expect(wrapper.find('[data-testid="settings_loading"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="settings_error"]').exists()).toBe(false);
  });

  it('spins inline while busy, and fills the tab when centered', async () => {
    const wrapper = mount(SettingsStatus, { props: { loading: true, busyLabel: 'Scanning' } });
    const spinner = wrapper.get('[data-testid="settings_loading"]');
    expect(spinner.text()).toBe('Scanning');
    expect(spinner.classes()).not.toContain('common_spinner_centered');

    await wrapper.setProps({ centered: true });
    expect(wrapper.get('[data-testid="settings_loading"]').classes()).toContain(
      'common_spinner_centered',
    );
  });

  it('reports an error, and offers to retry only when asked to', async () => {
    const wrapper = mount(SettingsStatus, { props: { error: 'Refused' } });
    expect(wrapper.get('[data-testid="settings_error"]').text()).toContain('Refused');
    expect(wrapper.find('button').exists()).toBe(false);

    await wrapper.setProps({ retryable: true });
    await wrapper.get('button').trigger('click');
    expect(wrapper.emitted('retry')).toHaveLength(1);
  });
});
