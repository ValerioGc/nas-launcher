import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
import type { AppOffer } from '@/apps/nas-app-manager/types';

const { When, Then } = createBdd();

const APP = 'photoAlbum';

async function offeredIds(request: import('@playwright/test').APIRequestContext) {
  const offered: AppOffer[] = await (await request.get('/api/apps/available')).json();
  return offered.map((app) => app.id);
}

When('I add an application to the catalogue from the manager settings', async ({ page }) => {
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId('app_menu_app_nas-app-manager').click();
  await page.getByTestId('app_manager_settings_open').click();

  await page.getByTestId('catalogue_add').click();
  await page.getByTestId('catalogue_id').fill(APP);
  await page.getByTestId('catalogue_name').fill('Album');
  await page.getByTestId('catalogue_repository').fill('example/photo-album');
  await page.getByTestId('catalogue_asset').fill('album-{version}.tar.gz');
  await page.getByTestId('catalogue_apply').click();
  await expect(page.getByTestId(`catalogue_row_${APP}`)).toBeVisible();
  await page.getByTestId('app_manager_settings_save').click();
  // Saved, the settings close. No toast to wait for: the scenario of the packages saved these
  // settings moments before, and an unread notice of the same section is refreshed, not repeated.
  await expect(page.getByTestId('app_manager_settings')).toHaveCount(0);
});

// The rest of the catalogue stays where it was: another scenario installs from it.
Then('the new application is on offer', async ({ request }) => {
  const ids = await offeredIds(request);
  expect(ids).toContain(APP);
  expect(ids).toContain('fileExplorer');
});

When('I remove it from the catalogue', async ({ page }) => {
  await page.getByTestId('app_manager_settings_open').click();
  await page.getByTestId(`catalogue_remove_${APP}`).click();
  await expect(page.getByTestId(`catalogue_row_${APP}`)).toBeVisible();
  await page.getByTestId('app_manager_cancel').click();
  await expect(page.getByTestId(`catalogue_row_${APP}`)).toBeVisible();
  await page.getByTestId(`catalogue_remove_${APP}`).click();
  await page.getByTestId('app_manager_confirm').click();
  await expect(page.getByTestId(`catalogue_row_${APP}`)).toHaveCount(0);
  // Enter in a field saves too: the settings are a form.
  await page.getByTestId('app_manager_token_input').press('Enter');
  await expect(page.getByTestId('app_manager_settings')).toHaveCount(0);
});

// The toast of the first save may still be on screen: the launcher is what is waited on.
Then('it is no longer on offer', async ({ request }) => {
  await expect.poll(() => offeredIds(request)).not.toContain(APP);
});
