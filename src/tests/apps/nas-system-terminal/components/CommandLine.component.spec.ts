import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, expect, it, vi } from 'vitest';
import CommandLine from '@/apps/nas-system-terminal/components/CommandLine.vue';
import { useTerminalStore } from '@/apps/nas-system-terminal/store';
import { setLocale } from '@/i18n';

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
});

function ready() {
  const terminal = useTerminalStore();
  terminal.connection = 'open';
  terminal.session = 'session-1';
  terminal.cwd = '/var/lib';
  const run = vi.spyOn(terminal, 'run').mockResolvedValue(undefined);
  return { terminal, run, wrapper: mount(CommandLine) };
}

it('shows the directory as the prompt and runs what is typed on Enter', async () => {
  const { run, wrapper } = ready();

  expect(wrapper.get('label[for="terminal_command"]').text()).toBe('/var/lib $');
  await wrapper.get('[data-testid="terminal_command"]').setValue('df -h');
  await wrapper.get('form').trigger('submit');

  expect(run).toHaveBeenCalledWith('df -h');
  expect((wrapper.get('input').element as HTMLInputElement).value).toBe('');
});

it('walks the history with the arrows, back to an empty line', async () => {
  const { terminal, wrapper } = ready();
  terminal.history = ['ls', 'pwd'];
  await wrapper.vm.$nextTick();
  const input = wrapper.get('input');

  await input.trigger('keydown', { key: 'ArrowUp' });
  expect((input.element as HTMLInputElement).value).toBe('pwd');
  await input.trigger('keydown', { key: 'ArrowUp' });
  expect((input.element as HTMLInputElement).value).toBe('ls');
  await input.trigger('keydown', { key: 'ArrowDown' });
  await input.trigger('keydown', { key: 'ArrowDown' });
  expect((input.element as HTMLInputElement).value).toBe('');
});

it('offers to interrupt while a command runs, and nothing to type without a shell', async () => {
  const { terminal, wrapper } = ready();
  const interrupt = vi.spyOn(terminal, 'interrupt').mockImplementation(() => undefined);
  terminal.running = true;
  await wrapper.vm.$nextTick();

  await wrapper.get('[data-testid="terminal_interrupt"]').trigger('click');
  expect(interrupt).toHaveBeenCalled();

  terminal.running = false;
  terminal.connection = 'closed';
  await wrapper.vm.$nextTick();
  expect(wrapper.get('input').attributes('disabled')).toBeDefined();
  expect(wrapper.get('[data-testid="terminal_run"]').attributes('disabled')).toBeDefined();
});
