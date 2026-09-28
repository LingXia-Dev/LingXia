import type { PageInfo } from '@lingxia/types/automation';
import { expect, type Fixture, type TestApp, type TestView } from '@lingxia/test';
import { eventually } from './poll.js';

// These waits read the first match of `css`, as the page checks they replace
// did; a selector the page renders once needs no `.first()` of its own. Pass
// the View of a bound page (`(await t.app.page({ name })).view`), or
// `t.app.view` for whichever page is current.

export async function waitForElementEnabled(
  view: TestView,
  css: string,
  timeoutMs = 10_000,
): Promise<void> {
  await expect(view.css(css).first()).toBeEnabled({ timeout: timeoutMs });
}

export async function waitForElementAttribute(
  view: TestView,
  css: string,
  attribute: string,
  expected: string,
  timeoutMs = 10_000,
): Promise<void> {
  await expect(view.css(css).first()).toHaveAttribute(attribute, expected, { timeout: timeoutMs });
}

/** Wait until the element's text passes `predicate`, and resolve to that text. */
export async function waitForElementText(
  t: Fixture,
  view: TestView,
  css: string,
  predicate: (text: string) => boolean,
  timeoutMs = 10_000,
): Promise<string> {
  const element = view.css(css).first();
  // Not rendered yet rejects the read, which `t.waitFor` retries.
  return t.waitFor(() => element.textContent(), { until: predicate, timeout: timeoutMs });
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
  await app.view.css(css).first().waitFor({ state: 'visible', timeout: timeoutMs });
  return current;
}
