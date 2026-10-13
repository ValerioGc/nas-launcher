import { mount } from '@vue/test-utils';
import { beforeEach, expect, it } from 'vitest';
import UpdateSchedule from '@/apps/nas-app-manager/components/UpdateSchedule.vue';
import { setLocale } from '@/i18n';
import type { ManagerSettings } from '@/apps/nas-app-manager/types';

beforeEach(() => setLocale('it'));

function schedule(settings: ManagerSettings) {
  return mount(UpdateSchedule, {
    props: {
      modelValue: settings,
      'onUpdate:modelValue': () => undefined,
    },
  });
}

it('offers the frequency only once the automatic check is on', async () => {
  const settings: ManagerSettings = {
    autoCheck: false,
    frequency: 'daily',
    checkOnOpen: false,
    updateOnOpen: false,
  };
  const wrapper = schedule(settings);
  const frequency = wrapper.get('[data-testid="app_manager_frequency"]');

  expect(frequency.attributes('disabled')).toBeDefined();
  expect(frequency.findAll('option').map((option) => option.text())).toEqual([
    'Ogni ora',
    'Ogni giorno',
    'Ogni settimana',
  ]);

  await wrapper.get('[data-testid="app_manager_auto_check_input"]').setValue(true);

  expect(settings.autoCheck).toBe(true);
  expect(frequency.attributes('disabled')).toBeUndefined();
});

it('writes the chosen frequency into the settings it was given', async () => {
  const settings: ManagerSettings = {
    autoCheck: true,
    frequency: 'daily',
    checkOnOpen: false,
    updateOnOpen: false,
  };
  const wrapper = schedule(settings);

  await wrapper.get('[data-testid="app_manager_frequency"]').setValue('weekly');

  expect(settings.frequency).toBe('weekly');
});

it('installs on opening only while it also checks on opening', async () => {
  const settings: ManagerSettings = {
    autoCheck: false,
    frequency: 'daily',
    checkOnOpen: false,
    updateOnOpen: false,
  };
  const wrapper = schedule(settings);
  const update = wrapper.get('[data-testid="app_manager_update_on_open_input"]');

  expect(update.attributes('disabled')).toBeDefined();

  await wrapper.get('[data-testid="app_manager_check_on_open_input"]').setValue(true);
  expect(update.attributes('disabled')).toBeUndefined();
  await update.setValue(true);
  expect(settings).toMatchObject({ checkOnOpen: true, updateOnOpen: true });

  await wrapper.get('[data-testid="app_manager_check_on_open_input"]').setValue(false);
  expect(settings).toMatchObject({ checkOnOpen: false, updateOnOpen: false });
  expect(update.attributes('disabled')).toBeDefined();
});
