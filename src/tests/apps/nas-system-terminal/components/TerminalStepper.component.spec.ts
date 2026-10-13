import { mount } from '@vue/test-utils';
import { beforeEach, expect, it } from 'vitest';
import TerminalStepper from '@/apps/nas-system-terminal/components/TerminalStepper.vue';
import { setLocale } from '@/i18n';

beforeEach(() => setLocale('it'));

function stepper(modelValue: number) {
  const wrapper = mount(TerminalStepper, {
    props: {
      id: 'height',
      min: 1,
      max: 2.5,
      step: 0.1,
      modelValue,
      'onUpdate:modelValue': (value: number) => wrapper.setProps({ modelValue: value }),
    },
  });
  return wrapper;
}

it('moves by one step with its buttons, rounded to the step', async () => {
  const wrapper = stepper(1.1);

  await wrapper.get('[data-testid="height_increase"]').trigger('click');
  await wrapper.get('[aria-label="Diminuisci"]').trigger('click');

  expect(wrapper.emitted('update:modelValue')).toEqual([[1.2], [1.1]]);
});

it('stays within its range, and turns the button off at the edge', async () => {
  const wrapper = stepper(2);

  await wrapper.get('input').setValue('9');
  await wrapper.get('input').trigger('change');

  expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([2.5]);
  expect(wrapper.get('[data-testid="height_increase"]').attributes('disabled')).toBeDefined();
});

it('ignores what is not a number', async () => {
  const wrapper = stepper(1.5);

  await wrapper.get('input').setValue('');
  (wrapper.get('input').element as HTMLInputElement).value = 'abc';
  await wrapper.get('input').trigger('change');

  expect(wrapper.emitted('update:modelValue')?.flat() ?? []).not.toContain(Number.NaN);
});
