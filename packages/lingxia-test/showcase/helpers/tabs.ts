import type { TestApp } from '@lingxia/test';

/**
 * A tab's index, found by any of its labels (one per language): the showcases
 * order their tabs differently (the GPU mirror has no Components tab), so a
 * spec never assumes an index.
 */
export async function tabIndex(app: TestApp, labels: readonly string[]): Promise<number | undefined> {
  return (await app.info()).tabBar?.items.find((item) => item.text !== null && labels.includes(item.text))?.index;
}

/** The current label of the tab `index` names, or null. */
export async function tabLabel(app: TestApp, index: number | undefined): Promise<string | null> {
  return (await app.info()).tabBar?.items.find((item) => item.index === index)?.text ?? null;
}
