import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';

const { When, Then } = createBdd();

When('I open the terminal', async ({ page }) => {
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId('app_menu_app_nas-system-terminal').click();
});

When('I run {string} in the terminal', async ({ page }, command: string) => {
  await expect(page.getByTestId('terminal_connection')).toHaveAttribute(
    'title',
    /Collegato|Connected/,
  );
  await page.getByTestId('terminal_command').fill(command);
  await page.getByTestId('terminal_command').press('Enter');
  // The command is over once the prompt comes back.
  await expect(page.getByTestId('terminal_run')).toBeVisible();
});

Then('the terminal shows {string}', async ({ page }, text: string) => {
  await expect(page.getByTestId('terminal_output')).toContainText(text);
});

Then('the prompt of the terminal is in {string}', async ({ page }, directory: string) => {
  await expect(page.locator('label[for="terminal_command"]')).toHaveText(`${directory} $`);
});

When('I open the system log of the terminal', async ({ page }) => {
  await page.getByTestId('terminal_view_log').click();
});

// Whether the machine running the scenarios has a journal to read is not known here: the tab is
// what is checked, with its panel, and no prompt since a log takes no command.
Then('the log tab takes the place of the console', async ({ page }) => {
  await expect(page.getByTestId('terminal_log')).toBeVisible();
  await expect(page.getByTestId('terminal_prompt')).toHaveCount(0);
  await page.getByTestId('terminal_view_console').click();
  await expect(page.getByTestId('terminal_prompt')).toBeVisible();
});

When('I make the terminal font bigger and save', async ({ page }) => {
  await page.getByTestId('terminal_settings_open').click();
  for (const field of ['text', 'background', 'highlight']) {
    const input = page.getByTestId(`terminal_${field}_color`);
    await expect(input).toBeVisible();
    const box = await input.boundingBox();
    expect(box?.width).toBe(40);
    expect(box?.height).toBe(40);
    expect(await input.evaluate((element) => getComputedStyle(element).borderRadius)).toBe('10px');
  }
  for (const field of ['font_size', 'line_height']) {
    const input = page.getByTestId(`terminal_${field}`);
    await input.hover();
    await expect(input).toHaveCSS('appearance', 'textfield');
  }
  await page.screenshot({ path: '/tmp/terminal-settings-controls.png' });
  const viewport = page.viewportSize();
  const theme = await page.locator('html').getAttribute('data-theme');
  try {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.locator('html').evaluate((element) => element.setAttribute('data-theme', 'dark'));
    const dialog = page.getByRole('dialog');
    expect(await dialog.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(
      true,
    );
    const positions: number[] = [];
    for (const field of ['text', 'background', 'highlight']) {
      const input = page.getByTestId(`terminal_${field}_color`);
      await expect(input).toBeInViewport({ ratio: 1 });
      positions.push((await input.boundingBox())!.x);
    }
    expect(new Set(positions).size).toBe(1);
    await page.screenshot({ path: '/tmp/terminal-settings-controls-mobile.png' });
  } finally {
    if (viewport) {
      await page.setViewportSize(viewport);
    }
    await page.locator('html').evaluate((element, value) => {
      if (value === null) {
        element.removeAttribute('data-theme');
      } else {
        element.setAttribute('data-theme', value);
      }
    }, theme);
  }
  await page.getByTestId('terminal_font_size_increase').click();
  await page.getByTestId('terminal_settings_save').click();
  await expect(
    page.getByTestId(/^notification_toast_/).filter({ hasText: 'Impostazioni salvate' }),
  ).toBeVisible();
});

Then('the terminal keeps the bigger font', async ({ page }) => {
  const saved = await (await page.request.get('/api/settings/terminal')).json();
  expect(saved.fontSize).toBeGreaterThan(14);
  await expect(page.getByTestId('terminal_app')).toHaveAttribute(
    'style',
    new RegExp(`--terminal-font-size: ${saved.fontSize}px`),
  );
});
