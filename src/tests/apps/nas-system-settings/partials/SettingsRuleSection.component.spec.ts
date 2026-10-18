import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import SettingsRuleSection from '@/apps/nas-system-settings/components/SettingsRuleSection.vue';
import { setLocale } from '@/i18n';

function mountSection(props: Record<string, unknown> = {}) {
  return mount(SettingsRuleSection, {
    attachTo: document.body,
    props: { title: 'Rules', formTitle: 'New rule', testid: 'rule', ...props },
    slots: {
      fields: '<input data-testid="field" /><button type="button" data-testid="other">x</button>',
      default: '<p data-testid="list">list</p>',
    },
  });
}

beforeEach(() => setLocale('en'));

describe('SettingsRuleSection', () => {
  it('shows the list and keeps the fields closed until Add is pressed', async () => {
    const wrapper = mountSection();
    expect(wrapper.get('h2').text()).toBe('Rules');
    expect(wrapper.find('[data-testid="list"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="rule_form"]').exists()).toBe(false);

    await wrapper.get('[data-testid="rule_open"]').trigger('click');
    expect(wrapper.get('[data-testid="rule_form"]').attributes('aria-label')).toBe('New rule');
    expect(wrapper.emitted('update:open')?.[0]).toEqual([true]);
    wrapper.unmount();
  });

  it('moves the focus to the first field when it opens', async () => {
    const wrapper = mountSection();
    await wrapper.get('[data-testid="rule_open"]').trigger('click');
    await wrapper.vm.$nextTick();

    expect(document.activeElement).toBe(wrapper.get('[data-testid="field"]').element);
    wrapper.unmount();
  });

  it('adds only when the rule is ready and nothing else is being saved', async () => {
    const wrapper = mountSection({ open: true, ready: false });
    expect(wrapper.get('[data-testid="rule_add"]').attributes('disabled')).toBeDefined();

    await wrapper.setProps({ ready: true, busy: true });
    expect(wrapper.get('[data-testid="rule_add"]').attributes('disabled')).toBeDefined();

    await wrapper.setProps({ busy: false });
    await wrapper.get('[data-testid="rule_add"]').trigger('click');
    expect(wrapper.emitted('submit')).toHaveLength(1);
    wrapper.unmount();
  });

  it('submits on Enter from a field, not from a button', async () => {
    const wrapper = mountSection({ open: true });

    await wrapper.get('[data-testid="other"]').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('submit')).toBeUndefined();

    await wrapper.get('[data-testid="field"]').trigger('keydown', { key: 'Enter' });
    expect(wrapper.emitted('submit')).toHaveLength(1);
    wrapper.unmount();
  });

  it('closes on cancel', async () => {
    const wrapper = mountSection({ open: true });
    await wrapper.get('[data-testid="rule_cancel"]').trigger('click');

    expect(wrapper.emitted('update:open')?.at(-1)).toEqual([false]);
    wrapper.unmount();
  });
});
