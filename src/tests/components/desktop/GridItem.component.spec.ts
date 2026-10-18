import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import GridItem from '@/components/desktop/GridItem.vue';

function open(props = {}) {
  return mount(GridItem, {
    props: { name: 'Gestore applicazioni', icon: '/logo.svg', ...props },
  });
}

describe('desktop icon', () => {
  it('shows the name next to the icon', () => {
    const wrapper = open();

    expect(wrapper.get('span').text()).toBe('Gestore applicazioni');
    expect(wrapper.get('img').attributes('src')).toBe('/logo.svg');
    expect(wrapper.get('img').attributes('alt')).toBe('');
    wrapper.unmount();
  });

  it('asks to be opened when it is clicked', async () => {
    const wrapper = open();
    await wrapper.get('button').trigger('click');

    expect(wrapper.emitted('open')).toHaveLength(1);
    wrapper.unmount();
  });

  it.each(['contextmenu', 'keydown'])('asks for its context menu on %s', async (event) => {
    const wrapper = open();
    await wrapper.get('button').trigger(event, { key: 'F10', shiftKey: true });

    expect(wrapper.emitted('context')).toHaveLength(1);
    expect(wrapper.emitted('open')).toBeUndefined();
    wrapper.unmount();
  });

  it.each([
    ['ArrowLeft', -1],
    ['ArrowRight', 1],
  ])('asks to move with Alt and %s', async (key, step) => {
    const wrapper = open();
    await wrapper.get('button').trigger('keydown', { key, altKey: true });

    expect(wrapper.emitted('move')).toEqual([[step]]);
    expect(wrapper.emitted('open')).toBeUndefined();
    wrapper.unmount();
  });
});
