import { openConsole } from './auth_helpers';
import { randomUUID } from 'node:crypto';
import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
const { Given, When, Then } = createBdd();
Given('the mock backend is available', async ({ request }) => {
  const response = await request.get('/api/settings/general');
  expect(response.ok()).toBe(true);
  const general = await response.json();
  const reset = await request.put('/api/settings/general', {
    data: { ...general, theme: 'system', language: 'it' },
  });
  expect(reset.ok()).toBe(true);
});
When('I use the launcher with its mock backend', async ({ page, request }) => {
  let notificationId: number | undefined;
  const notificationTitle = `<img src=x onerror=alert(1)> ${randomUUID()}`;
  try {
    await openConsole(page);
    await expect
      .poll(() =>
        page
          .getByTestId('header_menu_toggle')
          .getByTestId('common_icon')
          .evaluate((element) => getComputedStyle(element).backgroundImage),
      )
      .not.toBe('none');

    await expect(page.getByTestId('header_process_toggle')).toHaveCount(0);

    const telemetry = await request.get('/api/system/telemetry');
    expect(telemetry.status()).toBe(404);

    await page.getByTestId('header_task_toggle').click();
    await expect(page.getByTestId('header_task_toggle')).toHaveAttribute('aria-expanded', 'true');
    const panel = page.getByTestId('task_panel');
    await expect
      .poll(() => panel.evaluate((element) => element.getBoundingClientRect().right))
      .toBe(1280);
    expect(
      await panel.evaluate((element) => element.getBoundingClientRect().width),
    ).toBeLessThanOrEqual(640);
    await panel.press('Escape');
    await expect(panel).toHaveCount(0);
    await expect(page.getByTestId('header_task_toggle')).toBeFocused();
    await page.getByTestId('header_task_toggle').click();
    await page.getByTestId('widget_drawer_backdrop').click({ position: { x: 100, y: 100 } });
    await expect(panel).toHaveCount(0);

    await expect(page.getByTestId('desktop_shortcut_nas-app-manager')).toHaveCount(1);
    await page.getByTestId('header_menu_toggle').click();
    await page.getByTestId('app_menu_app_nas-app-manager').click({ button: 'right' });
    await expect(page.getByTestId('menu_shortcut')).toHaveText('Rimuovi collegamento');
    await page.getByTestId('menu_shortcut').click();
    await page.getByTestId('app_menu_backdrop').click({ position: { x: 1000, y: 500 } });
    await expect(page.getByTestId('desktop_shortcut_nas-app-manager')).toHaveCount(0);
    await page.getByTestId('header_menu_toggle').click();
    await page.getByTestId('app_menu_app_nas-app-manager').click({ button: 'right' });
    await expect(page.getByTestId('menu_shortcut')).toHaveText('Aggiungi al desktop');
    await page.getByTestId('menu_shortcut').click();
    await page.getByTestId('app_menu_backdrop').click({ position: { x: 1000, y: 500 } });
    await page.getByTestId('desktop_shortcut_nas-app-manager').getByRole('button').click();
    await expect(page.getByTestId('app_tab_nas-app-manager')).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await page.getByTestId('header_menu_toggle').click();
    await page.getByTestId('app_menu_settings').click();
    await page.getByTestId('settings_theme').selectOption('light');

    await page.getByTestId('settings_language').selectOption('en');
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
    await page.getByTestId('app_tab_nas-app-manager').click();
    await expect(page.getByRole('heading', { name: 'Application manager' })).toBeVisible();
    await page.getByTestId('app_tab_nas-system-settings').click();
    await expect(page.getByTestId('settings_language')).toHaveValue('en');

    const created = await request.post('/api/notifications', {
      data: {
        title: notificationTitle,
        message: 'E2E example',
        source: 'system',
        tone: 'info',
      },
    });
    expect(created.ok()).toBe(true);
    notificationId = (await created.json()).id;
    await page.getByTestId('header_menu_toggle').click();
    await page.getByTestId('app_menu_app_nas-notification-manager').click();
    await expect(
      page
        .getByTestId(`notification_${notificationId}`)
        .getByText(notificationTitle, { exact: true }),
    ).toBeVisible();
    await expect(page.locator('img[onerror]')).toHaveCount(0);
    await page.getByTestId('app_tab_nas-system-settings').click();
    await page.getByTestId('settings_theme').selectOption('system');
    await page.getByTestId('settings_language').selectOption('it');
    await expect(page.locator('html')).toHaveAttribute('lang', 'it');
  } finally {
    if (notificationId !== undefined) {
      await request.delete(`/api/notifications/${notificationId}`, { data: {} });
    }
    const general = await (await request.get('/api/settings/general')).json();
    await request.put('/api/settings/general', {
      data: { ...general, theme: 'system', language: 'it' },
    });
  }
});
Then('the launcher fits on a mobile screen', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const tabs = page.getByTestId('app_tabs').locator('.app_tabs_scroll');
  await tabs.evaluate((element) => element.scrollTo(0, 0));
  await expect(page.getByTestId('app_tabs_overflow')).toBeVisible();
  expect(await tabs.evaluate((element) => getComputedStyle(element).scrollbarWidth)).toBe('none');
  await tabs.evaluate((element) => element.scrollTo(element.scrollWidth, 0));
  await expect(page.getByTestId('app_tabs_overflow')).toHaveCount(0);
  await page.setViewportSize({ width: 1280, height: 844 });
  await expect(page.getByTestId('app_tabs_overflow')).toHaveCount(0);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByTestId('header_desktop').click();
  await page.getByTestId('header_task_toggle').click();
  await expect(page.getByTestId('task_widget')).toBeVisible();
  await page.getByTestId('widget_close').click();
  await expect(page.getByTestId('task_panel')).toHaveCount(0);
  await page.getByTestId('header_task_toggle').click();
  await page.getByTestId('header_task_toggle').click();
  await expect(page.getByTestId('task_panel')).toHaveCount(0);
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
  await page.screenshot({ path: '/tmp/nas-os-mobile.png', fullPage: true });
});

