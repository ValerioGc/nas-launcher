import { t } from '@/i18n';

/**
 * Returns a user-friendly failure message based on the provided reason.
 */
export function failureMessage(reason: unknown) {
  return reason instanceof Error && reason.message ? reason.message : t('requestFailed');
}
