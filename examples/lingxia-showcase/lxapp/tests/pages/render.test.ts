import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';
import { SHOWCASE_PAGE_EXPECTATIONS, SHOWCASE_PAGES } from './manifest.js';

async function waitForFeatureText(
  app: LxAppDriver,
  page: string,
  expected: string,
): Promise<void> {
  const deadline = Date.now() + 10_000;
  let actual = '';
  while (Date.now() < deadline) {
    try {
      const body = await app.page.query({ page, css: 'body', full: true });
      if (body.exists && 'text' in body) {
        actual = body.text;
        if (actual.toLocaleLowerCase().includes(expected.toLocaleLowerCase())) return;
      }
    } catch (error) {
      actual = String(error);
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(
    `Expected page '${page}' to render '${expected}', received: ${JSON.stringify(actual.slice(0, 500))}`,
  );
}

test('page manifest matches the running lxapp', async () => {
  const pages = await lx.automation().lxapp().pages();
  expect(pages.map((page) => page.name)).toEqual([...SHOWCASE_PAGES]);
});

for (const expectation of SHOWCASE_PAGE_EXPECTATIONS) {
  test(`renders showcase feature: ${expectation.page}`, async () => {
    const app = lx.automation().lxapp();
    try {
      await app.nav.relaunch({ page: expectation.page });
      await waitForFeatureText(app, expectation.page, expectation.text);
    } catch (error) {
      const screenshot = await app.page.screenshot({ page: expectation.page });
      await test.attach?.(`page-${expectation.page}.png`, {
        mimeType: 'image/png',
        base64: screenshot.base64,
      });
      throw error;
    }
  });
}
