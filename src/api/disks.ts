import { request } from '@/api/client';
import type { DiskPayload, DiskRequest } from '@/types/types';

/**
 * What the disk manager may ask about the disks, through the shell: list them, eject an external
 * one, mark one as not removable, format a data disk and open, close and key an encrypted volume.
 * The application never reaches the launcher itself, and what it sends is checked here again
 * before anything is asked.
 */

const DEVICE = /^[\w-]{1,64}$/;
const UUID = /^[\dA-Za-z-]{1,64}$/;
/** The launcher waits for the root unit, which unmounts and detaches the disk. */
const EJECT_WAIT_MS = 130_000;
/** A format partitions the disk and makes its filesystem, a lock waits for the unmounts. */
const CRYPT_WAIT_MS = 16 * 60 * 1000;
const NETWORK_OPERATIONS = new Set([
  'networkSave',
  'networkTest',
  'networkConnect',
  'networkDisconnect',
  'networkForget',
]);
const CRYPT_OPERATIONS = new Set([
  'format',
  'unlock',
  'lock',
  'automatic',
  'password',
  'recovery',
  'header',
]);

type Call = () => Promise<unknown>;

/** Follows the work of a disk until it is done, failed, or fifteen minutes have passed. */
export async function waitForTask(taskId: string) {
  const deadline = Date.now() + 15 * 60 * 1000;
  while (Date.now() < deadline) {
    const task = await request<{ state: string; detail: string }>(
      `/tasks/${encodeURIComponent(taskId)}`,
    );
    if (task.state === 'done') {
      return;
    }
    if (task.state === 'failed' || task.state === 'cancelled') {
      throw new Error(task.detail || 'Disk operation failed');
    }
    await new Promise((resolve) => globalThis.setTimeout(resolve, 500));
  }
  throw new Error('Disk operation timed out');
}

/** A password travels only on HTTPS, or on the machine itself (its monitor). */
function secure(): boolean {
  const page = globalThis.location;
  return page.protocol === 'https:' || ['localhost', '127.0.0.1', '[::1]'].includes(page.hostname);
}

/** A request with secrets for the root helper: one of `operations`, flat, its passwords only on
 * a secure page. */
function withSecrets(operations: Set<string>, path: string) {
  return ({ request: body }: DiskPayload): Call | null => {
    if (!body || typeof body !== 'object' || Array.isArray(body)) {
      return null;
    }
    const fields = body as Record<string, unknown>;
    const flat = Object.values(fields).every(
      (value) =>
        value === null ||
        value === undefined ||
        typeof value === 'string' ||
        typeof value === 'boolean',
    );
    if (typeof fields.op !== 'string' || !operations.has(fields.op) || !flat) {
      return null;
    }
    if (['password', 'current'].some((key) => typeof fields[key] === 'string') && !secure()) {
      return () => Promise.reject(new Error('Open the NAS over HTTPS to type a disk password'));
    }
    return async () => {
      const answer = await request<{ taskId?: string }>(path, {
        method: 'POST',
        body: JSON.stringify(fields),
        signal: AbortSignal.timeout(CRYPT_WAIT_MS),
      });
      if (answer.taskId) {
        await waitForTask(answer.taskId);
      }
      return answer;
    };
  };
}

function inventory({ kind }: DiskPayload): Call | null {
  return kind === 'local' || kind === 'external'
    ? () => request<unknown[]>(`/storage/disks?kind=${kind}`)
    : null;
}

function eject({ device }: DiskPayload): Call | null {
  return typeof device === 'string' && DEVICE.test(device)
    ? () =>
        request('/storage/disks/eject', {
          method: 'POST',
          body: JSON.stringify({ device }),
          signal: AbortSignal.timeout(EJECT_WAIT_MS),
        })
    : null;
}

function lock({ uuid, locked }: DiskPayload): Call | null {
  return typeof uuid === 'string' && UUID.test(uuid) && typeof locked === 'boolean'
    ? () =>
        request<string[]>('/storage/locks', {
          method: 'POST',
          body: JSON.stringify({ uuid, locked }),
        })
    : null;
}

const CALLS: Record<DiskRequest, (payload: DiskPayload) => Call | null> = {
  'disk-inventory': inventory,
  'disk-encryption': () => () => request<boolean>('/storage/encryption'),
  'disk-crypt': withSecrets(CRYPT_OPERATIONS, '/storage/encryption'),
  'smb-disks': () => () => request<unknown>('/storage/network'),
  'smb-disk': withSecrets(NETWORK_OPERATIONS, '/storage/network'),
  'eject-disk': eject,
  'disk-locks': () => () => request<string[]>('/storage/locks'),
  'lock-disk': lock,
};

/** The call for a request of the disk manager, or nothing when what it sent is not valid. */
export function diskCall(type: DiskRequest, payload: DiskPayload): Call | null {
  return Object.hasOwn(CALLS, type) ? CALLS[type](payload) : null;
}
