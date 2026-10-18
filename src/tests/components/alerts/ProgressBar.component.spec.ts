import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';

import ProgressBar from '@/components/alerts/ProgressBar.vue';

describe('progress bar', () => {
  it.each([
    [42.4, 42],
    [-10, 0],
    [500, 100],
    [NaN, 0],
    [Infinity, 0],
  ])('announces a valid value for %s', (value, expected) => {
    const wrapper = mount(ProgressBar, { props: { value, label: 'CPU' } });
    expect(wrapper.element.tagName).toBe('PROGRESS');
    expect(wrapper.attributes('value')).toBe(String(expected));
    expect(wrapper.attributes('max')).toBe('100');
    expect(wrapper.attributes('aria-label')).toBe('CPU');
    wrapper.unmount();
  });

  it('draws no position at all when there is none to draw', () => {
    const wrapper = mount(ProgressBar, { props: { value: null, label: 'Copia' } });

    expect(wrapper.attributes('value')).toBeUndefined();
    expect(wrapper.classes()).toContain('progress_bar_waiting');
    expect(wrapper.attributes('aria-label')).toBe('Copia');
    wrapper.unmount();
  });
});
