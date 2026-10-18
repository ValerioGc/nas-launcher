import { openConsole } from './auth_helpers';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
import type { InstalledApp } from '@/types/types';

const { Given, Then } = createBdd();

const APP = 'handCopied';
const DIRECTORY = join('backend/data/e2e-apps', APP);

const ICON =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><rect width="8" height="8"/></svg>';

Given('a package is copied into the applications directory', async ({ request }) => {
  rmSync(DIRECTORY, { recursive: true, force: true });
  mkdirSync(DIRECTORY, { recursive: true });
  writeFileSync(
    join(DIRECTORY, 'nas-app.json'),
    JSON.stringify({
      manifest_version: 1,
      app: { id: APP, name: 'Copiata a mano', version: '3.1.0', icon: 'icon.svg' },
      run: { exec: 'bin/hand-copied' },
      ui: { protocol: 1 },
    }),
  );
  writeFileSync(join(DIRECTORY, 'icon.svg'), ICON);

  const listed: InstalledApp[] = await (await request.get('/api/apps')).json();
  expect(listed.map((app) => app.id)).toContain(APP);
});

Then('the launcher lists it and serves the icon out of its package', async ({ page, request }) => {
  await openConsole(page);
  await page.getByTestId('header_menu_toggle').click();

  const entry = page.getByTestId(`app_menu_app_${APP}`);
  await expect(entry).toContainText('Copiata a mano');

  await expect(entry.locator('img')).toHaveAttribute('src', `/api/apps/${APP}/icon`);

  const icon = await request.get(`/api/apps/${APP}/icon`);
  expect(icon.status()).toBe(200);
  expect(icon.headers()['content-type']).toBe('image/svg+xml');

  expect(icon.headers()['content-security-policy']).toContain("default-src 'none'");
  expect(icon.headers()['x-content-type-options']).toBe('nosniff');
  expect(await icon.text()).toBe(ICON);
});

Then('taking the directory away takes the application with it', async ({ request }) => {
  rmSync(DIRECTORY, { recursive: true, force: true });

  const listed: InstalledApp[] = await (await request.get('/api/apps')).json();
  expect(listed.map((app) => app.id)).not.toContain(APP);

  const gone = await request.get(`/api/apps/${APP}/icon`);
  expect(gone.status()).toBe(404);
});
