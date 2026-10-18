import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import ConfigTransfer from '@/apps/nas-system-settings/components/ConfigTransfer.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request).mockReset().mockResolvedValue([]);
});

afterEach(() => {
  document.body.innerHTML = '';
});

it('says what the file carries and opens the export or the import', async () => {
  const wrapper = mount(ConfigTransfer, { attachTo: document.body });

  expect(wrapper.text()).toContain('Le impostazioni del NAS e delle applicazioni selezionate');
  expect(wrapper.text()).toContain('Nome del NAS, rete, account e token restano esclusi');
  await wrapper.get('[data-testid="config_import_button"]').trigger('click');
  await flushPromises();
  expect(document.querySelector('[data-testid="config_file"]')).not.toBeNull();

  (document.querySelector('[data-testid="common_modal_close"]') as HTMLElement).click();
  await flushPromises();
  for (const id of ['config_export_button', 'config_import_button']) {
    expect(wrapper.find(`[data-testid="${id}"] [data-testid="common_icon"]`).exists()).toBe(true);
  }
  await wrapper.get('[data-testid="config_export_button"]').trigger('click');
  await flushPromises();
  expect(document.querySelector('[data-testid="config_password"]')).not.toBeNull();
  expect(document.querySelector('[data-testid="config_file"]')).toBeNull();
  expect(document.querySelector('[data-testid="config_apps"]')).not.toBeNull();
  wrapper.unmount();
});
