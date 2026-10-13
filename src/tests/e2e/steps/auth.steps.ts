import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
import { ACCOUNT, configureNas } from './auth_helpers';

const { Given, When, Then } = createBdd();

Given('I have no gateway session', async ({ page }) => {
  await configureNas(page);
  await page.context().clearCookies();
});
When('I open the console without signing in', async ({ page }) => {
  await page.goto('/console');
});
Then('I am sent to the login page', async ({ page }) => {
  await expect(page).toHaveURL(/\/login$/);
});
When('I sign in through the gateway', async ({ page }) => {
  await page.getByTestId('login-username').fill(ACCOUNT.username);
  await page.getByTestId('login-password').fill(ACCOUNT.password);
  await page.getByTestId('login-submit').click();
});
Then('the console opens with a server session', async ({ page }) => {
  await expect(page).toHaveURL(/\/console$/);
  const session = await page.request.get('/auth/session');
  expect(session.ok()).toBe(true);
  expect(await session.json()).toEqual({ authenticated: true, username: ACCOUNT.username });
});
Then('the sign-in is refused', async ({ page }) => {
  await expect(page.getByRole('alert')).toBeVisible();
  expect((await page.request.get('/auth/session')).status()).toBe(401);
});
