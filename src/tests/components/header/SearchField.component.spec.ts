import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import SearchField from '@/components/header/SearchField.vue';
import { setLocale } from '@/i18n';

beforeEach(() => {
  setLocale('it');
});

describe('header search field', () => {
  it('hands the query over trimmed and empties the field', async () => {
    const wrapper = mount(SearchField);
    const input = wrapper.get('[data-testid="header_search_input"]');
    await input.setValue('  relazione annuale  ');
    await wrapper.get('form').trigger('submit');
    expect(wrapper.emitted('search')).toEqual([['relazione annuale']]);
    expect((input.element as HTMLInputElement).value).toBe('');
    wrapper.unmount();
  });

  it('opens the search with nothing typed, which is what the narrow screen does', async () => {
    const wrapper = mount(SearchField);
    const compact = wrapper.get('[data-testid="header_search_compact"]');
    expect(compact.attributes('type')).toBe('submit');
    expect(compact.element.closest('form')).toBe(wrapper.get('form').element);
    await wrapper.get('form').trigger('submit');
    expect(wrapper.emitted('search')).toEqual([['']]);
    wrapper.unmount();
  });
  it('lets the search be reached without seeing it', () => {
    const wrapper = mount(SearchField);
    expect(wrapper.get('form').attributes('role')).toBe('search');
    expect(wrapper.get('[data-testid="header_search_input"]').attributes('aria-label')).toBe(
      'Cerca',
    );
    expect(wrapper.get('[data-testid="header_search_submit"]').attributes('aria-label')).toBe(
      'Avvia la ricerca',
    );

    wrapper.unmount();
  });
});
