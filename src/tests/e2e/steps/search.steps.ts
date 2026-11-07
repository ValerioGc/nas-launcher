import { openConsole } from './auth_helpers';
import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
const { Given, When, Then } = createBdd();

Given('I open the search application', async ({ page }) => {
  await page.route('**/api/search?**', () => {
    throw new Error('File searches must go to the providing application');
  });
  await openConsole(page);
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId('app_menu_app_nas-global-search').click();
});

Given('an application provides the search with long file names', async ({ page }) => {
  await page.route('**/api/capabilities/search', (route) =>
    route.fulfill({ json: [{ app: 'nas-file-manager', endpoint: '/api/search', version: 1 }] }),
  );
  const long = 'relazione trimestrale del reparto amministrazione e contabilita '.repeat(3);
  await page.route('**/apps/nas-file-manager/api/search?**', (route) =>
    route.fulfill({
      json: [1, 2, 3].map((index) => ({
        id: `report-${index}`,
        name: `${long}${index}.pdf`,
        path: `/srv/dati/archivio/${long.replaceAll(' ', '/')}${index}.pdf`,
        type: 'document',
        extension: 'pdf',
        size: 1048576 * index,
        modified: '2026-11-07T10:30:00Z',
        app_id: 'nas-file-manager',
        target: `?path=/srv/dati/${index}`,
      })),
    }),
  );
});

Then('the table of files fits its width', async ({ page }) => {
  const results = page.getByTestId('search_results');
  await expect(page.getByTestId('search_result_nas-file-manager:report-1')).toBeVisible();
  await expect(page.getByRole('columnheader')).toHaveCount(5);
  expect(await results.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
    true,
  );
  const table = await page.getByRole('table').boundingBox();
  const area = await results.boundingBox();
  expect(Math.round(table?.width ?? 0)).toBe(Math.round(area?.width ?? -1));
});

When('I reset the file search filters', async ({ page }) => {
  await expect(page.getByTestId('search_reset_filters')).toBeDisabled();
  await page.getByTestId('search_query').fill('report');
  await page.getByTestId('search_type').selectOption('document');
  await page.getByTestId('search_extension').fill('.PDF');
  await page.getByTestId('search_reset_filters').click();
});

Then(
  'the query is preserved and the launcher does not handle file searches',
  async ({ page, request }) => {
    await expect(page.getByTestId('search_query')).toHaveValue('report');
    await expect(page.getByTestId('search_type')).toHaveValue('');
    await expect(page.getByTestId('search_extension')).toHaveValue('');
    await expect(page.getByTestId('search_reset_filters')).toBeDisabled();
    await expect(page.getByTestId('search_error')).toBeVisible();
    const response = await request.get('/api/search?q=report');
    expect(response.status()).toBe(404);
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.getByTestId('search_reset_filters')).toBeVisible();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  },
);
