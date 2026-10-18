import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, expect, it, vi } from 'vitest';
import RecoverySettingsTab from '@/apps/nas-system-settings/components/tabs/RecoverySettingsTab.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setLocale('it');
  vi.mocked(request).mockReset().mockResolvedValue({ available: true, reason: null });
});

it('offers the export and the import, then the factory reset', async () => {
  const wrapper = mount(RecoverySettingsTab);
  await flushPromises();

  const sections = wrapper
    .findAll('section[aria-label]')
    .map((section) => section.attributes('aria-label'));
  expect(sections).toEqual(['Esporta e importa la configurazione', 'Ripristino']);
  expect(wrapper.get('h1').text()).toBe('Ripristino e reset');
  expect(
    wrapper.get('[data-testid="factory_reset_button"]').attributes('disabled'),
  ).toBeUndefined();
});
