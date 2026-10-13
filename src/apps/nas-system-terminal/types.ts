export interface TerminalSettings {
  transparency: number;
  textColor: string;
  backgroundColor: string;
  highlightColor: string;
  /** In pixels. */
  fontSize: number;
  lineHeight: number;
}

export type TerminalConnection = 'connecting' | 'open' | 'closed';

export interface TerminalEntry {
  kind: 'input' | 'output' | 'status';
  text: string;
}
