import { openConsole } from './auth_helpers';
import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
import type { AppOffer } from '@/apps/nas-app-manager/types';
import type { ManagedApp } from '@/apps/nas-app-manager/types';
import type { UserTask } from '@/types/widgets/tasks';

const { Given, When, Then } = createBdd();

const APP = 'fileExplorer';
const VERSION = '1.2.0';

Given('the catalogue offers an application', async ({ request }) => {
  await request.post(`/api/apps/${APP}/uninstall`, { data: {} });

  const offered: AppOffer[] = await (await request.get('/api/apps/available')).json();
  expect(offered.map((app) => app.id)).toContain(APP);
  expect(offered.find((app) => app.id === APP)?.repository).toBe('example/nas-file-explorer');
});

When('I install it from the application manager', async ({ page }) => {
  await openConsole(page);
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId('app_menu_app_nas-app-manager').click();

  await page.getByTestId('app_manager_settings_open').click();
  await expect(page.getByTestId('app_manager_shortcuts_input')).toBeChecked();
  await page.getByTestId('app_manager_settings_save').click();
  await expect
    .poll(
      async () => (await (await page.request.get('/api/settings/general')).json()).createShortcut,
    )
    .toBe(true);
  await expect(page.getByTestId('app_manager_settings')).toHaveCount(0);

  await page.getByTestId('app_manager_check').click();
  await expect(page.getByTestId('app_manager_check')).toBeEnabled();

  await page.getByRole('button', { name: 'Disponibili' }).click();
  await page.getByTestId(`app_manager_app_${APP}`).click();
  await page.getByRole('button', { name: 'Installa', exact: true }).click();
  await page.locator('.common_modal_footer button').last().click();
});

Then('the package is fetched, unpacked and registered', async ({ page, request }) => {
  await expect
    .poll(
      async () => {
        const register: UserTask[] = await (await request.get('/api/tasks')).json();
        return register.find((entry) => entry.id === `install-${APP}`)?.state;
      },
      { timeout: 20_000 },
    )
    .toBe('done');

  await expect(
    page.getByTestId(/^notification_toast_/).filter({ hasText: 'Applicazione installata' }),
  ).toBeVisible({ timeout: 20_000 });

  await page.getByTestId('header_task_toggle').click();
  await page.getByTestId('task_view_finished').click();
  await expect(page.getByTestId(`task_state_install-${APP}`)).toHaveText('Conclusa');
  await expect(page.getByTestId(`task_install-${APP}`)).toContainText(VERSION);

  const installed: ManagedApp[] = await (await request.get('/api/apps/installed')).json();
  expect(installed.find((app) => app.id === APP)?.version).toBe(VERSION);
  expect(installed.find((app) => app.id === APP)?.system).toBe(true);

  const shortcuts = await (await request.get('/api/desktop/shortcuts')).json();
  expect(shortcuts).toEqual(
    expect.arrayContaining([expect.objectContaining({ kind: 'app', target: APP })]),
  );
  await page.getByTestId('widget_close').click();
  await page.getByTestId('header_desktop').click();
  await expect(page.getByTestId(`desktop_shortcut_${APP}`)).toBeVisible();

  const offered: AppOffer[] = await (await request.get('/api/apps/available')).json();
  expect(offered.map((app) => app.id)).not.toContain(APP);
});

Then('updating it preserves the desktop choices', async ({ request }) => {
  expect((await request.delete(`/api/desktop/shortcuts/${APP}`, { data: {} })).ok()).toBe(true);
  expect((await request.post(`/api/apps/${APP}/update`, { data: {} })).ok()).toBe(true);
  await expect
    .poll(
      async () => {
        const tasks: UserTask[] = await (await request.get('/api/tasks')).json();
        return tasks.find((task) => task.id === `install-${APP}`)?.state;
      },
      { timeout: 20_000 },
    )
    .toBe('done');
  const shortcuts = await (await request.get('/api/desktop/shortcuts')).json();
  expect(shortcuts).not.toEqual(expect.arrayContaining([expect.objectContaining({ target: APP })]));
});

Then('it opens in a frame served by its own process', async ({ page }) => {
  await openConsole(page);
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId(`app_menu_app_${APP}`).click();

  const frame = page.getByTestId(`app_frame_${APP}`);
  const source = await frame.getAttribute('src');
  expect(source).not.toBeNull();
  const frameUrl = new URL(source!, page.url());
  expect(frameUrl.pathname).toBe(`/apps/${APP}/`);
  expect(frameUrl.searchParams.get('launch')).toBeTruthy();

  const inside = page.frameLocator(`[data-testid="app_frame_${APP}"]`);
  await expect(inside.getByTestId('app_heading')).toBeVisible();
  await expect(inside.getByTestId('app_user')).toHaveText('local');

  await expect(inside.getByTestId('app_theme')).toHaveText(/light|dark/);
  await expect
    .poll(async () => {
      const [shown, received] = await Promise.all([
        page.locator('html').getAttribute('lang'),
        inside.getByTestId('app_locale').textContent(),
      ]);
      return shown === received;
    })
    .toBe(true);
});

Then('it can ask the launcher for a notification', async ({ page, request }) => {
  await page.frameLocator(`[data-testid="app_frame_${APP}"]`).getByTestId('app_notify').click();

  await expect
    .poll(async () => {
      const log: { title: string; source: string }[] = await (
        await request.get('/api/notifications')
      ).json();
      return log.find((entry) => entry.title === 'Copia completata')?.source;
    })
    .toBe(APP);
});

Then('removing it takes the package and the row away', async ({ page, request }) => {
  const removed = await request.post(`/api/apps/${APP}/uninstall`, { data: {} });
  expect(removed.ok()).toBe(true);

  await expect
    .poll(async () => {
      const again: AppOffer[] = await (await request.get('/api/apps/available')).json();
      return again.map((app) => app.id);
    })
    .toContain(APP);

  const orphan = await page.goto(`/apps/${APP}/`);
  expect(orphan?.status()).toBe(404);
  await expect(page.locator('body')).toContainText('not installed');

  const general = await (await request.get('/api/settings/general')).json();
  await request.put('/api/settings/general', { data: { ...general, createShortcut: false } });
});
