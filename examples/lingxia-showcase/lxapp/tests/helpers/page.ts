import type { LxAppDriver, PageInfo } from 'lingxia-types/automation';
import { eventually } from '../support/contract.js';

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
    },
  );
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
    { timeoutMs, describe: `${page} ${css} text` },
  );
  if (text === null) throw new Error(`Element disappeared after wait: ${page} ${css}`);
  return text;
}

export async function waitForCurrentPage(
  app: LxAppDriver,
  page: string,
  timeoutMs = 10_000,
): Promise<PageInfo> {
  return eventually(
    () => app.nav.current(),
    (current) => current.name === page && current.ready,
    { timeoutMs, describe: `current page '${page}' to become ready` },
  );
}
