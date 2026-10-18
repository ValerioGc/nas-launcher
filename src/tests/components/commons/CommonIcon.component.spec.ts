import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import CommonIcon from '@/components/commons/CommonIcon.vue';

describe('CommonIcon', () => {
  it('hides itself from screen readers when it has no label', () => {
    const wrapper = mount(CommonIcon, { props: { src: '/menu.svg' } });
    expect(wrapper.attributes('aria-hidden')).toBe('true');
    expect(wrapper.attributes('role')).toBeUndefined();
    wrapper.unmount();
  });

  it('exposes an accessible name when a label is given', () => {
    const wrapper = mount(CommonIcon, { props: { src: '/menu.svg', label: 'Menu' } });
    expect(wrapper.attributes('role')).toBe('img');
    expect(wrapper.attributes('aria-label')).toBe('Menu');
    expect(wrapper.attributes('aria-hidden')).toBeUndefined();
    wrapper.unmount();
  });

  it('paints the icon from its source and follows the requested size', () => {
    const wrapper = mount(CommonIcon, { props: { src: '/menu.svg', size: 'large' } });
    expect(wrapper.attributes('style')).toContain('url("/menu.svg")');
    expect(wrapper.classes()).toContain('common_icon_large');
    wrapper.unmount();
  });
});
