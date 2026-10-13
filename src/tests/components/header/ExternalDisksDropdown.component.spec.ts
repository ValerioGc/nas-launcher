import { flushPromises, mount } from '@vue/test-utils';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import ExternalDisksDropdown from '@/components/header/ExternalDisksDropdown.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import type { ExternalDisk } from '@/types/types';

vi.mock('@/api/client', () => ({ request: vi.fn() }));
const disk: ExternalDisk = { device: 'sdb', name: 'Backup', locked: false, canEject: true };
let listed: ExternalDisk[];

beforeEach(() => {
  setLocale('en');
  listed = [disk];
  vi.mocked(request)
    .mockReset()
    .mockImplementation(async (path) => {
      if (path.endsWith('/eject')) {
        listed = [];
        return undefined as never;
      }
      return listed as never;
    });
});
afterEach(() => vi.useRealTimers());

it('appears only while external disks are connected and follows hotplug', async () => {
  vi.useFakeTimers();
  listed = [];
  const wrapper = mount(ExternalDisksDropdown);
  await flushPromises();
  expect(wrapper.find('[data-testid="common_dropdown_trigger"]').exists()).toBe(false);
  listed = [disk];
  await vi.advanceTimersByTimeAsync(5000);
  expect(wrapper.find('[data-testid="common_dropdown_trigger"]').exists()).toBe(true);
  await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
  expect(wrapper.text()).toContain('Backup');
  wrapper.unmount();
});

it('asks for confirmation before ejecting and removes the disconnected disk', async () => {
  const wrapper = mount(ExternalDisksDropdown, { global: { stubs: { teleport: true } } });
  await flushPromises();
  await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
  await wrapper.get('[data-testid="header_eject_sdb"]').trigger('click');
  expect(wrapper.text()).toContain('Eject Backup?');
  expect(request).not.toHaveBeenCalledWith('/storage/disks/eject', expect.anything());
  await wrapper.get('[data-testid="header_eject_confirm"]').trigger('click');
  await flushPromises();
  expect(request).toHaveBeenCalledWith(
    '/storage/disks/eject',
    expect.objectContaining({
      method: 'POST',
      body: '{"device":"sdb"}',
    }),
  );
  expect(wrapper.find('[data-testid="common_dropdown_trigger"]').exists()).toBe(false);
  wrapper.unmount();
});

it('retains the removal lock and its hover title', async () => {
  listed = [{ ...disk, locked: true, canEject: false }];
  const wrapper = mount(ExternalDisksDropdown);
  await flushPromises();
  await wrapper.get('[data-testid="common_dropdown_trigger"]').trigger('click');
  const eject = wrapper.get('[data-testid="header_eject_sdb"]');
  expect(eject.attributes('aria-disabled')).toBe('true');
  expect(eject.attributes('title')).toContain('not removable');
  await eject.trigger('click');
  expect(wrapper.find('[data-testid="header_eject_confirm"]').exists()).toBe(false);
  wrapper.unmount();
});
