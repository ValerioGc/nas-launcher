import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import PowerPlanningTab from '@/apps/nas-system-settings/components/tabs/PowerPlanningTab.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

function backend(simulated: boolean, onSave?: () => unknown, enabled = true) {
  let saved: unknown = { enabled, rules: [], screenOffMinutes: 10 };
  vi.mocked(request)
    .mockReset()
    .mockImplementation(async (path, options) => {
      if (path === '/system/power') {
        return { simulated };
      }

      if (options?.method === 'PUT') {
        if (onSave) {
          return onSave();
        }
        saved = JSON.parse(String(options.body));
        return saved;
      }

      return saved;
    });
}
beforeEach(() => {
  setLocale('it');
});

describe('power schedule', () => {
  it('does not add a row unless the backend accepts it', async () => {
    backend(true, () => {
      throw new Error();
    });

    const wrapper = mount(PowerPlanningTab);
    await flushPromises();
    await wrapper.get('[data-testid="power_rule_open"]').trigger('click');
    await wrapper.get('[data-testid="power_rule_add"]').trigger('click');
    await flushPromises();
    expect(request).toHaveBeenCalledWith(
      '/settings/power',
      expect.objectContaining({ method: 'PUT' }),
    );
    expect(wrapper.findAll('tbody tr')).toHaveLength(0);
    expect(wrapper.find('[data-testid="power_rule_form"]').exists()).toBe(true);
    expect(wrapper.get('[role="alert"]').text()).toContain('non riuscita');
    wrapper.unmount();
  });

  it('warns that the rules only pretend to act, and stops warning when they do act', async () => {
    backend(true);
    const simulated = mount(PowerPlanningTab);
    await flushPromises();
    expect(simulated.get('[data-testid="power_mode"]').text()).toContain('simulata');
    simulated.unmount();
    backend(false);
    const real = mount(PowerPlanningTab);
    await flushPromises();
    expect(real.get('[data-testid="power_mode"]').text()).toContain('vengono eseguiti realmente');
    real.unmount();
  });

  it('adds a rule and sends the whole section, not just the new row', async () => {
    backend(false);
    const wrapper = mount(PowerPlanningTab);
    await flushPromises();
    expect(wrapper.get('[data-testid="power_rules_empty"]').text()).toBe(
      'Nessuna regola impostata',
    );
    expect(wrapper.find('[data-testid="power_rule_form"]').exists()).toBe(false);

    await wrapper.get('[data-testid="power_rule_open"]').trigger('click');
    await wrapper.get('[data-testid="power_rule_day"]').setValue('weekdays');
    await wrapper.get('[data-testid="power_rule_time"]').setValue('07:30');
    await wrapper.get('[data-testid="power_rule_action_start"]').trigger('click');
    await wrapper.get('[data-testid="power_rule_add"]').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/settings/power', {
      method: 'PUT',
      body: JSON.stringify({
        enabled: true,
        rules: [{ day: 'weekdays', time: '07:30', action: 'start', active: true }],
        screenOffMinutes: 10,
      }),
    });
    expect(wrapper.findAll('tbody tr')).toHaveLength(1);
    expect(wrapper.find('[data-testid="power_rule_form"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="power_saved"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('removes the row that was asked for and keeps the others', async () => {
    const rules = [
      { day: 'monday', time: '07:00', action: 'start', active: true },
      { day: 'friday', time: '23:00', action: 'shutdown', active: true },
    ];
    let saved: unknown = { enabled: true, rules };
    vi.mocked(request)
      .mockReset()
      .mockImplementation(async (path, options) => {
        if (path === '/system/power') {
          return { simulated: false };
        }
        if (options?.method === 'PUT') {
          saved = JSON.parse(String(options.body));
          return saved;
        }
        return saved;
      });
    const wrapper = mount(PowerPlanningTab);
    await flushPromises();
    expect(wrapper.findAll('tbody tr')).toHaveLength(2);

    await wrapper.findAll('tbody tr')[0]?.get('button').trigger('click');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/settings/power', {
      method: 'PUT',
      body: JSON.stringify({ enabled: true, rules: [rules[1]] }),
    });
    expect(wrapper.findAll('tbody tr')).toHaveLength(1);
    expect(wrapper.get('tbody tr').text()).toContain('23:00');
    wrapper.unmount();
  });

  it('switches a single rule off and saves the section', async () => {
    const rules = [{ day: 'monday', time: '07:00', action: 'start', active: true }];
    let saved: unknown = { enabled: true, rules };
    vi.mocked(request)
      .mockReset()
      .mockImplementation(async (path, options) => {
        if (path === '/system/power') {
          return { simulated: false };
        }
        if (options?.method === 'PUT') {
          saved = JSON.parse(String(options.body));
        }
        return saved;
      });
    const wrapper = mount(PowerPlanningTab);
    await flushPromises();

    await wrapper.get('tbody input[type="checkbox"]').setValue(false);
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/settings/power', {
      method: 'PUT',
      body: JSON.stringify({ enabled: true, rules: [{ ...rules[0], active: false }] }),
    });
    wrapper.unmount();
  });

  it('saves as soon as the schedule is switched on', async () => {
    backend(false, undefined, false);
    const wrapper = mount(PowerPlanningTab);
    await flushPromises();

    await wrapper.get('[data-testid="power_enabled_input"]').setValue(true);
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/settings/power', {
      method: 'PUT',
      body: JSON.stringify({ enabled: true, rules: [], screenOffMinutes: 10 }),
    });
    wrapper.unmount();
  });

  it('says nothing about the mode it could not read', async () => {
    vi.mocked(request)
      .mockReset()
      .mockImplementation(async (path) => {
        if (path === '/system/power') {
          throw new Error('down');
        }
        return { enabled: false, rules: [] };
      });
    const wrapper = mount(PowerPlanningTab);
    await flushPromises();

    expect(wrapper.find('[data-testid="power_mode"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('saves the minutes of the connected screen only within their bounds', async () => {
    backend(true);
    const wrapper = mount(PowerPlanningTab);
    await flushPromises();
    const minutes = wrapper.get('[data-testid="power_screen_minutes"]');
    expect((minutes.element as HTMLInputElement).value).toBe('10');

    await minutes.setValue('0');
    await minutes.trigger('change');
    await flushPromises();
    expect(request).not.toHaveBeenCalledWith(
      '/settings/power',
      expect.objectContaining({ method: 'PUT' }),
    );

    await minutes.setValue('25');
    await minutes.trigger('change');
    await flushPromises();
    expect(request).toHaveBeenCalledWith(
      '/settings/power',
      expect.objectContaining({
        method: 'PUT',
        body: expect.stringContaining('"screenOffMinutes":25'),
      }),
    );
    wrapper.unmount();
  });

  it('adds no rule while the rules are off, and fades their table', async () => {
    backend(false, undefined, false);
    const wrapper = mount(PowerPlanningTab);
    await flushPromises();
    expect(wrapper.get('[data-testid="power_rule_open"]').attributes('disabled')).toBeDefined();

    await wrapper.get('[data-testid="power_enabled_input"]').setValue(true);
    await flushPromises();
    expect(wrapper.get('[data-testid="power_rule_open"]').attributes('disabled')).toBeUndefined();
    await wrapper.get('[data-testid="power_rule_open"]').trigger('click');
    expect(wrapper.find('[data-testid="power_rule_form"]').exists()).toBe(true);

    // Turned off again, the form closes.
    await wrapper.get('[data-testid="power_enabled_input"]').setValue(false);
    await flushPromises();
    expect(wrapper.find('[data-testid="power_rule_form"]').exists()).toBe(false);
    wrapper.unmount();
  });
});
