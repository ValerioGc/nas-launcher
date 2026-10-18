import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import SetupCard from '@/components/setup/SetupCard.vue';

describe('setup card', () => {
  it('shows the logo, its title, its description, its content and its actions', () => {
    const wrapper = mount(SetupCard, {
      props: { title: 'Preparazione', description: 'Il NAS installa le applicazioni.', logo: true },
      slots: { default: '<p>content</p>', actions: '<button>Accedi</button>' },
    });

    expect(wrapper.get('[data-testid="setup_card"]').attributes('aria-label')).toBe('Preparazione');
    expect(wrapper.get('header img').attributes('alt')).toBe('');
    expect(wrapper.get('h1').text()).toBe('Preparazione');
    expect(wrapper.text()).toContain('Il NAS installa le applicazioni.');
    expect(wrapper.text()).toContain('content');
    expect(wrapper.get('footer button').text()).toBe('Accedi');
  });

  it('draws no footer when the card has no action', () => {
    const wrapper = mount(SetupCard, { props: { title: 'Preparazione' } });

    expect(wrapper.find('footer').exists()).toBe(false);
    expect(wrapper.find('img').exists()).toBe(false);
    expect(wrapper.find('p').exists()).toBe(false);
  });
});
