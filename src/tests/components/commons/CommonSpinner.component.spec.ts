import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import CommonSpinner from '@/components/commons/CommonSpinner.vue';
import { setLocale } from '@/i18n';

beforeEach(() => setLocale('en'));

describe('CommonSpinner', () => {
  it('announces the default label as a status', () => {
    const wrapper = mount(CommonSpinner, { props: { testid: 'spinner' } });

    const status = wrapper.get('[data-testid="spinner"]');
    expect(status.element.tagName).toBe('OUTPUT');
    expect(status.attributes('aria-live')).toBe('polite');
    expect(status.text()).toBe('Loading');
  });

  it('says what is loading when given a label', () => {
    const wrapper = mount(CommonSpinner, { props: { label: 'Reading the disks' } });

    expect(wrapper.text()).toBe('Reading the disks');
  });

  it('keeps the label for assistive technology when compact', () => {
    const wrapper = mount(CommonSpinner, { props: { compact: true } });

    expect(wrapper.get('span.visually_hidden').text()).toBe('Loading');
  });

  it.each(['small', 'medium', 'large'] as const)('draws the %s size', (size) => {
    const wrapper = mount(CommonSpinner, { props: { size } });

    expect(wrapper.find(`.common_spinner_${size}`).exists()).toBe(true);
  });

  it('fills and centers in its container only when asked', async () => {
    const wrapper = mount(CommonSpinner);
    expect(wrapper.classes()).not.toContain('common_spinner_centered');

    await wrapper.setProps({ centered: true });
    expect(wrapper.classes()).toContain('common_spinner_centered');
  });
});
