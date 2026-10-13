import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MAX_LINES, useTerminalStore } from '@/apps/nas-system-terminal/store';
import { request } from '@/api/client';
import { setLocale } from '@/i18n';
import { FakeEventSource, settings } from './fixtures';

vi.mock('@/api/client', () => ({ request: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  setLocale('it');
  vi.stubGlobal('EventSource', FakeEventSource);
  FakeEventSource.last = null;
  vi.mocked(request).mockReset().mockResolvedValue(undefined);
});

afterEach(() => vi.unstubAllGlobals());

function openShell() {
  const terminal = useTerminalStore();
  terminal.connect();
  FakeEventSource.last?.open();
  return terminal;
}

describe('terminal store', () => {
  it('opens a shell of its own and learns its session and directory', () => {
    const terminal = useTerminalStore();

    terminal.connect();
    expect(FakeEventSource.last?.url).toBe('/api/terminal/session');
    expect(terminal.connection).toBe('connecting');

    FakeEventSource.last?.open('abc');
    expect(terminal.connection).toBe('open');
    expect(terminal.session).toBe('abc');
    expect(terminal.cwd).toBe('/');
    expect(terminal.lines).toEqual([]);
  });

  it('sends a command, shows it with its directory and waits for its end', async () => {
    const terminal = openShell();

    await terminal.run('ls /tmp');
    expect(request).toHaveBeenCalledWith('/terminal/session/session-1/input', {
      method: 'POST',
      body: JSON.stringify({ command: 'ls /tmp' }),
    });
    expect(terminal.running).toBe(true);

    FakeEventSource.last?.print('a.txt');
    FakeEventSource.last?.done(0, '/tmp');

    expect(terminal.running).toBe(false);
    expect(terminal.cwd).toBe('/tmp');
    expect(terminal.lines).toEqual([
      { kind: 'input', text: '/ $ ls /tmp' },
      { kind: 'output', text: 'a.txt' },
    ]);
  });

  it('says a command failed with its exit code, and not one that succeeded', async () => {
    const terminal = openShell();

    await terminal.run('false');
    FakeEventSource.last?.done(1, '/');

    expect(terminal.lines.at(-1)).toEqual({ kind: 'status', text: 'Uscito con codice 1.' });
  });

  it('sends nothing while a command runs or without a shell', async () => {
    const terminal = openShell();
    await terminal.run('sleep 10');
    await terminal.run('echo too early');
    terminal.disconnect();
    await terminal.run('echo nobody');

    expect(request).toHaveBeenCalledTimes(1);
  });

  it('keeps the commands for the arrows, without repeating the last one', async () => {
    const terminal = openShell();
    for (const command of ['ls', 'ls', 'pwd']) {
      await terminal.run(command);
      FakeEventSource.last?.done(0, '/');
    }

    expect(terminal.history).toEqual(['ls', 'pwd']);
  });

  it('stops a command by replacing the shell, and says so', async () => {
    const terminal = openShell();
    const first = FakeEventSource.last;
    await terminal.run('sleep 100');

    terminal.interrupt();

    expect(first?.closed).toBe(true);
    expect(FakeEventSource.last).not.toBe(first);
    expect(terminal.running).toBe(false);
    expect(terminal.lines.at(-1)?.kind).toBe('status');
  });

  it('says why a command was not taken', async () => {
    const terminal = openShell();
    vi.mocked(request).mockRejectedValueOnce(new Error('invalid command'));

    await terminal.run('bad');

    expect(terminal.running).toBe(false);
    expect(terminal.lines.at(-1)).toEqual({ kind: 'status', text: 'invalid command' });
  });

  it('keeps only the latest lines', () => {
    const terminal = openShell();

    FakeEventSource.last?.print(...Array.from({ length: MAX_LINES + 5 }, (_, i) => `line ${i}`));

    expect(terminal.lines).toHaveLength(MAX_LINES);
    expect(terminal.lines[0]?.text).toBe('line 5');
    expect(terminal.received).toBe(MAX_LINES + 5);
  });

  it('says where the connection is when the stream drops', () => {
    const terminal = openShell();

    FakeEventSource.last?.fail(false);
    expect(terminal.connection).toBe('connecting');
    expect(terminal.session).toBeNull();
    FakeEventSource.last?.fail(true);
    expect(terminal.connection).toBe('closed');
  });

  it('reads and saves the look of the console, and says why a save was refused', async () => {
    const terminal = useTerminalStore();
    vi.mocked(request).mockResolvedValueOnce(settings);
    await terminal.loadSettings();
    expect(terminal.settings).toEqual(settings);

    const bigger = { ...settings, fontSize: 18 };
    vi.mocked(request).mockResolvedValueOnce(bigger);
    expect(await terminal.saveSettings(bigger)).toBe(true);
    expect(terminal.settings).toEqual(bigger);

    vi.mocked(request).mockRejectedValueOnce(new Error('invalid terminal settings'));
    expect(await terminal.saveSettings(bigger)).toBe(false);
    expect(terminal.error).toBe('invalid terminal settings');
  });

  it('follows the log of the machine and stops when asked, keeping only the latest lines', () => {
    const terminal = useTerminalStore();

    terminal.followLog();
    const log = FakeEventSource.last;
    expect(log?.url).toBe('/api/terminal/log');
    expect(terminal.logConnection).toBe('connecting');
    log?.connect();
    log?.print(...Array.from({ length: MAX_LINES + 2 }, (_, i) => `entry ${i}`));

    expect(terminal.logConnection).toBe('open');
    expect(terminal.log).toHaveLength(MAX_LINES);
    expect(terminal.log[0]).toEqual({ kind: 'output', text: 'entry 2' });
    expect(terminal.logReceived).toBe(MAX_LINES + 2);

    terminal.clearLog();
    terminal.stopLog();
    expect(terminal.log).toEqual([]);
    expect(log?.closed).toBe(true);
    expect(terminal.logConnection).toBe('closed');
  });
});
