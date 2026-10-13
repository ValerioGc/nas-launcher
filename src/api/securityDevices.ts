import { CONFIRMATION_HEADER, confirmPassword } from '@/api/gateway';
import type { SecurityDeviceRequest } from '@/types/types';

/**
 * What the security center may do to the devices allowed in with mTLS, through the shell: the
 * routes of the gateway, with the session of the console. Issuing a certificate and turning mTLS
 * on need the password again, which the application asks for and the shell checks with the
 * gateway: the code it gets back never leaves the console. What the application sends is checked
 * here again before anything is asked.
 */

type Call = () => Promise<unknown>;

const ICONS = new Set(['tv', 'desktop', 'laptop', 'phone', 'projector']);
const NAME_LIMIT = 60;
const PASSWORD_LIMIT = 128;

export function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function deviceId(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value > 0;
}

export function password(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= PASSWORD_LIMIT;
}

function named(value: Record<string, unknown>) {
  return (
    typeof value.name === 'string' &&
    value.name.trim().length > 0 &&
    [...value.name.trim()].length <= NAME_LIMIT &&
    typeof value.icon === 'string' &&
    ICONS.has(value.icon)
  );
}

/** A route of the gateway, answering its JSON or saying why it refused. */
export async function gateway(
  path: string,
  init: RequestInit = {},
  code?: string,
): Promise<unknown> {
  const response = await fetch(path, {
    ...init,
    credentials: 'include',
    cache: 'no-store',
    signal: AbortSignal.timeout(30000),
    headers: {
      'Content-Type': 'application/json',
      ...(code ? { [CONFIRMATION_HEADER]: code } : {}),
    },
  });
  if (response.status === 204) {
    return null;
  }
  const body: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const reason = record(body) && typeof body.error === 'string' ? body.error : '';
    throw new Error(reason || `the gateway answered ${response.status}`);
  }
  return body;
}

/** The same route, after the gateway checked the password once more. */
export async function confirmed(path: string, init: RequestInit, given: string): Promise<unknown> {
  return gateway(path, init, await confirmPassword(given));
}

function saveMtls(payload: unknown): Call | null {
  if (!record(payload) || typeof payload.enabled !== 'boolean' || !password(payload.password)) {
    return null;
  }
  const { enabled, password: given } = payload;
  return () => confirmed('/auth/mtls', { method: 'PUT', body: JSON.stringify({ enabled }) }, given);
}

function addDevice(payload: unknown): Call | null {
  if (!record(payload) || !named(payload) || !password(payload.password)) {
    return null;
  }
  const body = JSON.stringify({ name: payload.name, icon: payload.icon });
  const given = payload.password;
  return () => confirmed('/auth/devices', { method: 'POST', body }, given);
}

function updateDevice(payload: unknown): Call | null {
  if (!record(payload) || !deviceId(payload.id) || !named(payload)) {
    return null;
  }
  const body = JSON.stringify({ name: payload.name, icon: payload.icon });
  const id = payload.id;
  return () => gateway(`/auth/devices/${id}`, { method: 'PUT', body });
}

function renewDevice(payload: unknown): Call | null {
  if (!record(payload) || !deviceId(payload.id) || !password(payload.password)) {
    return null;
  }
  const { id, password: given } = payload;
  return () => confirmed(`/auth/devices/${id}/renew`, { method: 'POST' }, given);
}

function onDevice(method: string, suffix: string) {
  return (payload: unknown): Call | null => {
    if (!record(payload) || !deviceId(payload.id)) {
      return null;
    }
    const id = payload.id;
    return () => gateway(`/auth/devices/${id}${suffix}`, { method });
  };
}

/** Each request about the devices, and the call it becomes for what it sent. */
export const DEVICE_CALLS: Record<SecurityDeviceRequest, (payload: unknown) => Call | null> = {
  'mtls-state': () => () => gateway('/auth/mtls'),
  'save-mtls': saveMtls,
  devices: () => () => gateway('/auth/devices'),
  'add-device': addDevice,
  'update-device': updateDevice,
  'renew-device': renewDevice,
  'revoke-device': onDevice('POST', '/revoke'),
  'remove-device': onDevice('DELETE', ''),
};
