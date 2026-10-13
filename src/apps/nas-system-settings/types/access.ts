/**
 * Types for access settings.
 */
export interface AccessSettings {
  notifyFailures: boolean;
}

/**
 * How an allowed entry is matched: an exact address or a CIDR range.
 */
export type AccessRuleKind = 'ip' | 'range';
