import { openConsole } from './auth_helpers';
import { handleNotificationToasts } from './toast_helpers';
import { randomUUID } from 'node:crypto';
import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';

const { Given, When, Then } = createBdd();

/**
 * The notification log is the one thing every part of the launcher writes into, and it is also
 * the one that has to survive a restart. This scenario goes the whole way round: an application
 * records a line through the public API, the interface finds it, filters on it, marks it read and
 * removes it: and then checks that the log really forgot it.
 */
let title = '';
let source = '';
let id = '';

Given('an application records a notification', async ({ request }) => {
  title = `Copia completata ${randomUUID()}`;
  // A source of its own, so the scenario can filter on it without depending on what else is in
  // the log: and without two parallel runs seeing each other.
  source = `e2e-${randomUUID().slice(0, 8)}`;

  const created = await request.post('/api/notifications', {
    data: { title, message: 'Scritta dallo scenario end-to-end.', source, tone: 'success' },
  });
  expect(created.ok()).toBe(true);
  id = (await created.json()).id;

  // It is in the log before any browser has opened: this is the API applications will use.
  const listed = await (await request.get(`/api/notifications?source=${source}`)).json();
  expect(listed).toHaveLength(1);
});

When('I open the notification application', async ({ page }) => {
  await handleNotificationToasts(page);
  await openConsole(page);
  await page.getByTestId('header_menu_toggle').click();
  await page.getByTestId('app_menu_app_nas-notification-manager').click();
  await expect(page.getByTestId(`notification_${id}`)).toBeVisible();
});

Then('the notification is listed, can be filtered, read and removed', async ({ page, request }) => {
  const row = page.getByTestId(`notification_${id}`);
  await expect(row).toContainText(title);
  await expect(row).toContainText(source);

  // The source of the row is also the filter: one click, and only that source is left.
  await row.getByRole('button', { name: source }).click();
  await expect(page.getByTestId('notifications_source')).toHaveValue(source);
  await expect(page.getByTestId('notifications_matches')).toContainText('1 notifiche');
  await expect(page.getByTestId(`notification_${id}`)).toBeVisible();

  // A level it does not have leaves the list empty, and says so.
  await page.getByTestId('notifications_tone_error').click();
  await expect(page.getByTestId('notifications_empty')).toHaveText('Nessuna notifica trovata');
  await expect(page.getByTestId('notifications_matches')).toHaveCount(0);

  // Reset brings everything back, filters included.
  await page.getByTestId('notifications_reset').click();
  await expect(page.getByTestId('notifications_source')).toHaveValue('');
  await expect(page.getByTestId(`notification_${id}`)).toBeVisible();

  // Marking it read really writes: the action disappears from the row and the backend agrees.
  await row.getByRole('button', { name: `Segna come letta ${title}` }).click();
  await expect(row.getByRole('button', { name: `Segna come letta ${title}` })).toHaveCount(0);
  await expect
    .poll(async () => {
      const [saved] = await (await request.get(`/api/notifications?source=${source}`)).json();
      return saved?.read;
    })
    .toBe(true);

  // And removing it takes it out of the log, not just off the screen.
  await row.getByRole('button', { name: `Rimuovi ${title}` }).click();
  await expect(page.getByTestId(`notification_${id}`)).toHaveCount(0);
  await expect
    .poll(
      async () => (await (await request.get(`/api/notifications?source=${source}`)).json()).length,
    )
    .toBe(0);
});

// The first reading of the log only takes note of what is there: it has to happen before the
// notification is recorded, or the notification would be part of it. The clock is the page's own,
// so the next reading can be brought forward instead of waited for.
Given('the launcher is open', async ({ page }) => {
  await page.clock.install();
  const firstReading = page.waitForResponse((answer) =>
    answer.url().includes('/api/notifications?read=false'),
  );
  await openConsole(page);
  await firstReading;
});

// The launcher reads the log every fifteen seconds: the toast comes with the next reading.
Then(
  'the notification pops up as a toast that opens the notification application',
  async ({ page, request }) => {
    await page.clock.fastForward(15_000);
    const toast = page.getByTestId(`notification_toast_${id}`);
    await expect(toast).toContainText(title);

    await toast.getByRole('button', { name: 'Vedi' }).click();
    await expect(toast).toHaveCount(0);
    await expect(page.getByTestId(`notification_${id}`)).toBeVisible();

    await request.delete(`/api/notifications/${id}`, { data: {} });
  },
);
