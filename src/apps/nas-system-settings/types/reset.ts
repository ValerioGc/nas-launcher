export type ResetBlock = 'disabled' | 'noWizard' | 'noOriginalState';

/**
 * Whether the NAS can be reset here, and why not when it cannot.
 */
export interface ResetState {
  available: boolean;
  reason?: ResetBlock | null;
}
