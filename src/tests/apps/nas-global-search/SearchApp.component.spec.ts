import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import SearchApp from '@/apps/nas-global-search/SearchApp.vue';
import { NoSearchProviderError, searchFiles } from '@/apps/nas-global-search/api';
import { setLocale } from '@/i18n';
import { useAppsStore } from '@/stores/appsStore';
import { useShellStore } from '@/stores/shellStore';
import type { FileSearch, SearchResult } from '@/apps/nas-global-search/types';

vi.mock('@/apps/nas-global-search/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/apps/nas-global-search/api')>()),
  searchFiles: vi.fn(),
}));

const report: SearchResult = {
  id: 'nas-file-manager:1',
  name: 'Old report',
  path: '/old',
  type: 'document',
  extension: 'pdf',
  size: 10,
  modified: '',
  app_id: 'nas-file-manager',
  target: '?path=%2Fold',
};

function mountSearch() {
  const apps = useAppsStore();
  apps.installed = [
    { id: 'nas-system-settings', name: 'Settings', info: '', system: true },
    { id: 'nas-file-manager', name: 'Explorer', info: '', system: false },
  ];
  return mount(SearchApp, {
    props: { modelValue: true },
    global: { stubs: { teleport: true } },
  });
}

beforeEach(() => {
  setLocale('en');
  setActivePinia(createPinia());
  vi.useFakeTimers();
  vi.mocked(searchFiles).mockReset().mockResolvedValue({ results: [], failed: [] });
});

afterEach(() => vi.useRealTimers());

