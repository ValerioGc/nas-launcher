import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import CommonEmpty from '@/components/commons/CommonEmpty.vue';

describe('CommonEmpty', () => {
  it('says what is missing, and what would fill it when given a hint', () => {
    const wrapper = mount(CommonEmpty, {
      props: { title: 'No applications', hint: 'Install one from the catalogue', testid: 'empty' },
    });

    expect(wrapper.get('[data-testid="empty"]').text()).toContain('No applications');
    expect(wrapper.text()).toContain('Install one from the catalogue');
    expect(wrapper.find('[data-testid="common_icon"]').exists()).toBe(false);
  });

  it('draws an icon only when it has one', () => {
    const wrapper = mount(CommonEmpty, { props: { title: 'Nothing', icon: '/grid.svg' } });

    expect(wrapper.find('[data-testid="common_icon"]').exists()).toBe(true);
    expect(wrapper.findAll('p')).toHaveLength(1);
  });
});
