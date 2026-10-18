import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import FactoryReset from '@/apps/nas-system-settings/components/FactoryReset.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { ResetState } from '@/apps/nas-system-settings/types/reset';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setLocale('it');
  vi.mocked(request).mockReset();
});

afterEach(() => {
  document.body.innerHTML = '';
});

async function mountWith(state: ResetState) {
  vi.mocked(request).mockResolvedValue(state);
  const wrapper = mount(FactoryReset, { attachTo: document.body });
  await flushPromises();
  return wrapper;
}

describe('FactoryReset', () => {
  it('says the data disks are not touched, and asks for a confirmation first', async () => {
    const wrapper = await mountWith({ available: true, reason: null });

    expect(wrapper.text()).toContain('I dischi dati e i relativi file restano invariati');
    expect(wrapper.find('[data-testid="factory_reset_unavailable"]').exists()).toBe(false);
    expect(
      wrapper.get('[data-testid="factory_reset_button"] [data-testid="common_icon"]').classes(),
    ).toContain('common_icon_tinted');
    await wrapper.get('[data-testid="factory_reset_button"]').trigger('click');
    await flushPromises();

    expect(document.querySelector('[data-testid="factory_reset_form"]')).not.toBeNull();
    expect(vi.mocked(request).mock.calls).toEqual([['/system/reset']]);
    wrapper.unmount();
  });

  it.each([
    ['disabled', 'non disponibile in questa installazione'],
    ['noWizard', 'il wizard non è installato'],
    ['noOriginalState', 'non ha una copia della configurazione originale'],
  ] as const)('explains why it cannot reset: %s', async (reason, text) => {
    const wrapper = await mountWith({ available: false, reason });

    expect(wrapper.get('[data-testid="factory_reset_unavailable"]').text()).toContain(text);
    expect(
      wrapper.get('[data-testid="factory_reset_button"]').attributes('disabled'),
    ).toBeDefined();
    wrapper.unmount();
  });
});
