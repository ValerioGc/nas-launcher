import { expect } from '@playwright/test';
import { createBdd } from 'playwright-bdd';
import { openConsole } from './auth_helpers';
import type { InstalledApp } from '@/types/types';

const { Then } = createBdd();

Then(
  'the launcher closes the app view after terminating it from the {string}',
  async ({ page }, source: string) => {
    const picture = 'nas-picture-manager';
    const monitor = 'nas-system-monitor';
    let stopped = false;
    let actions = 0;
    let releaseFrame!: () => void;
    const frameReady = new Promise<void>((resolve) => {
      releaseFrame = resolve;
    });
    await page.route('**/api/apps', async (route) => {
      const response = await route.fetch();
      const apps: InstalledApp[] = await response.json();
      await route.fulfill({
        json: [
          ...apps,
          { id: picture, name: 'Pictures', info: '', system: false },
          { id: monitor, name: 'Monitor', info: '', system: false },
        ],
      });
    });
    await page.route(`**/apps/${picture}/**`, (route) =>
      route.fulfill({
        status: stopped ? 503 : 200,
        contentType: 'text/html',
        body: '<button>Picture action</button><script>window.addEventListener("message", () => parent.postMessage({ nas: 1, type: "ready" }, location.origin));</script>',
      }),
    );
    await page.route(`**/apps/${monitor}/**`, async (route) => {
      const isWidget = new URL(route.request().url()).searchParams.get('view') === 'widget';
      if (isWidget) {
        await frameReady;
      }
      await route.fulfill({
        contentType: 'text/html',
        body: `<button onclick="parent.postMessage({ nas: 1, type: 'process-action', requestId: 'terminate-picture', payload: { id: 400, started: 900, action: 'terminate' } }, location.origin)">Terminate Pictures</button><script>window.announceReady = () => parent.postMessage({ nas: 1, type: 'ready' }, location.origin); window.addEventListener('message', (event) => { if (event.data.type === 'context' && !${isWidget}) window.announceReady(); });</script>`,
      });
    });

    await page.route(`**/api/apps/*/start`, (route) => route.fulfill({ json: null }));
    await page.route('**/api/system/processes/400', async (route) => {
      expect(route.request().postDataJSON()).toEqual({ started: 900, action: 'terminate' });
      actions++;
      stopped = true;
      await route.fulfill({ json: { appId: picture } });
    });
    await openConsole(page);
    await page.getByTestId('header_menu_toggle').click();
    await page.getByTestId(`app_menu_app_${picture}`).click();
    const pictureFrame = page.getByTestId(`app_frame_${picture}`);
    await expect(pictureFrame).toBeVisible();
    await expect(
      page
        .frameLocator(`[data-testid="app_frame_${picture}"]`)
        .getByRole('button', { name: 'Picture action' }),
    ).toBeVisible();
    if (source === 'app') {
      await page.getByTestId('header_menu_toggle').click();
      await page.getByTestId(`app_menu_app_${monitor}`).click();
    } else {
      await page.locator('html').evaluate((element) => element.setAttribute('data-theme', 'dark'));
      await page.getByTestId('header_process_toggle').click();
      const loading = page.getByTestId('process_widget_loading');
      const frame = page.getByTestId('process_widget_frame');
      await expect(loading).toBeVisible();
      await expect(frame).toBeHidden();
      const address = new URL((await frame.getAttribute('src'))!, page.url());
      expect(address.searchParams.get('theme')).toBe(
        await page.locator('html').getAttribute('data-theme'),
      );
      await page
        .getByTestId('process_panel')
        .screenshot({ path: '/tmp/nas-monitor-widget-frame-loading.png' });
      releaseFrame();
      await expect(
        page
          .frameLocator('[data-testid="process_widget_frame"]')
          .getByRole('button', { name: 'Terminate Pictures' }),
      ).toHaveCount(1);
      await expect(loading).toBeVisible();
      await expect(frame).toBeHidden();
      const reveal = async () => {
        await frame.evaluate((element) => {
          const child = (element as HTMLIFrameElement).contentWindow as Window & {
            announceReady: () => void;
          };
          child.announceReady();
        });
        await expect(loading).toHaveCount(0);
        await expect(frame).toBeVisible();
      };
      await reveal();
      await page.getByTestId('widget_close').click();
      await expect(page.getByTestId('process_panel')).toHaveCount(0);
      await page.getByTestId('header_process_toggle').click();
      await expect(loading).toBeVisible();
      await expect(frame).toBeHidden();
      await expect(
        page
          .frameLocator('[data-testid="process_widget_frame"]')
          .getByRole('button', { name: 'Terminate Pictures' }),
      ).toHaveCount(1);
      await reveal();
    }
    const testid = source === 'app' ? `app_frame_${monitor}` : 'process_widget_frame';
    await page
      .frameLocator(`[data-testid="${testid}"]`)
      .getByRole('button', { name: 'Terminate Pictures' })
      .click();
    await expect(page.getByTestId(`app_tab_${picture}`)).toHaveCount(0);
    await expect(pictureFrame).toHaveCount(0);
    expect(actions).toBe(1);
    await page.reload();
    await expect(page.getByTestId('header_menu_toggle')).toBeVisible();
    await expect(page.getByTestId(`app_tab_${picture}`)).toHaveCount(0);
    await expect(pictureFrame).toHaveCount(0);
  },
);
