import { CodeRefused, MailNotSent } from '@/api/account';
import { LoginThrottled, PasswordRefused } from '@/api/gateway';
import { t } from '@/i18n';
import { failureMessage } from '@/utils/failureMessage';

/**
 * Why an action that asks for the password again did not go on, in the words of the page: the
 * gateway refuses a wrong password or code, and makes the address wait after too many; the Email
 * app may fail to send a code.
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
