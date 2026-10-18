import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import SettingsTabLayout from '@/apps/nas-system-settings/components/SettingsTabLayout.vue';
import { setLocale } from '@/i18n';

const slots = {
  default: '<p data-testid="body">body</p>',
  footer: '<p data-testid="footer">footer</p>',
};

beforeEach(() => setLocale('en'));

describe('SettingsTabLayout', () => {
  it('renders the title, the body and the footer in the element it is asked for', () => {
    const wrapper = mount(SettingsTabLayout, { props: { title: 'Access', as: 'form' }, slots });

    expect(wrapper.get('form h1').text()).toBe('Access');
    expect(wrapper.find('[data-testid="body"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="footer"]').exists()).toBe(true);
  });

  it('shows a centered spinner instead of the tab until it is ready', () => {
    const wrapper = mount(SettingsTabLayout, {
      props: { title: 'Access', ready: false, loading: true },
      slots,
    });

    expect(wrapper.find('h1').exists()).toBe(false);
    expect(wrapper.get('[data-testid="settings_loading"]').classes()).toContain(
      'common_spinner_centered',
    );
  });

  it('passes a retry on when the tab could not be read', async () => {
    const wrapper = mount(SettingsTabLayout, {
      props: { title: 'Access', ready: false, error: 'Refused' },
      slots,
    });

    await wrapper.get('[data-testid="settings_error"] button').trigger('click');
    expect(wrapper.emitted('retry')).toHaveLength(1);
  });
});
