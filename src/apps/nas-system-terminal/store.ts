import { defineStore } from 'pinia';
import { ref } from 'vue';
import { request } from '@/api/client';
import { t } from '@/i18n';
import { failureMessage } from '@/utils/failureMessage';
import type {
  TerminalConnection,
  TerminalEntry,
  TerminalSettings,
} from '@/apps/nas-system-terminal/types';

const SESSION_URL = '/api/terminal/session';
const LOG_URL = '/api/terminal/log';
const SETTINGS_PATH = '/settings/terminal';
export const MAX_LINES = 2000;
const MAX_HISTORY = 100;

function append(lines: TerminalEntry[], entry: TerminalEntry) {
  lines.push(entry);
  if (lines.length > MAX_LINES) {
    lines.splice(0, lines.length - MAX_LINES);
  }
}

export const useTerminalStore = defineStore('nas-system-terminal', () => {
  const lines = ref<TerminalEntry[]>([]);
  const received = ref(0);
  const connection = ref<TerminalConnection>('closed');
  const session = ref<string | null>(null);
  const cwd = ref('');
  const running = ref(false);
  const history = ref<string[]>([]);
  const log = ref<TerminalEntry[]>([]);
  const logReceived = ref(0);
  const logConnection = ref<TerminalConnection>('closed');
  const settings = ref<TerminalSettings | null>(null);
  const saving = ref(false);
  const error = ref('');
  let source: EventSource | null = null;
  let logSource: EventSource | null = null;

  function write(kind: TerminalEntry['kind'], text: string) {
    append(lines.value, { kind, text });
    received.value += 1;
  }

  /** Follows the log of the machine, from its last lines. */
  function followLog() {
    stopLog();
    logConnection.value = 'connecting';
    const stream = new EventSource(LOG_URL);
    stream.onopen = () => {
      logConnection.value = 'open';
    };
    stream.onmessage = (event: MessageEvent<string>) => {
      append(log.value, { kind: 'output', text: event.data });
      logReceived.value += 1;
    };
    stream.onerror = () => {
      logConnection.value = stream.readyState === EventSource.CLOSED ? 'closed' : 'connecting';
    };
    logSource = stream;
  }

  function stopLog() {
    logSource?.close();
    logSource = null;
    logConnection.value = 'closed';
  }

  function clearLog() {
    log.value = [];
  }

  /**
   * Opens a shell for this window. A stream that drops is reopened by the browser, with a new shell.
   */
  function connect() {
    disconnect();
    connection.value = 'connecting';
    const stream = new EventSource(SESSION_URL);
    stream.addEventListener('session', (event: MessageEvent<string>) => {
      session.value = event.data;
      connection.value = 'open';
    });

    stream.onmessage = (event: MessageEvent<string>) => write('output', event.data);
    stream.addEventListener('done', (event: MessageEvent<string>) => {
      const [code = '', ...directory] = event.data.split(' ');
      cwd.value = directory.join(' ');
      if (running.value && code !== '0') {
        write('status', t('terminalExit', { code }));
      }

      running.value = false;
    });

    stream.onerror = () => {
      session.value = null;
      running.value = false;
      connection.value = stream.readyState === EventSource.CLOSED ? 'closed' : 'connecting';
    };

    source = stream;
  }

  function disconnect() {
    source?.close();
    source = null;
    session.value = null;
    running.value = false;
    connection.value = 'closed';
  }

  /**
   * Sends one command to the shell. What it prints arrives on the stream, and so does its end.
   */
  async function run(command: string) {
    if (!session.value || running.value) {
      return;
    }
    write('input', `${cwd.value} $ ${command}`);
    if (command.trim() && history.value.at(-1) !== command) {
      history.value.push(command);
      history.value.splice(0, Math.max(0, history.value.length - MAX_HISTORY));
    }
    running.value = true;
    try {
      await request(`/terminal/session/${session.value}/input`, {
        method: 'POST',
        body: JSON.stringify({ command }),
      });
    } catch (reason) {
      running.value = false;
      write('status', failureMessage(reason));
    }
  }

  function interrupt() {
    write('status', t('terminalRestarted'));
    connect();
  }

  function clear() {
    lines.value = [];
  }

  async function loadSettings() {
    try {
      settings.value = await request<TerminalSettings>(SETTINGS_PATH);
    } catch (reason) {
      error.value = failureMessage(reason);
    }
  }

  /**
   * Saves how the console looks.
   * @returns Whether the launcher took the settings.
   */
  async function saveSettings(next: TerminalSettings) {
    saving.value = true;
    error.value = '';
    try {
      settings.value = await request<TerminalSettings>(SETTINGS_PATH, {
        method: 'PUT',
        body: JSON.stringify(next),
      });
      return true;
    } catch (reason) {
      error.value = failureMessage(reason);
      return false;
    } finally {
      saving.value = false;
    }
  }

  return {
    lines,
    received,
    connection,
    session,
    cwd,
    running,
    history,
    log,
    logReceived,
    logConnection,
    settings,
    saving,
    error,
    connect,
    disconnect,
    run,
    interrupt,
    clear,
    followLog,
    stopLog,
    clearLog,
    loadSettings,
    saveSettings,
  };
});
