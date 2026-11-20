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

/** Scroll until `target` sits well inside the viewport; its box there. */
export async function reveal(view: TestView, target: Locator): Promise<Box> {
  for (let step = 0; step < 8; step++) {
    const box = await target.boundingBox();
    const vp = await view.viewport();
    if (box.y >= vp.top + 80 && box.y + box.height <= vp.top + vp.height - 80) return box;
    // A real finger drag: `view.scroll` is the WebView DOM's, not a GPU page's.
    const limit = vp.height * 0.6;
    const dy = Math.max(-limit, Math.min(limit, box.y - (vp.top + vp.height / 3)));
    const x = vp.left + vp.width / 2;
    const y = vp.top + vp.height / 2;
    await view.swipe({ from: [x, y + dy / 2], to: [x, y - dy / 2], durationMs: 400, hold: 150 });
    await view.settled();
  }
  return target.boundingBox();
}

/** The platform's minimum touch target in pt/dp; a desktop has none, so its specs skip. */
export async function touchMinimum(t: { app: TestApp; skip(reason: string): never }): Promise<number> {
  const os: string = await t.app.logic.eval(({ lx }) => String(lx.host.info.os || '').toLowerCase());
  if (os === 'android') return 48;
  if (os === 'ios' || os === 'harmony' || os === 'harmonyos' || os === 'ohos') return 44;
  return t.skip(`no touch target on ${os}`);
}
