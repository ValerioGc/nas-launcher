/**
 * Why the NAS cannot be reset here: the switch is off, or the wizard that would configure it again
 * is not on the machine.
 */
export type ResetBlock = 'disabled' | 'noWizard' | 'noOriginalState';

/**
 * Whether the NAS can be reset here, and why not when it cannot.
 */
export interface ResetState {
  available: boolean;
  reason?: ResetBlock | null;
}
