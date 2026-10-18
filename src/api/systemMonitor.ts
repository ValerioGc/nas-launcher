import { request } from '@/api/client';
import type { SystemMonitorRequest } from '@/types/types';

/**
 * The shell validates privileged process actions and invokes its protected helper with the
 * console session. Telemetry requests from older clients are forwarded to the application's
 * backend; current clients read their own backend directly.
 */

/** The only application, and its widget, that reads and acts on the processes of the machine. */
export const SYSTEM_MONITOR = 'nas-system-monitor';

/** A service may take its time to restart, and the launcher waits for it. */
const RESTART_WAIT_MS = 120_000;

type Call = () => Promise<unknown>;

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function whole(value: unknown, least: number): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= least;
}

function processAction(payload: unknown): Call | null {
  if (
    !record(payload) ||
    !whole(payload.id, 1) ||
    !whole(payload.started, 0) ||
    (payload.action !== 'terminate' && payload.action !== 'restart')
  ) {
    return null;
  }
  const { id, started, action } = payload;
  return () =>
    request(`/system/processes/${id}`, {
      method: 'POST',
      body: JSON.stringify({ action, started }),
      signal: AbortSignal.timeout(RESTART_WAIT_MS),
    });
}

/** The call a request of the monitor becomes, or none when what it sent is not valid. */
export function monitorCall(type: SystemMonitorRequest, payload: unknown): Call | null {
  return type === 'system-telemetry' ? () => request('/system/telemetry') : processAction(payload);
}
