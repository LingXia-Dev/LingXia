import { expect, spec } from '@lingxia/test';
import type { LxAppDriver } from 'lingxia-types/automation';
import { showcaseApp } from '../helpers/app.js';
import { waitForCurrentPage } from '../helpers/page.js';
import { attachShot, eventually } from '../helpers/poll.js';
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

function isTransientPageReadinessError(error: unknown): boolean {
  const message = String(error).toLocaleLowerCase();
  return message.includes('page is not active:')
    || message.includes('page webview is not ready')
    || message.includes('no current page')
    || message.includes('0x8007139f')
    || message.includes('webview destroyed during javascript evaluation')
    || message.includes('navigation changed during javascript evaluation')
    || message.endsWith('javascript error: @');
}

async function waitForRenderedFeature(
  app: LxAppDriver,
  page: string,
  expectedTitle: string,
  expectedText: string | readonly string[],
): Promise<DocumentState> {
  const expectedTexts = typeof expectedText === 'string' ? [expectedText] : expectedText;
  const state = await eventually(
    () => app.page.eval({
        page,
        script: `(() => {
          const body = document.body;
          if (!body) return null;
          const text = body.innerText.trim();
          return {
            title: document.title,
            text,
            isNotFound: document.title === '404'
              || text.includes('Page Not Found')
              || text.includes('not_found'),
          };
        })()`,
      }) as Promise<DocumentState | null>,
    (candidate) => (
        candidate !== null
        && candidate.title === expectedTitle
        && expectedTexts.some((text) => (
          candidate.text.toLocaleLowerCase().includes(text.toLocaleLowerCase())
        ))
        && !candidate.isNotFound
    ),
    {
      describe: `rendered page '${page}' with title '${expectedTitle}' and text ${JSON.stringify(expectedTexts)}`,
      retryIf: isTransientPageReadinessError,
      timeoutMs: 30_000,
    });
  if (state === null) throw new Error(`page eval stayed null while rendering '${page}'`);
  return state;
}

spec('page manifest matches the running lxapp', async () => {
  const pages = await showcaseApp().pages();
  expect(pages.map((page) => page.name)).toEqual([...SHOWCASE_PAGES]);
  expect(pages.every((page) => (
    page.path.toLowerCase().includes(`pages/${page.name.toLowerCase()}/index.`)
  ))).toBeTruthy();
});

for (const expectation of SHOWCASE_PAGE_EXPECTATIONS) {
  spec(`renders showcase feature: ${expectation.page}`, async (t) => {
    const app = showcaseApp();
    try {
      const landed = await app.nav.relaunch({ page: expectation.page });
      expect(landed.name).toBe(expectation.page);
      expect(landed.path.toLowerCase()).toContain(
        `pages/${expectation.page.toLowerCase()}/index.`,
      );

      const current = await app.nav.current();
      expect(current.name).toBe(expectation.page);

      await app.page.waitFor({
        page: expectation.page,
        css: 'body',
        state: 'exists',
        timeoutMs: 20_000,
      });

      const documentState = await waitForRenderedFeature(
        app,
        expectation.page,
        SHOWCASE_PAGE_TITLES[expectation.page],
        expectation.text,
      );
      expect(documentState.title).toBe(SHOWCASE_PAGE_TITLES[expectation.page]);
      expect(documentState.text.length > 0).toBeTruthy();
      expect(documentState.isNotFound).toBeFalsy();
      const ready = await waitForCurrentPage(app, expectation.page, 30_000);
      expect(ready.name).toBe(expectation.page);
      expect(ready.ready).toBeTruthy();
    } catch (error) {
      try {
        const screenshot = await app.page.screenshot({ page: expectation.page });
        await attachShot(t, `page-${expectation.page}.png`, {
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
