import type { spec } from '@lingxia/test';
import { decodePng } from './png.js';

// Pixel readings shared by the specs that check what a control draws.
export type Fixture = Parameters<Parameters<typeof spec>[2]>[0];

export const apart = (a: number[], b: number[]) => a.some((value, i) => Math.abs(value - b[i]) > 40);

/** Hue in degrees and spread (max − min channel) of an sRGB sample; a grey has almost no spread. */
export function tint([r, g, b]: number[]) {
  const max = Math.max(r, g, b), spread = max - Math.min(r, g, b);
  if (spread === 0) return { hue: 0, spread };
  const sector = max === r ? (g - b) / spread : max === g ? (b - r) / spread + 2 : (r - g) / spread + 4;
  return { hue: (sector * 60 + 360) % 360, spread };
}

/** Whether `sample` is `role`'s colour, faded or not: the same hue, and not grey. */
export function sameHue(sample: number[], role: number[]) {
  const [a, b] = [tint(sample), tint(role)];
  const turn = Math.abs(a.hue - b.hue);
  return a.spread > 20 && Math.min(turn, 360 - turn) < 20;
}

/** The colour just inside a control's left padding, brought into the top of the screen first:
 * a phone-sized screenshot decodes only down to the sampled row, so a poll stays quick. */
export async function fillOf(t: Fixture, locator: ReturnType<Fixture['app']['view']['testId']>) {
  const view = t.app.view;
  await reveal(t, locator, 0.4);
  const box = await locator.boundingBox();
  const viewport = await view.viewport();
  let scale = 1;
  const py = box.y + box.height / 2 - viewport.top;
  const px = box.x + 6 - viewport.left;
  const shot = await decodePng((await view.screenshot()).base64, width => {
    scale = width / viewport.width;
    return Math.round(py * scale);
  }, () => Math.round(px * scale));
  return shot.at(Math.round(px * scale), Math.round(py * scale));
}

/** The colour of a switch's track near its inline end, where the thumb rests only when off. */
export async function trackFill(t: Fixture, locator: ReturnType<Fixture['app']['view']['testId']>) {
  await reveal(t, locator, 0.4);
  const box = await locator.boundingBox();
  const [pixel] = await sample(t, [[box.x + box.width - 12 - 40, box.y + box.height / 2]]);
  return pixel;
}

/** Swipe until the locator sits wholly on screen; one swipe moves at most a third of the viewport. */
export async function reveal(t: Fixture, locator: ReturnType<Fixture['app']['view']['testId']>, within = 1) {
  const view = t.app.view;
  for (let attempt = 0; attempt < 12; attempt++) {
    const box = await locator.boundingBox();
    const viewport = await view.viewport();
    if (box.y >= viewport.top + 8 && box.y + box.height <= viewport.top + viewport.height * within - 8) return;
    const wanted = box.y + box.height / 2 - (viewport.top + viewport.height * within / 2);
    const dy = Math.sign(wanted) * Math.min(Math.abs(wanted), viewport.height / 3);
    const x = viewport.left + 12, y = viewport.top + viewport.height / 2;
    await view.swipe({ from: [x, y + dy / 2], to: [x, y - dy / 2], durationMs: 600 });
    await new Promise<void>(resolve => setTimeout(resolve, 600));
  }
  throw new Error("the element never came on screen");
}

/** Screen pixels at logical points of the view. */
export async function sample(t: Fixture, points: [number, number][]) {
  const view = t.app.view;
  const viewport = await view.viewport();
  let scale = 1;
  const shot = await decodePng((await view.screenshot()).base64, width => {
    scale = width / viewport.width;
    return Math.round((Math.max(...points.map(([, y]) => y)) - viewport.top + 2) * scale);
  }, () => Math.round((Math.max(...points.map(([x]) => x)) - viewport.left + 2) * scale));
  return points.map(([x, y]) => shot.at(Math.round((x - viewport.left) * scale), Math.round((y - viewport.top) * scale)));
}

/** How many pixel rows the slider's line covers a quarter of the way along, where no thumb sits. */
export async function lineRows(t: Fixture, locator: ReturnType<Fixture['app']['view']['testId']>) {
  const view = t.app.view;
  await reveal(t, locator, 0.4);
  const box = await locator.boundingBox();
  const viewport = await view.viewport();
  let scale = 1;
  const shot = await decodePng((await view.screenshot()).base64, width => {
    scale = width / viewport.width;
    return Math.round((box.y + box.height - viewport.top) * scale);
  }, () => Math.round((box.x + box.width / 4 - viewport.left) * scale));
  const x = Math.round((box.x + box.width / 4 - viewport.left) * scale);
  const top = Math.round((box.y - viewport.top) * scale), bottom = Math.round((box.y + box.height - viewport.top) * scale);
  const page = shot.at(x, top);
  let rows = 0;
  for (let y = top; y < bottom; y++) if (shot.at(x, y).some((value, i) => Math.abs(value - page[i]) > 40)) rows++;
  return rows;
}
