import { mount } from '@vue/test-utils';
import { defineComponent, h } from 'vue';
import { beforeEach, describe, expect, it } from 'vitest';
import AppSections from '@/apps/components/AppSections.vue';
import { setLocale } from '@/i18n';

const panel = (text: string) =>
  defineComponent({ setup: () => () => h('p', { 'data-testid': `panel_${text}` }, text) });

const sections = [
  { id: 'general', label: 'general', component: panel('general') },
  { id: 'power', label: 'power', component: panel('power') },
];

function open() {
  return mount(AppSections, { props: { sections, label: 'Impostazioni' } });
}

beforeEach(() => setLocale('it'));

describe('sections of an application', () => {
  it('starts on the first section and marks it as the active one', () => {
    const wrapper = open();
    const buttons = wrapper.findAll('nav button');

    expect(buttons.map((button) => button.text())).toEqual(['Generali', 'Alimentazione']);
    expect(buttons[0]?.attributes('aria-pressed')).toBe('true');
    expect(wrapper.find('[data-testid="panel_general"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="panel_power"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('shows the section that was chosen', async () => {
    const wrapper = open();
    await wrapper.findAll('nav button')[1]?.trigger('click');

    expect(wrapper.find('[data-testid="panel_power"]').exists()).toBe(true);
    expect(wrapper.findAll('nav button')[1]?.attributes('aria-pressed')).toBe('true');
    wrapper.unmount();
  });

  it('names its own navigation', () => {
    const wrapper = open();

    expect(wrapper.get('nav').attributes('aria-label')).toBe('Impostazioni');
    wrapper.unmount();
  });

  it('shows nothing at all when there is no section', () => {
    const wrapper = mount(AppSections, { props: { sections: [], label: 'Vuoto' } });

    expect(wrapper.findAll('nav button')).toHaveLength(0);
    expect(wrapper.get('.app_sections_content').text()).toBe('');
    wrapper.unmount();
  });
});
