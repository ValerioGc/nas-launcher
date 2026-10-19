import { CodeRefused, MailNotSent } from '@/api/account';
import { LoginThrottled, PasswordRefused } from '@/api/gateway';
import { t } from '@/i18n';
import { failureMessage } from '@/utils/failureMessage';

/**
 * Returns a user-friendly message for a password-related failure.
 */
export function passwordFailure(error: unknown) {
  if (error instanceof PasswordRefused) {
    return t('wrongAccountPassword');
  }
  if (error instanceof LoginThrottled) {
    return t('tooManyPasswordAttempts');
  }
  if (error instanceof CodeRefused) {
    return t('accountCodeRefused');
  }
  if (error instanceof MailNotSent) {
    return t('accountMailNotSent');
  }
  return failureMessage(error);
}
