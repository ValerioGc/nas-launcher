import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import CommonSaveFooter from '@/components/commons/CommonSaveFooter.vue';
import { setLocale } from '@/i18n';

beforeEach(() => setLocale('en'));

describe('CommonSaveFooter', () => {
  it('submits the form it sits in, and is disabled while saving', async () => {
    const wrapper = mount(CommonSaveFooter, {
      props: { saved: false, busy: false, testid: 'save' },
    });
    const button = wrapper.get('[data-testid="save"]');

    expect(button.attributes('type')).toBe('submit');
    expect(button.text()).toBe('Save');
    expect(button.attributes('disabled')).toBeUndefined();

    await wrapper.setProps({ busy: true });
    expect(button.attributes('disabled')).toBeDefined();
  });

  it('leaves the button out while there is nothing to save', async () => {
    const wrapper = mount(CommonSaveFooter, {
      props: { saved: true, busy: false, changed: false, testid: 'save' },
    });
    expect(wrapper.find('[data-testid="save"]').exists()).toBe(false);

    await wrapper.setProps({ changed: true });
    expect(wrapper.find('[data-testid="save"]').exists()).toBe(true);
  });

  it('does not duplicate successful saves inside the form', async () => {
    const wrapper = mount(CommonSaveFooter, { props: { saved: false, busy: false } });
    expect(wrapper.find('[data-testid="save_footer_saved"]').exists()).toBe(false);

    await wrapper.setProps({ saved: true });
    expect(wrapper.find('[data-testid="save_footer_saved"]').exists()).toBe(false);
  });

  it('shows what the caller adds next to the button', () => {
    const wrapper = mount(CommonSaveFooter, {
      props: { saved: false, busy: false },
      slots: { default: '<p role="alert">Refused</p>' },
    });

    expect(wrapper.get('[role="alert"]').text()).toBe('Refused');
  });
});
