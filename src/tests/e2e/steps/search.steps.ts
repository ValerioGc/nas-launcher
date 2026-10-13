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
