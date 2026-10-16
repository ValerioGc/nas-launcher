import { mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it } from 'vitest';
import SystemRestartOverlay from '@/components/header/SystemRestartOverlay.vue';
import { setLocale, t } from '@/i18n';

beforeEach(() => setLocale('it'));

describe('system restart overlay', () => {
  it('shows the restart message once', () => {
    const wrapper = mount(SystemRestartOverlay, {
      props: { modelValue: true },
      attachTo: document.body,
    });
    const overlay = document.querySelector('[data-testid="system_restart_overlay"]');
    const visible = [...(overlay?.querySelectorAll('p, span:not(.visually_hidden)') ?? [])];
    expect(
      visible.filter((element) => element.textContent?.trim() === t('restartInProgress')),
    ).toHaveLength(1);
    wrapper.unmount();
  });
});
