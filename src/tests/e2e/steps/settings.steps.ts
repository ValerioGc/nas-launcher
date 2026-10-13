import { openConsole } from './auth_helpers';
import { handleNotificationToasts } from './toast_helpers';
import { readFileSync } from 'node:fs';
import { expect, type Page } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
const { Given, When, Then } = createBdd();

/**
 * The settings are the one piece of data already saved in the database: the scenario changes
 * them, checks that the launcher applies them, and puts them back as they were, so a second run
 * starts clean.
 */
When('I change the general settings and the widget refresh', async ({ page, request }) => {
  const toast = await handleNotificationToasts(page);
  await openConsole(page);
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId('app_menu_settings').click();

  // General: type of theme, the theme of that type and background pattern.
  await expect(page.getByRole('heading', { name: 'Generali' })).toBeVisible();
  await page.getByTestId('settings_theme').selectOption('dark');
  await page.getByTestId('settings_theme_name').selectOption('midnight');
  // Saved as they change: what the launcher answered to the save that holds them, not what it holds
  // a moment later, as another scenario may be writing the same section meanwhile.
  const saving = page.waitForResponse(
    (answer) =>
      answer.url().endsWith('/api/settings/general') &&
      answer.request().method() === 'PUT' &&
      answer.request().postDataJSON()?.background?.direction === 90,
  );
  await page.getByTestId('settings_background_pattern').selectOption('rings');
  await page.getByText('A destra', { exact: true }).click();
  const saved = await saving;
  expect(saved.ok()).toBe(true);
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(page.locator('html')).toHaveAttribute('data-palette', 'midnight');
  const general = saved.request().postDataJSON();
  expect(general.background.pattern).toBe('rings');
  expect(general.themes.dark).toBe('midnight');

  // Power: a rule is added from its own fields, shown in the table, and removed again. Adding
  // waits for the rules to be on, and their state is put back at the end.
  await page.getByRole('button', { name: 'Alimentazione', exact: true }).click();
  const rulesOn = page.getByTestId('power_enabled_input');
  const wasOn = await rulesOn.isChecked();
  if (!wasOn) {
    await expect(page.getByTestId('power_rule_open')).toBeDisabled();
    await page.getByTestId('power_enabled').click();
  }
  await page.getByTestId('power_rule_open').click();
  await page.getByTestId('power_rule_day').selectOption('sunday');
  await page.getByTestId('power_rule_time').fill('03:15');
  await page.getByTestId('power_rule_action_restart').click();
  await page.getByTestId('power_rule_add').click();
  await expect(page.getByTestId('power_rules')).toContainText('03:15');
  const power = await (await request.get('/api/settings/power')).json();
  expect(power.rules.at(-1)).toMatchObject({ day: 'sunday', time: '03:15', action: 'restart' });
  await page.getByRole('button', { name: 'Rimuovi Domenica 03:15' }).click();
  await expect
    .poll(async () => (await (await request.get('/api/settings/power')).json()).rules.length)
    .toBe(power.rules.length - 1);
  if (!wasOn) {
    await page.getByTestId('power_enabled').click();
    await expect
      .poll(async () => (await (await request.get('/api/settings/power')).json()).enabled)
      .toBe(false);
  }

  // Updates: the Debian list is the machine's own, read only without NAS_APT_MODE, and the token
  // of the launcher's repository shows whether it is there, never what it is.
  await page.getByRole('button', { name: 'Aggiornamenti', exact: true }).click();
  await expect(page.getByTestId('system_updates_readonly')).toContainText(
    'non disponibile in questa installazione',
  );
  // The check, or the installation when the machine has packages to update: either is locked.
  await expect(page.getByTestId(/^system_updates_(refresh|apply)$/)).toBeDisabled();
  // The launcher of the scenarios has a token of its own, which the page never shows.
  await expect(page.getByTestId('launcher_token').getByRole('img')).toHaveAccessibleName(
    'Token configurato',
  );
  await expect(page.getByTestId('launcher_token')).not.toContainText('e2e');

  // The refresh interval of the tasks widget is changed from the widget itself.
  //
  // It is put back straight away, and not in the cleanup at the end of the scenario: the interval
  // is a setting of the whole backend, which every scenario shares, and one of them waits for a
  // widget to refresh. Leaving it at thirty seconds for the length of this scenario made that one
  // fail depending on which ran first.
  // Clicking a toast outside the widget would close it while its controls are being used.
  await page.removeLocatorHandler(toast);
  await page.getByTestId('header_task_toggle').click();
  await page.getByTestId('widget_settings_toggle').click();
  await page.getByTestId('widget_polling').fill('30');
  await page.getByTestId('widget_settings_save').click();
  await expect
    .poll(async () => (await (await request.get('/api/settings/widgets')).json()).pollingSeconds)
    .toBe(30);
  await request.put('/api/settings/widgets', { data: { pollingSeconds: 10 } });
  await page.getByTestId('widget_close').click();
});

