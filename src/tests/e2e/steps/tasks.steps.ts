import { openConsole } from './auth_helpers';
import { randomUUID } from 'node:crypto';
import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
import type { UserTask } from '@/types/widgets/tasks';

const { Given, When, Then } = createBdd();

let id = '';
let source = '';

Given('an application registers a long operation', async ({ request }) => {
  id = `probe-${randomUUID().slice(0, 8)}`;
  source = `e2e-${randomUUID().slice(0, 8)}`;

  const created = await request.post('/api/tasks', {
    data: {
      id,
      source,
      name: 'Copia di prova',
      detail: '0 di 30 GB',
      progress: 0,
      pausable: true,
      cancellable: true,
    },
  });
  expect(created.ok()).toBe(true);
  expect((await created.json()).state).toBe('running');

  const advanced = await request.patch(`/api/tasks/${id}`, {
    data: { progress: 42, detail: '12 di 30 GB' },
  });
  expect((await advanced.json()).progress).toBe(42);
});

When('I open the operations widget', async ({ page }) => {
  await openConsole(page);
  await page.getByTestId('header_task_toggle').click();
  await expect(page.getByTestId(`task_${id}`)).toBeVisible();
});

Then(
  'the operation is shown, can be paused and can be swept up when it ends',
  async ({ page, request }) => {
    const row = page.getByTestId(`task_${id}`);
    await expect(row).toContainText('Copia di prova');
    await expect(row).toContainText('12 di 30 GB');
    await expect(row).toContainText(source);
    await expect(row.locator('progress')).toHaveAttribute('value', '42');
    await expect(page.getByTestId(`task_state_${id}`)).toHaveText('In corso');

    await row.getByRole('button', { name: 'Pausa Copia di prova' }).click();
    await expect(page.getByTestId(`task_state_${id}`)).toHaveText('In pausa');
    await expect
      .poll(async () => {
        const register: UserTask[] = await (await request.get('/api/tasks')).json();
        return register.find((entry) => entry.id === id)?.state;
      })
      .toBe('paused');

    const closed = await request.patch(`/api/tasks/${id}`, {
      data: { state: 'done', progress: 100, detail: '30 di 30 GB' },
    });
    expect(closed.ok()).toBe(true);
    await expect(row).toHaveCount(0, { timeout: 15_000 });
    await page.getByTestId('task_view_finished').click();
    await expect(page.getByTestId(`task_state_${id}`)).toHaveText('Conclusa');
    await expect(row.getByRole('button', { name: 'Pausa Copia di prova' })).toHaveCount(0);

    await page.getByTestId('task_clear_finished').click();
    await expect(page.getByTestId(`task_${id}`)).toHaveCount(0);
    await expect
      .poll(async () => {
        const register: UserTask[] = await (await request.get('/api/tasks')).json();
        return register.some((entry) => entry.id === id);
      })
      .toBe(false);
  },
);
