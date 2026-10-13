import { request } from '@/api/client';
import { ANTIVIRUS_CALLS } from '@/api/antivirus';
import { saveSignInPolicy, signInPolicy } from '@/api/gateway';
import { CERTIFICATE_CALLS } from '@/api/securityCertificates';
import { DEVICE_CALLS } from '@/api/securityDevices';
import type { AppNotification } from '@/apps/nas-notification-manager/types';
import type { AccessRequest, RefusedSignIn } from '@/types/types';

/**
 * What the security center may do to the access to the NAS, through the shell: the same routes
 * the console uses, with the session of the console. The application never reaches the launcher
 * or the gateway itself, and what it sends is checked here again before anything is asked.
 */

const RULE_ID = /^\d{1,20}$/;
const TEXT_LIMIT = 64;

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function text(value: unknown): value is string {
  return typeof value === 'string' && value.length <= TEXT_LIMIT;
}

function count(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0;
}

async function refusedSignIns(): Promise<RefusedSignIn[]> {
  const notifications = await request<AppNotification[]>('/notifications?limit=500');
  return notifications
    .filter((notification) => notification.key === 'signInRefused')
    .map((notification) => ({
      id: notification.id,
      at: notification.at,
      from: notification.params?.from ?? '',
      field: notification.params?.field ?? '',
    }));
}

type Call = () => Promise<unknown>;

function saveSettings(payload: unknown): Call | null {
  if (!record(payload) || typeof payload.notifyFailures !== 'boolean') {
    return null;
  }
  const body = JSON.stringify({ notifyFailures: payload.notifyFailures });
  return () => request('/settings/access', { method: 'PUT', body });
}

function saveDiscovery(payload: unknown): Call | null {
  if (!record(payload) || typeof payload.enabled !== 'boolean') {
    return null;
  }
  const body = JSON.stringify({ enabled: payload.enabled });
  return () => request('/settings/discovery', { method: 'PUT', body });
}

function addRule(payload: unknown): Call | null {
  if (
    !record(payload) ||
    (payload.kind !== 'ip' && payload.kind !== 'range') ||
    !text(payload.value) ||
    !text(payload.label)
  ) {
    return null;
  }
  const body = JSON.stringify({ kind: payload.kind, value: payload.value, label: payload.label });
  return () => request('/access/rules', { method: 'POST', body });
}

function removeRule(payload: unknown): Call | null {
  if (!record(payload) || typeof payload.id !== 'string' || !RULE_ID.test(payload.id)) {
    return null;
  }
  const id = payload.id;
  return () => request(`/access/rules/${id}`, { method: 'DELETE' });
}

function savePolicy(payload: unknown): Call | null {
  if (
    !record(payload) ||
    typeof payload.enabled !== 'boolean' ||
    !count(payload.attempts) ||
    !count(payload.lockMinutes)
  ) {
    return null;
  }
  const policy = {
    enabled: payload.enabled,
    attempts: payload.attempts,
    lockMinutes: payload.lockMinutes,
  };
  return () => saveSignInPolicy(policy);
}

/** Each request, and the call it becomes for what it sent. */
const CALLS: Record<AccessRequest, (payload: unknown) => Call | null> = {
  'access-settings': () => () => request('/settings/access'),
  'discovery-settings': () => () => request('/settings/discovery'),
  'access-rules': () => () => request('/access/rules'),
  'sign-in-policy': () => () => signInPolicy(),
  'refused-sign-ins': () => refusedSignIns,
  'save-access-settings': saveSettings,
  'save-discovery-settings': saveDiscovery,
  'add-access-rule': addRule,
  'remove-access-rule': removeRule,
  'save-sign-in-policy': savePolicy,
  ...DEVICE_CALLS,
  ...CERTIFICATE_CALLS,
  ...ANTIVIRUS_CALLS,
};

/**
 * The call that answers a request of the security center, or null when what it sent is not one
 * the launcher takes: such a request gets no answer, like any other malformed message.
 */
export function accessCall(type: AccessRequest, payload: unknown): Call | null {
  return Object.hasOwn(CALLS, type) ? CALLS[type](payload) : null;
}
