/**
 * Types for power management settings.
 */
export interface PowerRule {
  day: string;
  time: string;
  action: string;
  active: boolean;
}

/**
 * Types for overall power settings.
 */
export interface PowerSettings {
  enabled: boolean;
  rules: PowerRule[];
  /** Minutes without a key or the mouse before the monitor connected to the NAS goes dark. */
  screenOffMinutes: number;
}
