import { expect, test } from '@rongjs/test';
import type { LxAppDriver } from 'lingxia-types';
import {
  SHOWCASE_PAGE_EXPECTATIONS,
  SHOWCASE_PAGE_TITLES,
  SHOWCASE_PAGES,
} from './manifest.js';

interface DocumentState {
  title: string;
  text: string;
  isNotFound: boolean;
}

async function waitForRenderedFeature(
  app: LxAppDriver,
  page: string,
  expectedTitle: string,
  expectedText: string,
): Promise<DocumentState> {
  const deadline = Date.now() + 10_000;
  let lastState: DocumentState | null = null;
  while (Date.now() < deadline) {
    try {
      lastState = await app.page.eval({
        page,
        script: `({
          title: document.title,
          text: document.body.innerText.trim(),
          isNotFound: document.title === '404'
            || document.body.innerText.includes('Page Not Found')
            || document.body.innerText.includes('not_found'),
        })`,
      }) as DocumentState;
      if (
        lastState.title === expectedTitle
        && lastState.text.toLocaleLowerCase().includes(expectedText.toLocaleLowerCase())
        && !lastState.isNotFound
      ) {
        return lastState;
      }
    } catch {
      // The WebView can be attached just before its first document is ready.
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(
    `Expected rendered page '${page}' with title '${expectedTitle}' and text `
      + `'${expectedText}', received: ${JSON.stringify(lastState)}`,
  );
}

test('page manifest matches the running lxapp', async () => {
  const pages = await lx.automation().lxapp().pages();
  expect(pages.map((page) => page.name)).toEqual([...SHOWCASE_PAGES]);
  expect(pages.every((page) => (
    page.path.toLowerCase().includes(`pages/${page.name.toLowerCase()}/index.`)
  ))).toBeTruthy();
});

for (const expectation of SHOWCASE_PAGE_EXPECTATIONS) {
  test(`renders showcase feature: ${expectation.page}`, async () => {
    const app = lx.automation().lxapp();
    try {
      const landed = await app.nav.relaunch({ page: expectation.page });
      expect(landed.name).toBe(expectation.page);
      expect(landed.path.toLowerCase()).toContain(
        `pages/${expectation.page.toLowerCase()}/index.`,
      );

      const current = await app.nav.current();
      expect(current.name).toBe(expectation.page);
      expect(current.ready).toBeTruthy();

      const documentState = await waitForRenderedFeature(
        app,
        expectation.page,
        SHOWCASE_PAGE_TITLES[expectation.page],
        expectation.text,
      );
      expect(documentState.title).toBe(SHOWCASE_PAGE_TITLES[expectation.page]);
      expect(documentState.text.length > 0).toBeTruthy();
      expect(documentState.isNotFound).toBeFalsy();
    } catch (error) {
      try {
        const screenshot = await app.page.screenshot({ page: expectation.page });
        await test.attach?.(`page-${expectation.page}.png`, {
          mimeType: 'image/png',
          base64: screenshot.base64,
        });
      } catch {
        // Preserve the assertion/navigation error when screenshot capture also fails.
      }
      throw error;
    }
  });
}
