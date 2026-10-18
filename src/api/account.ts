import { request } from '@/api/client';
import { CONFIRMATION_HEADER, LoginThrottled } from '@/api/gateway';
import type { AccountInfo, MailStatus } from '@/types/types';

const ACCOUNT_URL = '/auth/account';
const RECOVERY_URL = '/auth/recovery';

/** The gateway refused a code sent by mail: wrong, used or expired. */
export class CodeRefused extends Error {}

/** The Email app could not send the mail with the code. */
export class MailNotSent extends Error {}

function isAccount(value: unknown): value is AccountInfo {
  return (
    typeof value === 'object' &&
    value !== null &&
    'username' in value &&
    typeof value.username === 'string' &&
    'email' in value &&
    (value.email === null || typeof value.email === 'string') &&
    'recovery' in value &&
    typeof value.recovery === 'boolean'
  );
}

/**
 * One request to the routes of the account on the gateway. `confirmation` is the code of
 * `confirmPassword`, for what needs the password again.
 */
async function call(
  path: string,
  options: { method?: string; body?: unknown; confirmation?: string } = {},
): Promise<Response> {
  const response = await fetch(path, {
    method: options.method ?? 'GET',
    credentials: 'include',
    cache: 'no-store',
    signal: AbortSignal.timeout(30000),
    headers: {
      'Content-Type': 'application/json',
      ...(options.confirmation ? { [CONFIRMATION_HEADER]: options.confirmation } : {}),
    },
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  });
  if (response.ok) {
    return response;
  }
  if (response.status === 429) {
    throw new LoginThrottled('too many refused attempts');
  }
  if (response.status === 502 || response.status === 503) {
    throw new MailNotSent('the mail with the code could not be sent');
  }
  let reason = `the account could not be changed (${response.status})`;
  try {
    const failure: unknown = await response.json();
    if (typeof failure === 'object' && failure !== null && 'error' in failure) {
      reason = String(failure.error);
    }
  } catch {
    // No reason given: the status says enough.
  }
  throw response.status === 400 && reason.includes('code')
    ? new CodeRefused(reason)
    : new Error(reason);
}

async function accountOf(response: Response): Promise<AccountInfo> {
  const result: unknown = await response.json();
  if (!isAccount(result)) {
    throw new Error('invalid account response');
  }
  return result;
}

export async function readAccount(): Promise<AccountInfo> {
  return accountOf(await call(ACCOUNT_URL));
}

export async function changeUsername(username: string, confirmation: string) {
  return accountOf(
    await call(`${ACCOUNT_URL}/username`, { method: 'PUT', body: { username }, confirmation }),
  );
}

export async function changePassword(password: string, confirmation: string): Promise<void> {
  await call(`${ACCOUNT_URL}/password`, { method: 'PUT', body: { password }, confirmation });
}

/** Sends the code that verifies `email` as the recovery address. */
export async function requestEmail(email: string, confirmation: string): Promise<void> {
  await call(`${ACCOUNT_URL}/email`, { method: 'POST', body: { email }, confirmation });
}

export async function verifyEmail(code: string): Promise<AccountInfo> {
  return accountOf(await call(`${ACCOUNT_URL}/email/verify`, { method: 'POST', body: { code } }));
}

/** Turns the reset of the password by mail on or off. */
export async function setRecovery(enabled: boolean, confirmation: string) {
  return accountOf(
    await call(`${ACCOUNT_URL}/recovery`, { method: 'PUT', body: { enabled }, confirmation }),
  );
}

/** Whether the sign-in page offers the reset: never when the gateway cannot say. */
export async function recoveryOffered(): Promise<boolean> {
  try {
    const result: unknown = await (await call(RECOVERY_URL)).json();
    return (
      typeof result === 'object' &&
      result !== null &&
      'enabled' in result &&
      result.enabled === true
    );
  } catch {
    return false;
  }
}

export async function removeEmail(confirmation: string): Promise<void> {
  await call(`${ACCOUNT_URL}/email`, { method: 'DELETE', confirmation });
}

/** Asks for the reset code, which the gateway sends only to a verified address. */
export async function requestPasswordReset(username: string): Promise<void> {
  await call(RECOVERY_URL, { method: 'POST', body: { username } });
}

export async function resetPassword(
  username: string,
  code: string,
  password: string,
): Promise<void> {
  await call(`${RECOVERY_URL}/reset`, { method: 'POST', body: { username, code, password } });
}

/** Whether the Email app is installed and has an SMTP server and a sender. */
export function mailStatus(): Promise<MailStatus> {
  return request<MailStatus>('/mail/status');
}