Then(
  'the network section shows the connection and its wireless networks',
  async ({ page, request }) => {
    try {
      await handleNotificationToasts(page);
      await page.getByTestId('header_menu_toggle').click();
      await page.getByTestId('app_menu_settings').click();
      await page.getByRole('button', { name: 'Rete', exact: true }).click();
      // The backend reads the network of the machine, and which one it is is not known here: the
      // tab has to show the one the backend declares, not a choice made in advance.
      const network = await (await request.get('/api/network')).json();
      const expected =
        network.connection.kind === 'wifi' ? (network.connection.ssid ?? 'Wi-Fi') : 'Rete via cavo';
      await expect(page.getByTestId('network_name')).toHaveText(expected);
      await expect(page.getByTestId('network_details')).toContainText(network.connection.interface);
      await expect(page.getByTestId('network_details')).toContainText(network.connection.ipv4);

      // The scan really does ask the antenna to look again.
      await expect(page.getByTestId('network_scan')).toBeEnabled();
      const [scan] = await Promise.all([
        page.waitForResponse(
          (answer) =>
            answer.url().endsWith('/api/network/scan') && answer.request().method() === 'POST',
        ),
        page.getByTestId('network_scan').click(),
      ]);
      expect(scan.status()).toBe(200);

      // Changing network is offered only where it does something: in development it does not.
      if (!network.canConnect) {
        await expect(page.getByTestId('network_readonly')).toBeVisible();
        await expect(page.locator('[data-testid^="network_connect_"]')).toHaveCount(0);
      }
    } finally {
      const general = await (await request.get('/api/settings/general')).json();
      await request.put('/api/settings/general', {
        data: {
          ...general,
          theme: 'system',
          themes: { light: 'default', dark: 'default' },
          language: 'it',
          createShortcut: false,
          desktopIconSize: 'large',
          background: { ...general.background, pattern: 'multiple' },
        },
      });
      await request.put('/api/settings/widgets', { data: { pollingSeconds: 10 } });
    }
  },
);

When('I open the recovery section of the settings', async ({ page }) => {
  await openConsole(page);
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId('app_menu_settings').click();
  await page.getByRole('button', { name: 'Ripristino e reset', exact: true }).click();
  await expect(page.getByTestId('factory_reset')).toBeVisible();
});

// The development launcher never resets: NAS_RESET_MODE is simulated there.
Then('the factory reset says it is disabled on this installation', async ({ page }) => {
  await expect(page.getByTestId('factory_reset_unavailable')).toContainText(
    'Ripristino non disponibile',
  );
  await expect(page.getByTestId('factory_reset_button')).toBeDisabled();
});

/** A NAS with the wizard installed, behind a gateway that refuses the password. */
Given('the NAS can be reset but the gateway refuses the password', async ({ page }) => {
  await page.route('**/api/system/reset', (route) =>
    route.request().method() === 'POST'
      ? route.abort()
      : route.fulfill({ json: { available: true, reason: null } }),
  );
  await page.route('**/auth/confirm', (route) =>
    route.fulfill({ status: 403, json: { error: 'wrong password' } }),
  );
});

When('I confirm the factory reset with a wrong password', async ({ page }) => {
  await page.getByTestId('factory_reset_button').click();
  await expect(page.getByTestId('factory_reset_confirm')).toBeDisabled();
  await page.getByTestId('factory_reset_password').fill('not the one');
  await page.getByTestId('factory_reset_confirm').click();
});

Then('the reset is not started and the password is asked again', async ({ page }) => {
  await expect(page.getByTestId('factory_reset_failed')).toHaveText('La password non è corretta.');
  await expect(page.getByTestId('factory_reset_password')).toHaveValue('');
  await expect(page.getByTestId('factory_reset_running')).toHaveCount(0);
});

const ACCOUNT_PASSWORD = 'long enough';
const FILE_PASSWORD = 'only for the file';
const exportedConfig = new WeakMap<Page, unknown>();
const importedConfig = new WeakMap<Page, unknown>();
const exportedFile = new WeakMap<Page, string>();

/**
 * The gateway of the NAS, in front of the development launcher: the right password gives a code,
 * and the code lets the export through, named as the gateway names it. The import is caught
 * before the launcher: the scenarios share one database, and replacing it would disturb the others.
 */
Given('the gateway checks the password as the NAS does', async ({ page }) => {
  await page.route('**/auth/confirm', (route) =>
    route.request().postDataJSON().password === ACCOUNT_PASSWORD
      ? route.fulfill({ json: { code: 'e2e' } })
      : route.fulfill({ status: 403, json: { error: 'wrong password' } }),
  );
  await page.route('**/api/system/config', async (route) => {
    const request = route.request();
    if (request.headers()['x-nas-confirmation'] !== 'e2e') {
      await route.fulfill({ status: 403, json: { error: 'confirm the password first' } });
    } else if (request.method() === 'GET') {
      const answer = await route.fetch({
        headers: { ...request.headers(), 'x-nas-reauthenticated': 'admin' },
      });
      exportedConfig.set(page, await answer.json());
      await route.fulfill({ response: answer });
    } else {
      importedConfig.set(page, request.postDataJSON());
      await route.fulfill({ json: request.postDataJSON() });
    }
  });
});

When('I export the configuration encrypted with a password for the file', async ({ page }) => {
  await page.getByTestId('config_export_button').click();
  await page.getByTestId('config_password').fill(ACCOUNT_PASSWORD);
  await page.getByTestId('config_file_password').fill(FILE_PASSWORD);
  const download = page.waitForEvent('download');
  await page.getByTestId('config_transfer_confirm').click();
  const file = await (await download).path();
  await expect(page.getByTestId('config_exported')).toContainText('nas-os-config-');
  expect(readFileSync(file, 'utf8')).not.toContain('"theme"');
  exportedFile.set(page, file);
  await page.getByTestId('common_modal_close').click();
});

When('I import the exported file', async ({ page }) => {
  await page.getByTestId('config_import_button').click();
  await page.getByTestId('config_file').setInputFiles(exportedFile.get(page) ?? '');
  await page.getByTestId('config_file_password').fill(FILE_PASSWORD);
  await page.getByTestId('config_password').fill(ACCOUNT_PASSWORD);
  await page.getByTestId('config_transfer_confirm').click();
});

Then('the launcher receives the configuration it exported', async ({ page }) => {
  await expect.poll(() => importedConfig.get(page)).toEqual(exportedConfig.get(page));
  expect(JSON.stringify(exportedConfig.get(page))).not.toContain('nasName');
});
