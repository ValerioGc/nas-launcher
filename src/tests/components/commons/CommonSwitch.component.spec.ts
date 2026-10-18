import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import CommonSwitch from '@/components/commons/CommonSwitch.vue';

it('is a checkbox with a label, and reports every change', async () => {
  const wrapper = mount(CommonSwitch, {
    props: { modelValue: false, label: 'Mostra le app di sistema', testid: 'demo_switch' },
  });

  const input = wrapper.get('input[type="checkbox"]');
  expect(wrapper.text()).toContain('Mostra le app di sistema');
  expect(input.attributes('data-testid')).toBe('demo_switch_input');
  expect(wrapper.get('[data-testid="demo_switch"]').element.tagName).toBe('LABEL');
  await input.setValue(true);
  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([true]);
  await wrapper.setProps({ modelValue: true });
  expect((input.element as HTMLInputElement).checked).toBe(true);
  wrapper.unmount();
});

it('keeps a hidden label for assistive technology when asked', () => {
  const wrapper = mount(CommonSwitch, {
    props: { modelValue: true, label: 'Abilitato Lunedì 07:00', hideLabel: true },
  });

  expect(wrapper.get('.common_switch_label').classes()).toContain('visually_hidden');
  expect(wrapper.get('label').text()).toBe('Abilitato Lunedì 07:00');
  wrapper.unmount();
});

it('names its checkbox in the form, after its test id unless told otherwise', () => {
  const byTestid = mount(CommonSwitch, {
    props: { modelValue: false, label: 'A', testid: 'demo_switch' },
  });
  expect(byTestid.get('input').attributes('name')).toBe('demo_switch');
  const named = mount(CommonSwitch, {
    props: { modelValue: false, label: 'B', testid: 'demo_switch', name: 'chosen' },
  });
  expect(named.get('input').attributes('name')).toBe('chosen');
});
