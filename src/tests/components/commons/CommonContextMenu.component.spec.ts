import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import CommonContextMenu from '@/components/commons/CommonContextMenu.vue';

const mounted: { wrapper: VueWrapper; opener: HTMLElement }[] = [];

async function openMenu() {
  const opener = document.createElement('button');
  document.body.append(opener);
  opener.focus();
  const wrapper = mount(CommonContextMenu, {
    attachTo: document.body,
    props: {
      modelValue: false,
      'onUpdate:modelValue': (value: boolean) => wrapper.setProps({ modelValue: value }),
      x: 40,
      y: 60,
      label: 'Shortcut',
    },
    slots: { default: '<button role="menuitem">Remove</button>' },
  });
  mounted.push({ wrapper, opener });
  await wrapper.setProps({ modelValue: true });
  await flushPromises();
  return { wrapper, opener };
}

afterEach(() => {
  for (const { wrapper, opener } of mounted.splice(0)) {
    wrapper.unmount();
    opener.remove();
  }
});

describe('CommonContextMenu', () => {
  it('opens where it was asked, labelled, with the focus on its first item', async () => {
    await openMenu();

    const menu = document.querySelector<HTMLElement>('[role="menu"]');
    expect(menu?.getAttribute('aria-label')).toBe('Shortcut');
    expect(menu?.style.left).toBe('40px');
    expect(menu?.style.top).toBe('60px');
    expect(document.activeElement?.textContent).toBe('Remove');
  });

  it('closes on Escape and gives the focus back', async () => {
    const { wrapper, opener } = await openMenu();

    document.activeElement?.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }),
    );
    await flushPromises();

    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
    expect(document.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it('closes on a click outside it', async () => {
    const { wrapper, opener } = await openMenu();

    opener.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    await nextTick();

    expect(wrapper.emitted('update:modelValue')?.at(-1)).toEqual([false]);
  });
});
