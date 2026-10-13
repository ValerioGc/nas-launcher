import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, expect, it } from 'vitest';
import WidgetDrawer from '@/widgets/WidgetDrawer.vue';

afterEach(() => {
  document.body.innerHTML = '';
});

const props = { panelId: 'demo_panel', label: 'Demo', testid: 'demo_panel', modelValue: false };

it('takes the focus when it opens, gives it back when it closes and answers Esc', async () => {
  const trigger = document.createElement('button');
  document.body.append(trigger);
  trigger.focus();

  const wrapper = mount(WidgetDrawer, { props, attachTo: document.body });
  expect(wrapper.find('aside').exists()).toBe(false);
  await wrapper.setProps({ modelValue: true });
  await flushPromises();
  expect(document.activeElement).toBe(wrapper.get('aside').element);
  await wrapper.get('aside').trigger('keydown', { key: 'Escape' });
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
  await wrapper.setProps({ modelValue: false });
  expect(document.activeElement).toBe(trigger);

  wrapper.unmount();
});

it('closes on a click outside without taking focus back from the clicked control', async () => {
  const outside = document.createElement('button');
  document.body.append(outside);
  const wrapper = mount(WidgetDrawer, {
    props,
    slots: { default: '<button data-testid="inside">Inside</button>' },
    attachTo: document.body,
  });
  await wrapper.setProps({ modelValue: true });
  await flushPromises();

  await wrapper.get('[data-testid="inside"]').trigger('click');
  expect(wrapper.emitted('update:modelValue')).toBeUndefined();

  outside.focus();
  outside.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  await flushPromises();
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
  await wrapper.setProps({ modelValue: false });
  expect(document.activeElement).toBe(outside);

  wrapper.unmount();
});

it('closes from the backdrop covering content outside the drawer', async () => {
  const wrapper = mount(WidgetDrawer, { props, attachTo: document.body });
  await wrapper.setProps({ modelValue: true });
  await flushPromises();

  await wrapper.get('[data-testid="widget_drawer_backdrop"]').trigger('click');
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);

  wrapper.unmount();
});
