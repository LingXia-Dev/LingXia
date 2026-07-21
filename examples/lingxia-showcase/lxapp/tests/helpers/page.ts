import type { LxAppDriver } from 'lingxia-types';

export async function waitForElementEnabled(
  app: LxAppDriver,
  page: string,
  css: string,
  timeoutMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const element = await app.page.query({ page, css, full: true });
    if (element.exists && element.enabled) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`Timed out waiting for enabled element: ${page} ${css}`);
}

export async function waitForElementAttribute(
  app: LxAppDriver,
  page: string,
  css: string,
  attribute: string,
  expected: string,
  timeoutMs = 10_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let actual: unknown = null;
  while (Date.now() < deadline) {
    actual = await app.page.eval({
      page,
      script: `document.querySelector(${JSON.stringify(css)})?.getAttribute(${JSON.stringify(attribute)}) ?? null`,
    });
    if (actual === expected) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(
    `Timed out waiting for ${page} ${css} ${attribute}=${JSON.stringify(expected)}, `
      + `received ${JSON.stringify(actual)}`,
  );
}
