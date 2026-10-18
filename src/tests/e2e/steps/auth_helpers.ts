import { expect, type Page } from '@playwright/test';

export const ACCOUNT = { username: 'admin', password: 'long enough' };

export async function configureNas(page: Page) {
  const response = await page.request.post('/api/setup/complete', {
    data: { language: 'it', theme: 'system' },
  });
  expect([200, 409]).toContain(response.status());
  await expect
    .poll(async () => (await (await page.request.get('/api/setup')).json()).ready, {
      timeout: 20_000,
    })
    .toBe(true);
  if (response.status() === 200) {
    const setup = await (await page.request.get('/api/setup')).json();
    expect(setup.steps).toEqual([
      expect.objectContaining({ id: 'app:fileExplorer', state: 'done' }),
    ]);
    const notices = await (
      await page.request.get('/api/notifications?source=nas-app-manager')
    ).json();
    expect(notices).toEqual([]);
    const shortcuts = await (await page.request.get('/api/desktop/shortcuts')).json();
    expect(shortcuts).toEqual(
      expect.arrayContaining([expect.objectContaining({ target: 'fileExplorer' })]),
    );
    expect(shortcuts).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ target: 'nas-system-terminal' })]),
    );
    const installed = await (await page.request.get('/api/apps/installed')).json();
    expect(installed).toEqual(
      expect.arrayContaining([expect.objectContaining({ id: 'fileExplorer', system: true })]),
    );
  }
}

export async function openConsole(page: Page) {
  await configureNas(page);
  await page.goto('/login');
  await page.getByTestId('login-username').fill(ACCOUNT.username);
  await page.getByTestId('login-password').fill(ACCOUNT.password);
  await page.getByTestId('login-submit').click();
  await expect(page).toHaveURL(/\/console$/);
}
