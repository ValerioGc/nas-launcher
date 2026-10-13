import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import WidgetPanel from '@/widgets/WidgetPanel.vue';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

function mountPanel() {
  return mount(WidgetPanel, {
    props: { title: 'Processes', testid: 'panel' },
    slots: { default: '<p data-testid="content">content</p>' },
  });
}

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('en');
  vi.mocked(request).mockReset().mockResolvedValue({ pollingSeconds: 10 });
});

describe('WidgetPanel', () => {
  it('shows its content under a named heading and reads the refresh interval once', async () => {
    const wrapper = mountPanel();
    await flushPromises();

    expect(wrapper.get('[data-testid="panel"]').attributes('aria-label')).toBe('Processes');
    expect(wrapper.find('[data-testid="content"]').exists()).toBe(true);
    expect(request).toHaveBeenCalledWith('/settings/widgets');
  });

  it('asks to be closed', async () => {
    const wrapper = mountPanel();
    await wrapper.get('[data-testid="widget_close"]').trigger('click');

    expect(wrapper.emitted('close')).toHaveLength(1);
  });

  it('saves a new refresh interval without a local success banner', async () => {
    const wrapper = mountPanel();
    await flushPromises();
    await wrapper.get('[data-testid="widget_settings_toggle"]').trigger('click');
    expect(wrapper.find('[data-testid="content"]').exists()).toBe(false);
    expect((wrapper.get('[data-testid="widget_polling"]').element as HTMLInputElement).value).toBe(
      '10',
    );

    vi.mocked(request).mockResolvedValueOnce({ pollingSeconds: 30 });
    await wrapper.get('[data-testid="widget_polling"]').setValue(30);
    await wrapper.get('form').trigger('submit');
    await flushPromises();

    expect(request).toHaveBeenCalledWith('/settings/widgets', {
      method: 'PUT',
      body: JSON.stringify({ pollingSeconds: 30 }),
    });
    expect(wrapper.find('[data-testid="save_footer_saved"]').exists()).toBe(false);
  });

  it('says so when the interval cannot be saved', async () => {
    const wrapper = mountPanel();
    await flushPromises();
    await wrapper.get('[data-testid="widget_settings_toggle"]').trigger('click');

    vi.mocked(request).mockRejectedValueOnce(new Error('down'));
    await wrapper.get('form').trigger('submit');
    await flushPromises();

    expect(wrapper.get('[role="alert"]').text()).not.toBe('');
    expect(wrapper.find('[data-testid="save_footer_saved"]').exists()).toBe(false);
  });
});
