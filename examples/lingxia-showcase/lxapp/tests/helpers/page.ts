import type { PageInfo } from '@lingxia/types/automation';
import type { Fixture, TestApp } from '@lingxia/test';
import { eventually } from './poll.js';

// These waits read the first match of `css`, as the page checks they replace
// did; a selector the page renders once needs no `.first()` of its own. They
// wait on `t.app` unless given another handle of the app (one reopened since).

export async function waitForElementEnabled(
  t: Fixture,
  page: string,
  css: string,
  timeoutMs = 10_000,
  app: TestApp = t.app,
): Promise<void> {
  await t.expect(app.view.css(css, { page }).first()).toBeEnabled({ timeout: timeoutMs });
}

export async function waitForElementAttribute(
  t: Fixture,
  page: string,
  css: string,
  attribute: string,
  expected: string,
  timeoutMs = 10_000,
  app: TestApp = t.app,
): Promise<void> {
  await t.expect(app.view.css(css, { page }).first()).toHaveAttribute(attribute, expected, { timeout: timeoutMs });
}

export async function waitForElementText(
  t: Fixture,
  page: string,
  css: string,
  predicate: (text: string) => boolean,
  timeoutMs = 10_000,
  app: TestApp = t.app,
): Promise<string> {
  const element = app.view.css(css, { page }).first();
  const text = await t.waitFor(
    async () => {
      const found = await element.query();
      return found.exists ? found.text : null;
    },
    { until: (value) => value !== null && predicate(value), timeout: timeoutMs });
  if (text === null) throw new Error(`Element disappeared after wait: ${page} ${css}`);
  return text;
}

function isCurrentPageTransition(error: unknown): boolean {
  return String(error).includes('current page');
}

/** Current page lookup that treats an empty relaunch-transition stack as absent. */
export async function currentPageOrNull(app: TestApp): Promise<PageInfo | null> {
  try {
    return await app.nav.current();
  } catch (error) {
    if (isCurrentPageTransition(error)) return null;
    throw error;
  }
}

export async function waitForCurrentPage(
  app: TestApp,
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
  app: TestApp,
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
  await app.view.css(css, { page }).first().waitFor({ state: 'visible', timeout: timeoutMs });
  return current;
}
