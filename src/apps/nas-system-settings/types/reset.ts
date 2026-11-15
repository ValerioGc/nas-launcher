export type ResetBlock = 'disabled' | 'noWizard' | 'noOriginalState';

/**
 * Whether the NAS can be reset here, why not when it cannot, and how a requested reset goes.
 */
export interface ResetState {
  available: boolean;
  reason?: ResetBlock | null;
  running: boolean;
  /** The reset this launcher started stopped before it removed anything. */
  failed: boolean;
}