Then(
  'app management shows centered loading and temporary update feedback',
  async ({ page, request }) => {
    await page.route('**/api/storage/disks/external', (route) =>
      route.fulfill({
        json: [{ device: 'sdb', name: 'Backup USB', locked: false, canEject: true }],
      }),
    );
    await openConsole(page);
    const app = {
      id: 'demo',
      name: 'Demo',
      info: '',
      system: false,
      version: '1.0.0',
      running: false,
    };
    let release!: () => void;
    const waiting = new Promise<void>((resolve) => {
      release = resolve;
    });
    let notificationId: string | undefined;
    await page.route('**/api/apps/credentials', (route) =>
      route.fulfill({ json: { configured: true, packagesEnabled: true, appsManaged: true } }),
    );
    await page.route('**/api/apps/installed', async (route) => {
      await waiting;
      await route.fulfill({ json: [app] });
    });
    try {
      await page.getByTestId('header_menu_toggle').click();
      await page.getByTestId('app_menu_app_nas-app-manager').click();
      const loading = page.getByTestId('app_manager_loading');
      await expect(loading).toBeVisible();
      for (const control of ['refresh', 'view_list', 'view_preview']) {
        await expect(page.getByTestId(`app_manager_${control}`)).toBeDisabled();
      }
      await expect(page.getByTestId('app_manager_catalog')).toHaveCount(0);
      const bounds = await loading.boundingBox();
      const panel = await page.getByTestId('app_tab_nas-app-manager').evaluate(() => {
        const box = document.querySelector('.app_view')!.getBoundingClientRect();
        return { x: box.x, y: box.y, width: box.width, height: box.height };
      });
      expect(bounds!.height).toBeGreaterThan(panel.height / 2);
      const ring = await loading.locator('.common_spinner_ring').boundingBox();
      expect(ring!.width).toBeGreaterThanOrEqual(48);
      expect(Math.abs(ring!.x + ring!.width / 2 - (panel.x + panel.width / 2))).toBeLessThan(2);
      release();
      await expect(loading).toHaveCount(0);
      for (const control of ['refresh', 'view_list', 'view_preview']) {
        await expect(page.getByTestId(`app_manager_${control}`)).toBeEnabled();
      }
      await page.route('**/api/apps/updates', async (route) => {
        const response = await request.post('/api/notifications', {
          data: {
            source: 'nas-app-manager',
            title: 'Updates available',
            message: 'Demo has an update',
            tone: 'info',
            key: 'appUpdatesAvailable',
            params: { count: '1', apps: 'Demo' },
          },
        });
        expect(response.ok()).toBe(true);
        notificationId = (await response.json()).id;
        await route.fulfill({
          json: [
            {
              ...app,
              repository: 'example/demo',
              installedVersion: '1.0.0',
              latestVersion: '2.0.0',
              updateAvailable: true,
            },
          ],
        });
      });
      await page.getByTestId('app_manager_check').click();
      const toast = page
        .getByTestId('notification_toasts')
        .getByText('Aggiornamenti disponibili', { exact: true });
      await expect(toast).toBeVisible();
      await expect(page.getByTestId('app_manager_updates')).toHaveCount(0);
      await expect(page.getByTestId('app_manager_update_available')).toBeVisible();
      await page.mouse.move(0, 0);
      await expect(toast).toHaveCount(0, { timeout: 10_000 });
      await page.getByTestId('header_menu_toggle').click();
      await page.getByTestId('app_menu_app_nas-notification-manager').click();
      const active = page.getByTestId('app_tab_nas-notification-manager');
      const inactive = page.getByTestId('app_tab_nas-app-manager');
      expect(
        await active.evaluate((element) => getComputedStyle(element.parentElement!).boxShadow),
      ).toBe('none');
      expect(
        await active.evaluate((element) => getComputedStyle(element.parentElement!).borderTopWidth),
      ).toBe('1px');
      expect(await active.evaluate((element) => getComputedStyle(element).boxShadow)).toBe('none');

      const tab = (locator: typeof active, property: 'backgroundColor' | 'borderTopWidth') =>
        locator.evaluate(
          (element, name) => getComputedStyle(element.parentElement!)[name],
          property,
        );
      const activeBackground = await tab(active, 'backgroundColor');
      await active.hover();
      expect(await active.evaluate((element) => getComputedStyle(element).boxShadow)).toBe('none');
      await expect.poll(() => tab(active, 'backgroundColor')).toBe(activeBackground);
      expect(await tab(active, 'borderTopWidth')).toBe('1px');
      const resting = await tab(inactive, 'backgroundColor');
      await inactive.hover();
      await expect.poll(() => tab(inactive, 'backgroundColor')).not.toBe(resting);
      expect(await tab(inactive, 'borderTopWidth')).toBe('1px');
      await page.mouse.move(0, 0);
      expect(
        await inactive.evaluate((element) => getComputedStyle(element.parentElement!).boxShadow),
      ).toBe('none');
      await expect
        .poll(() =>
          inactive.locator('img').evaluate((element) => getComputedStyle(element).opacity),
        )
        .toBe('0.65');
      await inactive.click();
      expect(
        await inactive.locator('img').evaluate((element) => getComputedStyle(element).opacity),
      ).toBe('1');
      await page.screenshot({ path: '/tmp/nas-os-active-tabs.png' });
      await page.route('**/api/apps/available', (route) => route.fulfill({ json: [] }));
      await page.getByRole('button', { name: 'Disponibili', exact: true }).click();
      await expect(page.getByTestId('app_manager_empty')).toBeVisible();
      for (const control of ['refresh', 'view_list', 'view_preview']) {
        await expect(page.getByTestId(`app_manager_${control}`)).toBeDisabled();
      }
      await expect(page.getByTestId('app_manager_settings_open')).toBeEnabled();
      await page
        .getByTestId('header_external_disks')
        .getByTestId('common_dropdown_trigger')
        .click();
      const eject = page.getByTestId('header_eject_sdb');
      await expect(eject).toBeVisible();
      await expect
        .poll(() =>
          page
            .getByTestId('header_external_disks')
            .getByTestId('common_dropdown_content')
            .evaluate((element) => getComputedStyle(element).opacity),
        )
        .toBe('1');
      const originalTheme = await page.locator('html').getAttribute('data-theme');
      const colors: string[] = [];
      try {
        for (const theme of ['dark', 'light']) {
          await page.locator('html').evaluate((element, value) => {
            element.setAttribute('data-theme', value);
          }, theme);
          const color = await eject.evaluate((element) => getComputedStyle(element).color);
          const icon = eject.getByTestId('common_icon');
          expect(await icon.evaluate((element) => getComputedStyle(element).backgroundColor)).toBe(
            color,
          );
          expect(await icon.evaluate((element) => getComputedStyle(element).maskImage)).not.toBe(
            'none',
          );
          colors.push(color);
          await page.screenshot({ path: `/tmp/nas-os-empty-manager-eject-${theme}.png` });
        }
        expect(colors[0]).not.toBe(colors[1]);
      } finally {
        await page.locator('html').evaluate((element, value) => {
          if (value === null) {
            element.removeAttribute('data-theme');
          } else {
            element.setAttribute('data-theme', value);
          }
        }, originalTheme);
      }
    } finally {
      release();
      if (notificationId !== undefined) {
        await request.delete(`/api/notifications/${notificationId}`, { data: {} });
      }
      await page.unroute('**/api/apps/credentials');
      await page.unroute('**/api/apps/installed');
      await page.unroute('**/api/apps/updates');
      await page.unroute('**/api/apps/available');
      await page.unroute('**/api/storage/disks/external');
    }
  },
);
