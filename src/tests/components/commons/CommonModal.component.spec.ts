import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import CommonModal from '@/components/commons/CommonModal.vue';

function mountModal(open = true) {
  return mount(CommonModal, {
    props: { modelValue: open, title: 'Eject confirmation' },
    slots: { default: '<p>Corpo della modale</p>' },
    attachTo: document.body,
  });
}

describe('CommonModal', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders nothing while it is closed', () => {
    const wrapper = mountModal(false);
    expect(document.querySelector('[data-testid="common_modal"]')).toBeNull();
    wrapper.unmount();
  });

  it('renders a labelled dialog with its content when open', () => {
    const wrapper = mountModal();
    const dialog = document.querySelector('dialog');
    expect(dialog?.getAttribute('aria-label')).toBe('Eject confirmation');
    expect(dialog?.getAttribute('aria-modal')).toBe('true');
    expect(document.body.textContent).toContain('Corpo della modale');
    wrapper.unmount();
  });

  it.each([
    ['il bottone di chiusura', '[data-testid="common_modal_close"]'],
    ['lo sfondo', '[data-testid="common_modal_backdrop"]'],
  ])('asks to close from %s', async (_name, selector) => {
    const wrapper = mountModal();
    document.querySelector(selector)?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await wrapper.vm.$nextTick();
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
    wrapper.unmount();
  });

  it('asks to close on Escape', async () => {
    const wrapper = mountModal();
    document
      .querySelector('dialog')
      ?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await wrapper.vm.$nextTick();
    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
    wrapper.unmount();
  });

  it('moves the focus into the dialog when it opens', async () => {
    const wrapper = mountModal(false);
    await wrapper.setProps({ modelValue: true });
    await wrapper.vm.$nextTick();
    expect(document.activeElement).toBe(document.querySelector('dialog'));
    wrapper.unmount();
  });

  it('gives the focus back to whatever opened it', async () => {
    const opener = document.createElement('button');
    document.body.append(opener);
    opener.focus();

    const wrapper = mountModal(false);
    await wrapper.setProps({ modelValue: true });
    await wrapper.vm.$nextTick();
    await wrapper.setProps({ modelValue: false });
    await wrapper.vm.$nextTick();

    expect(document.activeElement).toBe(opener);
    wrapper.unmount();
  });

  it('stops Tab from leaving the dialog', async () => {
    const wrapper = mountModal();
    await wrapper.vm.$nextTick();

    const event = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
    document.querySelector('dialog')?.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    wrapper.unmount();
  });

  it('can be told to do without its close button', () => {
    const wrapper = mount(CommonModal, {
      props: { modelValue: true, title: 'Eject confirmation', hideCloseButton: true },
      attachTo: document.body,
    });

    expect(document.querySelector('[data-testid="common_modal_close"]')).toBeNull();
    expect(document.querySelector('[data-testid="common_modal_backdrop"]')).not.toBeNull();
    wrapper.unmount();
  });
});
