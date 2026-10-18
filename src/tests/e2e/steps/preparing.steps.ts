import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';

const { Given, Then } = createBdd();

const explorer = { id: 'app:nas-file-manager', kind: 'app', name: 'Explorer', detail: '' };

Given('the wizard has just configured the NAS', async ({ page }) => {
  const configuredAt = Date.now();
  await page.context().clearCookies();
  await page.route('**/api/setup', (route) => {
    const ready = Date.now() - configuredAt > 3000;
    return route.fulfill({
      json: { completed: true, ready, steps: [{ ...explorer, state: ready ? 'done' : 'running' }] },
    });
  });
});
Given('the wizard has not configured the NAS', async ({ page }) => {
  await page.context().clearCookies();
  await page.route('**/api/setup', (route) =>
    route.fulfill({ json: { completed: false, ready: false, steps: [] } }),
  );
});
Then('the NAS is prepared before the sign-in', async ({ page }) => {
  await expect(page).toHaveURL(/\/preparing$/);
  await expect(page.getByTestId('preparing_step_app:nas-file-manager')).toBeVisible();
});
Then('the page asks to run the wizard', async ({ page }) => {
  await expect(page).toHaveURL(/\/preparing$/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(
    /NAS non configurato|NAS not configured/,
  );
});
