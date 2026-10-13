import { expect, type Page } from '@playwright/test';

export async function handleNotificationToasts(page: Page) {
  const first = page.getByTestId('notification_toasts').getByRole('listitem').first();
  await page.addLocatorHandler(
    first,
    async (toast) => {
      const id = await toast.getAttribute('data-testid');
      if (!id) {
        throw new Error('Notification toast is missing its test id');
      }
      const current = page.getByTestId(id);
      await current.getByTestId('alert_banner_close').click();
      await expect(current).toBeHidden();
    },
    // Another toast may take the first one's place; let the handler close that one on retry.
    { noWaitAfter: true },
  );
  return first;
}
