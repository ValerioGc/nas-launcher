import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import CommonField from '@/components/commons/CommonField.vue';

const control = '<label for="name">Nome</label><input id="name" />';

it('holds the label and the control it is given, in that order', () => {
  const wrapper = mount(CommonField, { slots: { default: control } });

  expect(wrapper.get('label').attributes('for')).toBe('name');
  expect(wrapper.get('label').element.nextElementSibling).toBe(wrapper.get('input#name').element);
  expect(wrapper.find('[aria-live="polite"]').exists()).toBe(false);
});

it('says what is wrong under the control, as it changes', async () => {
  const wrapper = mount(CommonField, {
    props: { error: 'Troppo corto.' },
    slots: { default: control },
  });

  expect(wrapper.get('[aria-live="polite"]').text()).toBe('Troppo corto.');
  await wrapper.setProps({ error: '' });
  expect(wrapper.find('[aria-live="polite"]').exists()).toBe(false);
});
