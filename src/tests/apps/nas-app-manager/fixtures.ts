import { vi } from 'vitest';
import { request } from '@/api/client';
import type { AppOffer, ForgeCredentials, ManagedApp } from '@/apps/nas-app-manager/types';

export const installed: ManagedApp = {
  id: 'example',
  name: 'Example',
  info: 'A package',
  system: false,
  version: '1.0.0',
  running: false,
};

export const system: ManagedApp = {
  ...installed,
  id: 'nas-global-search',
  name: 'System',
  system: true,
};

export const offer: AppOffer = {
  id: installed.id,
  name: installed.name,
  info: installed.info,
  repository: 'example/app',
  logo: '',
  installedVersion: '1.0.0',
  latestVersion: '2.0.0',
  updateAvailable: true,
};

export const credentials: ForgeCredentials = {
  configured: false,
  packagesEnabled: true,
  appsManaged: true,
};

export function answerManager() {
  vi.mocked(request)
    .mockReset()
    .mockImplementation(async (path) => {
      if (path === '/apps/credentials') {
        return credentials as never;
      }
      if (path === '/apps/installed') {
        return [installed, system] as never;
      }
      if (path === '/apps/available' || path === '/apps/updates') {
        return [offer] as never;
      }
      return [] as never;
    });
}

export function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
