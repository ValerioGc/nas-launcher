import { confirmed, gateway, password, record } from '@/api/securityDevices';
import type { SecurityCertificateRequest } from '@/types/types';

/**
 * What the security center may do to the certificate HTTPS answers with, through the shell: the
 * routes of the gateway, with the session of the console. Every change needs the password again,
 * which the shell checks with the gateway. What the application sends is checked here first.
 */

type Call = () => Promise<unknown>;

const NAME_LIMIT = 20;
const NAME_LENGTH = 253;
// A chain and its key in PEM: the gateway refuses more than 64 KiB.
const PEM_LIMIT = 60 * 1024;

function names(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.length <= NAME_LIMIT &&
    value.every((name) => typeof name === 'string' && name.length <= NAME_LENGTH)
  );
}

function pem(value: unknown): value is string {
  return typeof value === 'string' && value.includes('-----BEGIN') && value.length <= PEM_LIMIT;
}

function saveNames(payload: unknown): Call | null {
  if (!record(payload) || !names(payload.names) || !password(payload.password)) {
    return null;
  }
  const body = JSON.stringify({ names: payload.names });
  const given = payload.password;
  return () => confirmed('/auth/certificate/names', { method: 'PUT', body }, given);
}

function upload(payload: unknown): Call | null {
  if (
    !record(payload) ||
    !pem(payload.certificate) ||
    !pem(payload.key) ||
    !password(payload.password)
  ) {
    return null;
  }
  const body = JSON.stringify({ certificate: payload.certificate, key: payload.key });
  const given = payload.password;
  return () => confirmed('/auth/certificate/custom', { method: 'PUT', body }, given);
}

function withPassword(method: string, path: string) {
  return (payload: unknown): Call | null => {
    if (!record(payload) || !password(payload.password)) {
      return null;
    }
    const given = payload.password;
    return () => confirmed(path, { method }, given);
  };
}

export function httpsSettings(value: unknown): value is { port: number; force: boolean } {
  return (
    record(value) &&
    Number.isInteger(value.port) &&
    typeof value.port === 'number' &&
    value.port >= 1 &&
    value.port <= 65535 &&
    typeof value.force === 'boolean'
  );
}

function saveHttps(payload: unknown): Call | null {
  if (!record(payload) || !password(payload.password)) {
    return null;
  }
  const given = payload.password;
  if (!httpsSettings(payload)) {
    return null;
  }
  const { port, force } = payload;
  return () =>
    confirmed('/auth/https', { method: 'PUT', body: JSON.stringify({ port, force }) }, given);
}

/** Each request about the certificate, and the call it becomes for what it sent. */
export const CERTIFICATE_CALLS: Record<
  SecurityCertificateRequest,
  (payload: unknown) => Call | null
> = {
  certificate: () => () => gateway('/auth/certificate'),
  'https-settings': () => () => gateway('/auth/https'),
  'save-https-settings': saveHttps,
  'save-certificate-names': saveNames,
  'renew-certificate': withPassword('POST', '/auth/certificate/renew'),
  'upload-certificate': upload,
  'remove-custom-certificate': withPassword('DELETE', '/auth/certificate/custom'),
};
