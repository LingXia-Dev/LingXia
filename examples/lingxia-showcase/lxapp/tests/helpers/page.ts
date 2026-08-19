import type { LxAppDriver, PageInfo } from 'lingxia-types/automation';
import { eventually } from './poll.js';

export async function waitForElementEnabled(
  app: LxAppDriver,
  page: string,
  css: string,
  timeoutMs = 10_000,
): Promise<void> {
  await app.page.waitFor({ page, css, state: 'enabled', timeoutMs });
}

export async function waitForElementAttribute(
  app: LxAppDriver,
  page: string,
  css: string,
  attribute: string,
  expected: string,
  timeoutMs = 10_000,
): Promise<void> {
  await eventually(
    () => app.page.eval({
      page,
      script: `document.querySelector(${JSON.stringify(css)})?.getAttribute(${JSON.stringify(attribute)}) ?? null`,
    }),
    (actual) => actual === expected,
    {
      timeoutMs,
      describe: `${page} ${css} ${attribute}=${JSON.stringify(expected)}`,
    });
}

export async function waitForElementText(
  app: LxAppDriver,
  page: string,
  css: string,
  predicate: (text: string) => boolean,
  timeoutMs = 10_000,
): Promise<string> {
  const text = await eventually(
    async () => {
      const element = await app.page.query({ page, css, full: true });
      return element.exists ? element.text : null;
    },
    (value) => value !== null && predicate(value),
    { timeoutMs, describe: `${page} ${css} text` });
  if (text === null) throw new Error(`Element disappeared after wait: ${page} ${css}`);
  return text;
}

function isCurrentPageTransition(error: unknown): boolean {
  return String(error).includes('current page');
}

/** Current page lookup that treats an empty relaunch-transition stack as absent. */
export async function currentPageOrNull(app: LxAppDriver): Promise<PageInfo | null> {
  try {
    return await app.nav.current();
  } catch (error) {
    if (isCurrentPageTransition(error)) return null;
    throw error;
  }
}

export async function waitForCurrentPage(
  app: LxAppDriver,
  page: string,
  timeoutMs = 10_000,
): Promise<PageInfo> {
  return eventually(
    () => app.nav.current(),
    (current) => current.name === page && current.ready,
    {
      timeoutMs,
      describe: `current page '${page}' to become ready`,
      retryIf: isCurrentPageTransition,
    });
}

export async function waitForCurrentPageVisible(
  app: LxAppDriver,
  page: string,
  css: string,
  timeoutMs = 10_000,
): Promise<PageInfo> {
  const current = await eventually(
    () => app.nav.current(),
    (candidate) => candidate.name === page && candidate.current,
    {
      timeoutMs,
      describe: `current page '${page}' to become active`,
      retryIf: isCurrentPageTransition,
    });
  await app.page.waitFor({ page, css, state: 'visible', timeoutMs });
  return current;
}
