import type { Locator, TestView } from '@lingxia/test';

/**
 * Ids this page repeats per row (`todo-label`, `channel-symbol`, …) that the
 * GPU mirror keys instead (`todo-label-<id>`); the GPU showcase lists them in
 * its tests/shared-specs.json `keyed` and names them before these specs load.
 */
const keyed = new Set((globalThis as { __SHOWCASE_KEYED_IDS__?: string[] }).__SHOWCASE_KEYED_IDS__ ?? []);

/** One row's element: `<base>-<key>` where the page keys it, else `repeated` (this page's own locator). */
export function keyedElement(view: TestView, base: string, key: string | number, repeated: Locator): Locator {
  return keyed.has(base) ? view.testId(`${base}-${key}`) : repeated;
}