describe('search lifecycle', () => {
  it('debounces input and ignores old results when the query is cleared', async () => {
    let finish: (value: FileSearch) => void = () => {};
    vi.mocked(searchFiles).mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );

    const wrapper = mountSearch();
    expect(wrapper.get('dialog').attributes('aria-label')).toBe('Global search');
    await wrapper.get('[data-testid="search_query"]').setValue('report');
    expect(searchFiles).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(250);
    expect(searchFiles).toHaveBeenCalledWith(new URLSearchParams('q=report'));
    await wrapper.get('[data-testid="search_query"]').setValue('');
    finish({ results: [report], failed: [] });
    await flushPromises();
    expect(wrapper.text()).not.toContain('Old report');
    wrapper.unmount();
  });

  it('shows applications and settings at once and the files when they arrive', async () => {
    vi.mocked(searchFiles).mockResolvedValue({ results: [report], failed: ['photos'] });
    const wrapper = mountSearch();
    await wrapper.get('[data-testid="search_query"]').setValue('s');
    expect(wrapper.find('[data-testid="search_group_app"]').text()).toContain('Settings');
    expect(wrapper.find('[data-testid="search_group_setting"]').exists()).toBe(true);
    await vi.advanceTimersByTimeAsync(250);
    expect(wrapper.find('[data-testid="search_group_file"]').text()).toContain('Old report');
    expect(wrapper.get('[data-testid="search_warning"]').text()).toContain('photos');
    wrapper.unmount();
  });

  it('searches no file on the applications and settings tabs', async () => {
    const wrapper = mountSearch();
    await wrapper.get('[data-testid="search_scope_apps"]').trigger('click');
    await wrapper.get('[data-testid="search_query"]').setValue('explorer');
    await vi.advanceTimersByTimeAsync(250);
    expect(searchFiles).not.toHaveBeenCalled();
    expect(wrapper.find('[data-testid="search_origin"]').exists()).toBe(true);
    await wrapper.get('[data-testid="search_origin"]').setValue('system');
    expect(wrapper.find('[data-testid="search_empty"]').exists()).toBe(true);

    await wrapper.get('[data-testid="search_scope_settings"]').trigger('click');
    expect(wrapper.find('[data-testid="search_section"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="search_type"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('filters the files through the type and the extension', async () => {
    const wrapper = mountSearch();
    await wrapper.get('[data-testid="search_scope_files"]').trigger('click');
    const type = wrapper.get('select[data-testid="search_type"]');
    expect(type.findAll('option')).toHaveLength(7);
    await wrapper.get('[data-testid="search_query"]').setValue('report');
    await type.setValue('document');
    await wrapper.get('[data-testid="search_extension"]').setValue('.PDF');
    await vi.advanceTimersByTimeAsync(250);
    expect(searchFiles).toHaveBeenLastCalledWith(
      new URLSearchParams('q=report&type=document&ext=pdf'),
    );
    await wrapper.get('[data-testid="search_reset_filters"]').trigger('click');
    expect((type.element as HTMLSelectElement).value).toBe('');
    expect((wrapper.get('[data-testid="search_query"]').element as HTMLInputElement).value).toBe(
      'report',
    );
    expect(
      wrapper.get('[data-testid="search_reset_filters"]').attributes('disabled'),
    ).toBeDefined();
    await vi.advanceTimersByTimeAsync(250);
    expect(searchFiles).toHaveBeenLastCalledWith(new URLSearchParams('q=report'));
    expect(searchFiles).toHaveBeenCalledTimes(2);
    wrapper.unmount();
  });

  it('explains on the files tab that no application searches files', async () => {
    vi.mocked(searchFiles).mockRejectedValue(new NoSearchProviderError());
    const wrapper = mountSearch();
    await wrapper.get('[data-testid="search_scope_files"]').trigger('click');
    await wrapper.get('[data-testid="search_query"]').setValue('report');
    await vi.advanceTimersByTimeAsync(250);
    expect(wrapper.get('[role="alert"]').text()).toContain('No installed application');
    expect(wrapper.get('[data-testid="search_results"]').attributes('aria-busy')).toBe('false');
    wrapper.unmount();
  });

  it('opens a settings entry on its section and closes the search', async () => {
    const wrapper = mountSearch();
    const shell = useShellStore();
    await wrapper.get('[data-testid="search_query"]').setValue('scheduled');
    await wrapper.get('[data-testid="search_entry_setting:power:powerRules"]').trigger('click');
    expect(shell.activeId).toBe('nas-system-settings');
    expect(shell.launchQuery).toBe('power');
    expect(wrapper.emitted('update:modelValue')).toEqual([[false]]);
    wrapper.unmount();
  });
});

describe('search by scope', () => {
  it('puts the files right after everything, and says what each scope looks through', async () => {
    const wrapper = mountSearch();
    const query = wrapper.get<HTMLInputElement>('[data-testid="search_query"]');
    const tabs = wrapper.findAll('[data-testid^="search_scope_"]').map((tab) => tab.text());
    expect(tabs).toEqual(['Everything', 'Files', 'Applications', 'Settings']);

    const prompt = () => wrapper.get('[data-testid="search_idle"]').text();
    expect(prompt()).toBe('Search applications, settings and files by name.');
    expect(query.attributes('placeholder')).toBe('Search apps, settings and files');

    await wrapper.get('[data-testid="search_scope_files"]').trigger('click');
    expect(prompt()).toBe('Search the files and folders of the NAS by name.');
    expect(query.attributes('placeholder')).toBe('Search files and folders');
    await wrapper.get('[data-testid="search_scope_apps"]').trigger('click');
    expect(prompt()).toBe('Search the applications by name.');
    await wrapper.get('[data-testid="search_scope_settings"]').trigger('click');
    expect(prompt()).toBe('Search the settings by name.');
    expect(query.attributes('placeholder')).toBe('Search settings');
    wrapper.unmount();
  });

  it('marks folders and files apart, in the list and in the table', async () => {
    const folder: SearchResult = {
      ...report,
      id: 'nas-file-manager:2',
      name: 'Photos',
      type: 'folder',
    };
    vi.mocked(searchFiles).mockResolvedValue({ results: [report, folder], failed: [] });
    const wrapper = mountSearch();
    await wrapper.get('[data-testid="search_query"]').setValue('o');
    await vi.advanceTimersByTimeAsync(250);

    const icon = (selector: string) =>
      wrapper.get(`${selector} [data-testid="common_icon"]`).attributes('style');
    const fileIcon = icon('[data-testid="search_entry_file:nas-file-manager:1"]');
    const folderIcon = icon('[data-testid="search_entry_file:nas-file-manager:2"]');
    expect(fileIcon).not.toBe(folderIcon);

    await wrapper.get('[data-testid="search_scope_files"]').trigger('click');
    await vi.advanceTimersByTimeAsync(250);
    expect(icon('[data-testid="search_result_nas-file-manager:1"]')).toBe(fileIcon);
    expect(icon('[data-testid="search_result_nas-file-manager:2"]')).toBe(folderIcon);
    wrapper.unmount();
  });
});

describe('search failures', () => {
  it('says the search service is unavailable when the files cannot be searched', async () => {
    vi.mocked(searchFiles).mockRejectedValue(new Error('down'));
    const wrapper = mountSearch();
    await wrapper.get('[data-testid="search_scope_files"]').trigger('click');
    await wrapper.get('[data-testid="search_query"]').setValue('report');
    await vi.advanceTimersByTimeAsync(250);

    expect(wrapper.get('[data-testid="search_error"]').text()).toContain(
      'Search service unavailable',
    );
    wrapper.unmount();
  });
});
