import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import CommonSegmented from '@/components/commons/CommonSegmented.vue';
import { setLocale } from '@/i18n';

const options = [
  { value: 'start', label: 'start', icon: '/start.svg' },
  { value: 'shutdown', label: 'shutdown' },
];

beforeEach(() => setLocale('en'));

describe('CommonSegmented', () => {
  it('shows every option translated, as a labelled group, with the chosen one pressed', () => {
    const wrapper = mount(CommonSegmented, {
      props: { options, label: 'Action', modelValue: 'shutdown', testid: 'choice' },
    });

    expect(wrapper.element.tagName).toBe('FIELDSET');
    expect(wrapper.attributes('aria-label')).toBe('Action');
    expect(wrapper.findAll('button').map((button) => button.text())).toEqual([
      'Start',
      'Shut down',
    ]);
    expect(wrapper.get('[data-testid="choice_shutdown"]').attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('[data-testid="choice_start"]').attributes('aria-pressed')).toBe('false');
    expect(wrapper.findAll('[data-testid="common_icon"]')).toHaveLength(1);
  });

  it('chooses the option that is pressed', async () => {
    const wrapper = mount(CommonSegmented, {
      props: { options, label: 'Action', modelValue: 'shutdown', testid: 'choice' },
    });

    await wrapper.get('[data-testid="choice_start"]').trigger('click');
    expect(wrapper.emitted('update:modelValue')?.[0]).toEqual(['start']);
  });
});
