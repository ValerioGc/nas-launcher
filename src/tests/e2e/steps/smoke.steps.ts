import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';

const { Given, Then } = createBdd();

Given('I open the application', async ({ page }) => {
  await page.goto('/');
});

Then('the page has a title', async ({ page }) => {
  await expect(page).toHaveTitle(/\S/);
});

Then('search engines are asked to stay away', async ({ page, request }) => {
  await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', /noindex/);
  expect(await (await request.get('/robots.txt')).text()).toContain('Disallow: /');
  const api = await request.get('/api/status');
  expect(api.headers()['x-robots-tag']).toBe(
    'noindex, nofollow, noarchive, nosnippet, noimageindex',
  );
  await expect(page.locator('meta[name="referrer"]')).toHaveAttribute('content', 'no-referrer');
});
