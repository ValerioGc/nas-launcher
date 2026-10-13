import type { GatewayLogin, SignInPolicy } from '@/types/types';

export const logoutUrl = __GATEWAY_LOGOUT_URL__;
const LOGIN_URL = '/auth/login';
const SESSION_URL = '/auth/session';
const POLICY_URL = '/auth/policy';
const CONFIRM_URL = '/auth/confirm';
/** Where a request that needs the password again carries the code of `confirmPassword`. */
export const CONFIRMATION_HEADER = 'X-NAS-Confirmation';
const AFTER_LOGOUT = '/login';

export class LoginRefused extends Error {}

export class LoginThrottled extends Error {}

export class PasswordRefused extends Error {}

export async function login(username: string, password: string): Promise<GatewayLogin> {
  const response = await fetch(LOGIN_URL, {
    method: 'POST',
    credentials: 'include',
    cache: 'no-store',
    signal: AbortSignal.timeout(10000),
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  if (response.status === 401) {
    throw new LoginRefused('wrong username or password');
  }
  if (response.status === 429) {
    throw new LoginThrottled('too many refused attempts');
  }
  if (!response.ok) {
    throw new Error(`login failed with ${response.status}`);
  }
  const result: unknown = await response.json();
  if (
    typeof result !== 'object' ||
    result === null ||
    !('username' in result) ||
    typeof result.username !== 'string'
  ) {
    throw new Error('invalid login response');
  }
  const device = 'device' in result && typeof result.device === 'string' ? result.device : null;
  return { username: result.username, device };
}

export async function hasSession(): Promise<boolean> {
  try {
    const response = await fetch(SESSION_URL, {
      credentials: 'include',
      cache: 'no-store',
      signal: AbortSignal.timeout(10000),
    });

    if (!response.ok) {
      return false;
    }

    const result: unknown = await response.json();
    return (
      typeof result === 'object' &&
      result !== null &&
      'authenticated' in result &&
      result.authenticated === true
    );
  } catch {
    return false;
  }
}

async function policyRequest(init?: RequestInit): Promise<SignInPolicy> {
  const response = await fetch(POLICY_URL, {
    credentials: 'include',
    cache: 'no-store',
    signal: AbortSignal.timeout(10000),
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
  if (!response.ok) {
    throw new Error(`the sign-in policy is unavailable (${response.status})`);
  }
  return (await response.json()) as SignInPolicy;
}

export function signInPolicy(): Promise<SignInPolicy> {
  return policyRequest();
}

export function saveSignInPolicy(policy: SignInPolicy): Promise<SignInPolicy> {
  return policyRequest({ method: 'PUT', body: JSON.stringify(policy) });
}

/**
 * Checks the password of the account again, for what the gateway protects beyond the session (the
 * factory reset, the export and the import of the configuration). The code it gives is good for one
 * request, within a minute, in `CONFIRMATION_HEADER`.
 */
export async function confirmPassword(password: string): Promise<string> {
  const response = await fetch(CONFIRM_URL, {
    method: 'POST',
    credentials: 'include',
    cache: 'no-store',
    signal: AbortSignal.timeout(10000),
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ password }),
  });
  if (response.status === 403) {
    throw new PasswordRefused('wrong password');
  }
  if (response.status === 429) {
    throw new LoginThrottled('too many refused attempts');
  }
  if (!response.ok) {
    throw new Error(`the password could not be checked (${response.status})`);
  }
  const result: unknown = await response.json();
  if (
    typeof result !== 'object' ||
    result === null ||
    !('code' in result) ||
    typeof result.code !== 'string'
  ) {
    throw new Error('invalid confirmation response');
  }
  return result.code;
}

export async function logout(): Promise<void> {
  const response = await fetch(logoutUrl, {
    method: 'POST',
    credentials: 'include',
    cache: 'no-store',
    signal: AbortSignal.timeout(10000),
    headers: { 'Content-Type': 'application/json' },
    body: '{}',
  });

  if (!response.ok) {
    throw new Error(`logout failed with ${response.status}`);
  }

  window.location.assign(AFTER_LOGOUT);
}
