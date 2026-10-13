import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';

import AlertBanner from '@/components/alerts/AlertBanner.vue';

describe('alerts', () => {
  it('announces errors and emits dismissal without interpreting titles as HTML', async () => {
    const wrapper = mount(AlertBanner, {
      props: { title: '<img onerror=alert(1)>', tone: 'error', dismissible: true },
      slots: { default: 'Message' },
    });

    expect(wrapper.attributes('role')).toBe('alert');
    expect(wrapper.find('img').exists()).toBe(false);
    expect(wrapper.text()).toContain('<img onerror=alert(1)>');
    await wrapper.get('button').trigger('click');
    expect(wrapper.emitted('dismiss')).toHaveLength(1);
    wrapper.unmount();
  });

  it('announces the rest as a status, and draws a compact variant on request', async () => {
    const wrapper = mount(AlertBanner, { props: { tone: 'success' }, slots: { default: 'Saved' } });
    expect(wrapper.attributes('role')).toBe('status');
    expect(wrapper.classes()).not.toContain('alert_banner_compact');

    await wrapper.setProps({ compact: true });
    expect(wrapper.classes()).toContain('alert_banner_compact');
    wrapper.unmount();
  });
});
