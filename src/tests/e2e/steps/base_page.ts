import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';

const { When, Then } = createBdd();

// Steps any feature can use. Every element is named by its `data-testid`, never by a CSS class:
// `When I click on "login-submit"` reaches `[data-testid="login-submit"]`.

When('I go to {string}', async ({ page }, path: string) => {
  await page.goto(path);
});

When('I click on {string}', async ({ page }, testId: string) => {
  await page.getByTestId(testId).click();
});

When('I fill {string} with {string}', async ({ page }, testId: string, value: string) => {
  await page.getByTestId(testId).fill(value);
});

Then('{string} is visible', async ({ page }, testId: string) => {
  await expect(page.getByTestId(testId)).toBeVisible();
});

Then('{string} is not visible', async ({ page }, testId: string) => {
  await expect(page.getByTestId(testId)).toBeHidden();
});

Then('{string} contains {string}', async ({ page }, testId: string, text: string) => {
  await expect(page.getByTestId(testId)).toContainText(text);
});

Then('the address ends with {string}', async ({ page }, path: string) => {
  await expect(page).toHaveURL((url) => url.pathname.endsWith(path));
});
