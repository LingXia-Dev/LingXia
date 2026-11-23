import type { Locator, TestApp, TestView } from '@lingxia/test';

export const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

export type Box = { x: number; y: number; width: number; height: number };

/** A real finger tap at page CSS pixels. */
export async function tap(view: TestView, at: [number, number]): Promise<void> {
  const finger = await view.touch(at);
  await pause(60);
  await finger.up();
  await pause(400);
}

/** Scroll `target` into view through the semantics, with no pointer; its box there. */
export async function reveal(view: TestView, target: Locator): Promise<Box> {
  await target.scrollIntoView();
  await view.settled();
  return target.boundingBox();
}

/** The platform's minimum touch target in pt/dp; a desktop has none, so its specs skip. */
export async function touchMinimum(t: { app: TestApp; skip(reason: string): never }): Promise<number> {
  const os: string = await t.app.logic.eval(({ lx }) => String(lx.host.info.os || '').toLowerCase());
  if (os === 'android') return 48;
  if (os === 'ios' || os === 'harmony' || os === 'harmonyos' || os === 'ohos') return 44;
  return t.skip(`no touch target on ${os}`);
}
