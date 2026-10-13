import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import OutputPanel from '@/apps/nas-system-terminal/components/OutputPanel.vue';
import type { TerminalEntry } from '@/apps/nas-system-terminal/types';

function panel(lines: TerminalEntry[]) {
  return mount(OutputPanel, {
    props: { lines, received: lines.length, waiting: 'Nothing yet.', label: 'Log', testid: 'out' },
  });
}

it('says what it waits for until there is a line, and names itself', () => {
  const wrapper = panel([]);

  expect(wrapper.text()).toBe('Nothing yet.');
  expect(wrapper.get('[role="log"]').attributes('aria-label')).toBe('Log');
  expect(wrapper.find('[data-testid="out"]').exists()).toBe(true);
});

it('shows every line as text, markup included, marked by what it is', () => {
  const wrapper = panel([
    { kind: 'input', text: '/ $ cat page.html' },
    { kind: 'output', text: '<img src=x onerror=alert(1)>' },
    { kind: 'status', text: 'Uscito con codice 1.' },
  ]);
  const lines = wrapper.findAll('.terminal_line');

  expect(wrapper.find('img').exists()).toBe(false);
  expect(lines.map((line) => line.text())).toEqual([
    '/ $ cat page.html',
    '<img src=x onerror=alert(1)>',
    'Uscito con codice 1.',
  ]);

  expect(wrapper.findAll('.terminal_output')).toHaveLength(1);
  expect(lines.map((line) => line.classes().find((name) => name !== 'terminal_line'))).toEqual([
    'terminal_line_input',
    'terminal_line_output',
    'terminal_line_status',
  ]);
});
