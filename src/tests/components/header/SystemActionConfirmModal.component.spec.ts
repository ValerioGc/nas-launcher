import { mount, enableAutoUnmount } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import SystemActionConfirmModal from '@/components/header/SystemActionConfirmModal.vue';
import { setLocale } from '@/i18n';
import type { SystemAction } from '@/types/types';

const shutdown: SystemAction = { id: 'shutdown', icon: 'shutdown.svg' };
const signOut: SystemAction = { id: 'logout', icon: 'logout.svg' };

enableAutoUnmount(afterEach);
beforeEach(() => {
  setLocale('it');
});
afterEach(() => {
  document.body.innerHTML = '';
});

function open(props: Partial<InstanceType<typeof SystemActionConfirmModal>['$props']> = {}) {
  return mount(SystemActionConfirmModal, {
    attachTo: document.body,
    props: {
      open: true,
      selected: shutdown,
      remaining: null,
      busy: false,
      error: '',
      ...props,
    },
  });
}

function text() {
  return document.querySelector('dialog')?.textContent ?? '';
}

function button(id: string) {
  return document.querySelector<HTMLButtonElement>(`[data-testid="${id}"]`)!;
}

describe('system action confirmation modal', () => {
  it('stays out of the way until the action is chosen', () => {
    open({ open: false });
    expect(document.querySelector('dialog')).toBeNull();
  });

  it('names the action being confirmed, and says nothing else about it', () => {
    open();
    expect(text()).toContain('Spegni');
    expect(text()).toContain('Confermare');
    expect(text()).not.toContain('sessione corrente');
    expect(document.querySelector('[role="timer"]')).toBeNull();
  });

  it('says the current session is closed when signing out', () => {
    open({ selected: signOut });
    expect(text()).toContain('sessione corrente');
  });

  it('shows the remaining seconds without stealing the screen reader', () => {
    open({ remaining: 42 });
    const timer = document.querySelector('[data-testid="system_action_countdown"]');
    expect(timer?.textContent).toContain('42');
    expect(timer?.getAttribute('aria-live')).toBe('off');
  });

  it('locks both buttons and hides the close while the action is running', () => {
    open({ busy: true });
    expect(button('system_action_cancel').disabled).toBe(true);
    expect(button('system_action_confirm').disabled).toBe(true);
    expect(document.querySelector('[data-testid="common_modal_close"]')).toBeNull();
    expect(text()).toContain('Caricamento');
  });

  it('names the operations the action interrupts', () => {
    open({ tasks: ['Backup'] });
    expect(document.querySelector('[data-testid="system_action_tasks"]')?.textContent).toContain(
      'Backup',
    );
    expect(button('system_action_confirm').disabled).toBe(false);
  });

  it('refuses the action while a job of root runs', () => {
    open({ tasks: ['Backup'], jobs: ['apt', 'packages'] });
    expect(document.querySelector('[role="alert"]')?.textContent).toContain(
      'non può essere interrotta',
    );
    const jobs = document.querySelector('[data-testid="system_action_jobs"]');
    expect(jobs?.textContent).toContain('Aggiornamento dei pacchetti di sistema');
    expect(jobs?.textContent).toContain('Pacchetti di sistema di un’applicazione');
    expect(document.querySelector('[data-testid="system_action_tasks"]')).toBeNull();
    expect(button('system_action_confirm').disabled).toBe(true);
  });

  it('reports a failure as an alert', () => {
    open({ error: 'Operazione non riuscita' });
    expect(document.querySelector('[role="alert"]')?.textContent).toContain(
      'Operazione non riuscita',
    );
  });

  it('asks the parent to act: it owns neither the state nor the request', async () => {
    const wrapper = open();
    button('system_action_confirm').click();
    await wrapper.vm.$nextTick();
    expect(wrapper.emitted('execute')).toHaveLength(1);
    expect(wrapper.emitted('cancel')).toBeUndefined();
    button('system_action_cancel').click();
    await wrapper.vm.$nextTick();
    expect(wrapper.emitted('cancel')).toHaveLength(1);
    expect(document.querySelector('dialog')).not.toBeNull();
  });

  it('turns a close from the modal itself into the same cancel', async () => {
    const wrapper = open();
    button('common_modal_close').click();
    await wrapper.vm.$nextTick();
    expect(wrapper.emitted('cancel')).toHaveLength(1);
  });
});
