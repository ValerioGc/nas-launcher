import type { TerminalSettings } from '@/apps/nas-system-terminal/types';

export const settings: TerminalSettings = {
  transparency: 20,
  textColor: '#d6e2ea',
  backgroundColor: '#0e1922',
  highlightColor: '#3fd39b',
  fontSize: 14,
  lineHeight: 1.5,
};

type Listener = (event: MessageEvent<string>) => void;

export class FakeEventSource {
  static readonly CLOSED = 2;
  static last: FakeEventSource | null = null;
  static readonly opened = new Map<string, FakeEventSource>();

  readyState = 0;
  closed = false;
  onopen: (() => void) | null = null;
  onmessage: Listener | null = null;
  onerror: (() => void) | null = null;
  private readonly listeners = new Map<string, Listener>();

  constructor(readonly url: string) {
    FakeEventSource.last = this;
    FakeEventSource.opened.set(url, this);
  }

  addEventListener(type: string, listener: Listener) {
    this.listeners.set(type, listener);
  }

  close() {
    this.closed = true;
    this.readyState = FakeEventSource.CLOSED;
  }

  open(session = 'session-1') {
    this.readyState = 1;
    this.listeners.get('session')?.({ data: session } as MessageEvent<string>);
    this.done(0, '/');
  }

  connect() {
    this.readyState = 1;
    this.onopen?.();
  }

  print(...lines: string[]) {
    for (const data of lines) {
      this.onmessage?.({ data } as MessageEvent<string>);
    }
  }

  done(code: number, cwd: string) {
    this.listeners.get('done')?.({ data: `${code} ${cwd}` } as MessageEvent<string>);
  }

  fail(closed: boolean) {
    this.readyState = closed ? FakeEventSource.CLOSED : 0;
    this.onerror?.();
  }
}
