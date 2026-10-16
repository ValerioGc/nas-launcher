import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import SearchResults from '@/apps/nas-global-search/components/SearchResults.vue';
import { fileEntry } from '@/apps/nas-global-search/entries';
import type { SearchEntry, SearchResult } from '@/apps/nas-global-search/types';
import { setLocale } from '@/i18n';

const results: SearchResult[] = [
  {
    id: '1',
    name: 'File 10',
    path: '/z',
    type: 'folder',
    extension: '',
    size: 2048,
    modified: '2026-01-01T01:00:00+02:00',
    app_id: 'nas-disk-manager',
    target: '',
  },
  {
    id: '2',
    name: 'File 2',
    path: '/a',
    type: 'audio',
    extension: 'mp3',
    size: 900,
    modified: '2025-12-31T23:30:00Z',
    app_id: 'nas-disk-manager',
    target: '',
  },
];

const props = {
  results,
  entries: [],
  table: true,
  loading: false,
  errorMessage: '',
  hasQuery: true,
  scope: 'all' as const,
};

beforeEach(() => setLocale('it'));

describe('search results table', () => {
  it('sorts all five fields with numeric size, chronological dates and natural names', async () => {
    const wrapper = mount(SearchResults, { props });
    const order = () =>
      wrapper.findAll('tbody button').map((button) => button.attributes('data-testid'));
    expect(order()).toEqual(['search_result_2', 'search_result_1']);
    expect(wrapper.findAll('th')).toHaveLength(5);
    for (const [field, first] of [
      ['path', '2'],
      ['type', '2'],
      ['size', '2'],
      ['modified', '1'],
      ['name', '2'],
    ]) {
      const header = wrapper.get(`[data-testid="search_sort_${field}"]`);
      if (field === 'name') {
        await wrapper.get('[data-testid="search_sort_path"]').trigger('click');
      }
      await header.trigger('click');
      expect(order()[0]).toBe(`search_result_${first}`);
      expect(header.element.closest('th')?.getAttribute('aria-sort')).toBe('ascending');
      await header.trigger('click');
      expect(order()[0]).not.toBe(`search_result_${first}`);
      expect(header.element.closest('th')?.getAttribute('aria-sort')).toBe('descending');
    }
    expect(results.map((result) => result.id)).toEqual(['1', '2']);
    expect(wrapper.text()).toContain('2.0 kB');
    await wrapper.get('[data-testid="search_result_1"]').trigger('click');
    expect(wrapper.emitted('open')?.[0]).toEqual([fileEntry(results[0]!)]);
    wrapper.unmount();
  });

  it.each([
    ['idle', { hasQuery: false }, 'status'],
    ['loading', { loading: true }, 'status'],
    ['empty', {}, 'status'],
    ['error', { errorMessage: 'Offline' }, 'alert'],
  ] as const)('renders what fits the %s state', (kind, overrides, role) => {
    const wrapper = mount(SearchResults, { props: { ...props, results: [], ...overrides } });
    const state = wrapper.get(`[data-testid="search_${kind}"]`);
    expect(state.attributes('role')).toBe(role);
    if (kind === 'loading') {
      // Only the spinner, large, with its name for screen readers.
      expect(state.find('.common_spinner_huge').exists()).toBe(true);
      expect(state.find('p').exists()).toBe(false);
    } else {
      expect(state.find('[data-testid="common_icon"]').exists()).toBe(true);
      expect(state.get('p').text()).not.toBe('');
    }
    expect(wrapper.find('table').exists()).toBe(false);
    expect(wrapper.attributes('aria-busy')).toBe(kind === 'loading' ? 'true' : 'false');
    wrapper.unmount();
  });
});

describe('search results list', () => {
  const entries: SearchEntry[] = [
    { key: 'app:photos', kind: 'app', title: 'Photos', detail: 'Pictures', app: 'photos' },
    {
      key: 'setting:access:signInLock',
      kind: 'setting',
      title: 'Lock',
      detail: 'Settings / Access',
      app: 'nas-system-settings',
      query: 'access',
    },
  ];

  it('groups the entries by kind and opens the one chosen', async () => {
    const wrapper = mount(SearchResults, { props: { ...props, table: false, entries } });
    expect(wrapper.find('table').exists()).toBe(false);
    expect(wrapper.find('[data-testid="search_group_app"]').text()).toContain('Photos');
    expect(wrapper.find('[data-testid="search_group_setting"]').text()).toContain('Lock');
    expect(wrapper.find('[data-testid="search_group_file"]').exists()).toBe(false);
    await wrapper.get('[data-testid="search_entry_setting:access:signInLock"]').trigger('click');
    expect(wrapper.emitted('open')).toEqual([[entries[1]]]);
    wrapper.unmount();
  });

  it('is empty when no entry matches, whatever the files are', () => {
    const wrapper = mount(SearchResults, { props: { ...props, table: false } });
    expect(wrapper.find('[data-testid="search_empty"]').exists()).toBe(true);
    wrapper.unmount();
  });
});

describe('search result messages', () => {
  it('says only that there are no results, and that the service is unavailable when it fails', () => {
    const empty = mount(SearchResults, { props: { ...props, results: [] } });
    expect(empty.get('[data-testid="search_empty"] p').text()).toBe('Nessun risultato');
    empty.unmount();
  });
});
