import { request } from '@/api/client';
import { record } from '@/api/securityDevices';
import type { AntivirusRequest } from '@/types/types';

type Call = () => Promise<unknown>;
const KINDS = new Set(['full', 'quick', 'file', 'custom']);
function paths(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.length <= 100 &&
    value.every(
      (path) =>
        typeof path === 'string' &&
        path.startsWith('/') &&
        path.length <= 4096 &&
        !Array.from(path).some((character) => {
          const point = character.codePointAt(0);
          return point !== undefined && (point < 32 || point === 127);
        }) &&
        !path.split('/').includes('..'),
    )
  );
}
function options(value: unknown): boolean {
  return (
    record(value) &&
    typeof value.archives === 'boolean' &&
    typeof value.detectPua === 'boolean' &&
    typeof value.maxFileMb === 'number' &&
    Number.isInteger(value.maxFileMb) &&
    value.maxFileMb >= 1 &&
    value.maxFileMb <= 2048 &&
    paths(value.paths) &&
    paths(value.exclusions)
  );
}
function scan(payload: unknown): Call | null {
  if (
    !record(payload) ||
    typeof payload.kind !== 'string' ||
    !KINDS.has(payload.kind) ||
    !paths(payload.paths) ||
    !['boolean', 'undefined'].includes(typeof payload.skipUnchanged)
  ) {
    return null;
  }
  return () => request('/antivirus/scan', { method: 'POST', body: JSON.stringify(payload) });
}
function lifecycle(value: unknown): boolean {
  return (
    record(value) &&
    typeof value.enabled === 'boolean' &&
    typeof value.kind === 'string' &&
    KINDS.has(value.kind) &&
    paths(value.paths) &&
    (!value.enabled || !['file', 'custom'].includes(value.kind) || value.paths.length > 0) &&
    (!value.enabled || value.kind !== 'file' || value.paths.length === 1)
  );
}
function save(payload: unknown): Call | null {
  if (
    !record(payload) ||
    typeof payload.enabled !== 'boolean' ||
    typeof payload.schedulerEnabled !== 'boolean' ||
    !lifecycle(payload.bootScan) ||
    !lifecycle(payload.shutdownScan) ||
    typeof payload.autoQuarantine !== 'boolean' ||
    !['full', 'quick', 'file', 'custom'].every((kind) => options(payload[kind])) ||
    !Array.isArray(payload.schedules) ||
    payload.schedules.length > 20 ||
    !payload.schedules.every(
      (job: unknown) =>
        record(job) &&
        typeof job.id === 'string' &&
        /^[a-zA-Z0-9-]{1,64}$/.test(job.id) &&
        typeof job.enabled === 'boolean' &&
        typeof job.kind === 'string' &&
        KINDS.has(job.kind) &&
        typeof job.day === 'string' &&
        ['*', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].includes(job.day) &&
        typeof job.time === 'string' &&
        /^([01]\d|2[0-3]):[0-5]\d$/.test(job.time) &&
        paths(job.paths),
    )
  ) {
    return null;
  }
  return () => request('/antivirus/settings', { method: 'PUT', body: JSON.stringify(payload) });
}
function entry(action: string) {
  return (payload: unknown): Call | null => {
    if (
      !record(payload) ||
      typeof payload.id !== 'string' ||
      !/^[a-zA-Z0-9-]{1,64}$/.test(payload.id)
    ) {
      return null;
    }
    return () =>
      request(`/antivirus/quarantine/${action}`, {
        method: 'POST',
        body: JSON.stringify({ id: payload.id }),
      });
  };
}
export const ANTIVIRUS_CALLS: Record<AntivirusRequest, (payload: unknown) => Call | null> = {
  'antivirus-status': () => () => request('/antivirus'),
  'antivirus-enable': (payload) =>
    record(payload) && typeof payload.enabled === 'boolean'
      ? () =>
          request('/antivirus/enabled', {
            method: 'POST',
            body: JSON.stringify({ enabled: payload.enabled }),
          })
      : null,
  'antivirus-settings': save,
  'antivirus-scan': scan,
  'antivirus-quarantine': entry('quarantine'),
  'antivirus-restore': entry('restore'),
  'antivirus-delete': entry('delete'),
  'antivirus-forget': () => () => request('/antivirus/scanned/forget', { method: 'POST' }),
};
