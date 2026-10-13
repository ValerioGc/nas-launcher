import { openConsole } from './auth_helpers';
import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
import type { AppOffer } from '@/apps/nas-app-manager/types';
import type { ManagedApp } from '@/apps/nas-app-manager/types';
import type { UserTask } from '@/types/widgets/tasks';

const { Given, When, Then } = createBdd();

/**
 * The whole package path, for real, against a forge on loopback: the release is looked up, the
 * asset is downloaded, its published checksum is checked, `tar` unpacks it and the register is
 * written. Nothing here is simulated except the repository the package comes from.
 */
const APP = 'fileExplorer';
const VERSION = '1.2.0';

Given('the catalogue offers an application', async ({ request }) => {
  // Left over from an earlier run: the scenario starts from nothing installed.
  await request.post(`/api/apps/${APP}/uninstall`, { data: {} });

  const offered: AppOffer[] = await (await request.get('/api/apps/available')).json();
  expect(offered.map((app) => app.id)).toContain(APP);
  expect(offered.find((app) => app.id === APP)?.repository).toBe('example/nas-file-explorer');
});

When('I install it from the application manager', async ({ page }) => {
  await openConsole(page);
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId('app_menu_app_nas-app-manager').click();

  // A new application gets its desktop shortcut by default, which the package installer reads.
  await page.getByTestId('app_manager_settings_open').click();
  await expect(page.getByTestId('app_manager_shortcuts_input')).toBeChecked();
  await page.getByTestId('app_manager_settings_save').click();
  await expect
    .poll(
      async () => (await (await page.request.get('/api/settings/general')).json()).createShortcut,
    )
    .toBe(true);
  await expect(page.getByTestId('app_manager_settings')).toHaveCount(0);

  // The check is a button: opening the page reaches no repository on its own.
  await page.getByTestId('app_manager_check').click();
  await expect(page.getByTestId('app_manager_check')).toBeEnabled();

  await page.getByRole('button', { name: 'Disponibili' }).click();
  await page.getByTestId(`app_manager_app_${APP}`).click();
  await page.getByRole('button', { name: 'Installa', exact: true }).click();
  await page.locator('.common_modal_footer button').last().click();
});

Then('the package is fetched, unpacked and registered', async ({ page, request }) => {
  // The operation is a real one in the register, and it ends. The register is the authority here,
  // so it is what is waited on: the widget refreshes on a timer that another scenario is allowed
  // to change, and waiting on a poll would make this scenario depend on the order they run in.
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

  // Opening the widget mounts it, so what it shows is read straight away and not on the next tick.
  await page.getByTestId('header_task_toggle').click();
  await page.getByTestId('task_view_finished').click();
  await expect(page.getByTestId(`task_state_install-${APP}`)).toHaveText('Conclusa');
  await expect(page.getByTestId(`task_install-${APP}`)).toContainText(VERSION);

  // The register carries the version that was installed, not just the fact that something was.
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

  // And it is no longer on offer, because it is here.
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

/**
 * The installed application is a process of its own on a unix socket, and the launcher serves it
 * under `/apps/<id>/`, on the address the console was opened at: nothing on the network has to
 * know about it. What is checked here is that the page really arrives through the proxy, and that
 * the user the application is told about is the launcher's word.
 */
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

  // The shell and the application talk across the two origins: the application says it is ready
  // and the launcher answers with the theme and the language it is being shown in. Another
  // scenario may be switching the language of the same backend meanwhile, so what is compared is
  // the language of the page, not a fixed one: the application follows it either way.
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

/**
 * The other direction. An application on its own origin cannot call the launcher's API from a
 * page (the browser would not let it) so asking for a notification is a message, and the
 * launcher signs it with the frame it came from rather than with anything the message said.
 */
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

  // With the package gone its address serves nothing, and the launcher says so.
  const orphan = await page.goto(`/apps/${APP}/`);
  expect(orphan?.status()).toBe(404);
  await expect(page.locator('body')).toContainText('not installed');

  const general = await (await request.get('/api/settings/general')).json();
  await request.put('/api/settings/general', { data: { ...general, createShortcut: false } });
});
