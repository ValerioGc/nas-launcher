import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import CommonDropdown from '@/components/commons/CommonDropdown.vue';

function mountDropdown() {
  return mount(CommonDropdown, {
    props: { icon: '/power.svg', triggerLabel: 'Azioni di sistema' },
    slots: { default: '<li><button type="button">Riavvia</button></li>' },
    attachTo: document.body,
  });
}

describe('CommonDropdown', () => {
  it('starts closed and announces the state on the trigger', () => {
    const wrapper = mountDropdown();
    const trigger = wrapper.get('[data-testid="common_dropdown_trigger"]');
    expect(trigger.attributes('aria-expanded')).toBe('false');
    expect(wrapper.find('[data-testid="common_dropdown_content"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('opens on click and closes again on a second click', async () => {
    const wrapper = mountDropdown();
    const trigger = wrapper.get('[data-testid="common_dropdown_trigger"]');

    await trigger.trigger('click');
    expect(trigger.attributes('aria-expanded')).toBe('true');
    expect(wrapper.get('[data-testid="common_dropdown_content"]').text()).toContain('Riavvia');

    await trigger.trigger('click');
    expect(wrapper.find('[data-testid="common_dropdown_content"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('closes on Escape', async () => {
    const wrapper = mountDropdown();
    await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
    await wrapper.trigger('keydown', { key: 'Escape' });
    expect(wrapper.find('[data-testid="common_dropdown_content"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('closes when the pointer goes down outside of it', async () => {
    const wrapper = mountDropdown();
    await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');

    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-testid="common_dropdown_content"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
