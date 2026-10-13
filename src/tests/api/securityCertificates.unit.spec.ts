import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { accessCall } from '@/api/access';
import { CERTIFICATE_CALLS } from '@/api/securityCertificates';
import { confirmPassword } from '@/api/gateway';

vi.mock('@/api/gateway', async (original) => ({
  ...(await original<typeof import('@/api/gateway')>()),
  confirmPassword: vi.fn(),
}));

const PEM = '-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n';
const KEY = '-----BEGIN PRIVATE KEY-----\nMIIE\n-----END PRIVATE KEY-----\n';

beforeEach(() => {
  vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('{}')));
  vi.mocked(confirmPassword).mockReset().mockResolvedValue('c0de');
});

afterEach(() => vi.unstubAllGlobals());

const sent = () =>
  vi.mocked(fetch).mock.calls.map(([path, init]) => ({
    path,
    method: (init as RequestInit).method ?? 'GET',
    body: (init as RequestInit).body,
    code: ((init as RequestInit).headers as Record<string, string>)['X-NAS-Confirmation'],
  }));

describe('what the security center may ask about the certificate', () => {
  it('reads the certificate in use without the password', async () => {
    await accessCall('certificate', undefined)?.();
    expect(sent()).toEqual([
      { path: '/auth/certificate', method: 'GET', body: undefined, code: undefined },
    ]);
    expect(confirmPassword).not.toHaveBeenCalled();
  });

  it('confirms the password before every change', async () => {
    const password = 'long enough';
    await CERTIFICATE_CALLS['save-certificate-names']({ names: ['nas.example.org'], password })?.();
    await CERTIFICATE_CALLS['renew-certificate']({ password })?.();
    await CERTIFICATE_CALLS['upload-certificate']({ certificate: PEM, key: KEY, password })?.();
    await CERTIFICATE_CALLS['remove-custom-certificate']({ password })?.();

    expect(confirmPassword).toHaveBeenCalledTimes(4);
    expect(sent()).toEqual([
      {
        path: '/auth/certificate/names',
        method: 'PUT',
        body: '{"names":["nas.example.org"]}',
        code: 'c0de',
      },
      { path: '/auth/certificate/renew', method: 'POST', body: undefined, code: 'c0de' },
      {
        path: '/auth/certificate/custom',
        method: 'PUT',
        body: JSON.stringify({ certificate: PEM, key: KEY }),
        code: 'c0de',
      },
      { path: '/auth/certificate/custom', method: 'DELETE', body: undefined, code: 'c0de' },
    ]);
  });

  it('asks nothing for a malformed request', () => {
    for (const [type, payload] of [
      ['save-certificate-names', { names: 'nas.example.org', password: 'x' }],
      ['save-certificate-names', { names: Array(21).fill('a'), password: 'x' }],
      ['renew-certificate', {}],
      ['upload-certificate', { certificate: 'not pem', key: KEY, password: 'x' }],
      ['upload-certificate', { certificate: PEM, key: KEY, password: '' }],
      ['remove-custom-certificate', null],
    ] as const) {
      expect(accessCall(type, payload), type).toBeNull();
    }
  });
});
