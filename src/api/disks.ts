import { request } from '@/api/client';
import type { DiskPayload, DiskRequest } from '@/types/types';

/**
 * What the disk manager may do to the external disks, through the shell: eject one, and mark one
 * as not removable. The application never reaches the launcher itself, and what it sends is
 * checked here again before anything is asked.
 */

const DEVICE = /^[\w-]{1,64}$/;
const UUID = /^[\dA-Za-z-]{1,64}$/;
/** The launcher waits for the root unit, which unmounts and detaches the disk. */
const EJECT_WAIT_MS = 130_000;

type Call = () => Promise<unknown>;

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
  'eject-disk': eject,
  'disk-locks': () => () => request<string[]>('/storage/locks'),
  'lock-disk': lock,
};

/** The call for a request of the disk manager, or nothing when what it sent is not valid. */
export function diskCall(type: DiskRequest, payload: DiskPayload): Call | null {
  return Object.hasOwn(CALLS, type) ? CALLS[type](payload) : null;
}
