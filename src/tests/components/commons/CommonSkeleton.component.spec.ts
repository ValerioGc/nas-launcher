import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import CommonSkeleton from '@/components/commons/CommonSkeleton.vue';

describe('CommonSkeleton', () => {
  it('is a medium placeholder that screen readers skip', () => {
    const wrapper = mount(CommonSkeleton);

    expect(wrapper.attributes('aria-hidden')).toBe('true');
    expect(wrapper.classes()).toContain('common_skeleton_medium');
  });

  it.each(['short', 'long', 'icon'] as const)('takes the %s width', (width) => {
    const wrapper = mount(CommonSkeleton, { props: { width } });

    expect(wrapper.classes()).toContain(`common_skeleton_${width}`);
  });
});
