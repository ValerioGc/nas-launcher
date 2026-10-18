import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import ConfigAppPicker from '@/apps/nas-system-settings/components/ConfigAppPicker.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import { useAppsStore } from '@/stores/appsStore';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(async () => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.mocked(request)
    .mockReset()
    .mockResolvedValueOnce([
      { id: 'nas-file-manager', name: 'Explorer', info: '', system: false },
      { id: 'nas-system-terminal', name: 'Terminale', info: '', system: true },
      { id: 'nas-app-manager', name: 'App Manager', info: '', system: true },
    ])
    .mockResolvedValueOnce([]);
  await useAppsStore().fetchInstalled();
});

describe('ConfigAppPicker', () => {
  it('lists the applications with settings first, and hands back the ones checked', async () => {
    const wrapper = mount(ConfigAppPicker, {
      props: {
        modelValue: [],
        exportable: ['nas-system-terminal', 'nas-app-manager'],
        'onUpdate:modelValue': (value: string[]) => wrapper.setProps({ modelValue: value }),
      },
    });
    await flushPromises();

    const names = wrapper.findAll('.config_app_name').map((name) => name.text());
    expect(names).toEqual(['Gestore applicazioni', 'Terminale', 'Explorer']);
    expect(wrapper.get('[data-testid="config_app_nas-file-manager"]').attributes()).toHaveProperty(
      'disabled',
    );

    await wrapper.get('[data-testid="config_app_nas-system-terminal"]').setValue(true);
    await wrapper.get('[data-testid="config_app_nas-app-manager"]').setValue(true);
    expect(wrapper.props('modelValue')).toEqual(['nas-system-terminal', 'nas-app-manager']);

    await wrapper.get('[data-testid="config_app_nas-system-terminal"]').setValue(false);
    expect(wrapper.props('modelValue')).toEqual(['nas-app-manager']);
  });

  it('locks every choice while the file is being made', () => {
    const wrapper = mount(ConfigAppPicker, {
      props: { modelValue: [], exportable: [], disabled: true },
    });

    expect(wrapper.get('fieldset').attributes()).toHaveProperty('disabled');
  });
});
