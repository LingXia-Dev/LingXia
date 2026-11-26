import { expect, test, type Locator, type TestView } from '@lingxia/test';
import { decodePng } from './png';

// What is drawn, not the class asked for: an ink with alpha laid once per mark has one tone, so
// nothing inside an icon is darker than its strokes. Light theme: the ink is darker than its ground.

type Tone = number[];
const sum = (tone: Tone) => tone[0] + tone[1] + tone[2];

/** The tones inside a box, in device pixels: the ground (the commonest), the ink (the commonest other), the darkest. */
async function tones(view: TestView, box: { x: number; y: number; width: number; height: number }) {
  const shot = await view.screenshot();
  const scale = shot.width / (await view.viewport()).width;
  const [x0, y0] = [Math.ceil(box.x * scale), Math.ceil(box.y * scale)];
  const [x1, y1] = [Math.floor((box.x + box.width) * scale), Math.floor((box.y + box.height) * scale)];
  const png = await decodePng(shot.base64, () => y1, () => x1);
  const counts = new Map<string, number>();
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) {
    const key = png.at(x, y).join(',');
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const ranked = [...counts].sort((a, b) => b[1] - a[1]).map(([key]) => key.split(',').map(Number));
  const [ground, ink] = ranked;
  const darkest = ranked.reduce((a, b) => (sum(b) < sum(a) ? b : a));
  return { ground, ink, darkest };
}

async function boxOf(locator: Locator) {
  await locator.scrollIntoView();
  const box = await locator.boundingBox();
  if (!box) throw Error('nothing to measure');
  return box;
}

// A few levels of edge rounding between a full-cover pixel and the darkest one.
const near = (a: Tone, b: Tone, by = 9) => Math.abs(sum(a) - sum(b)) <= by;

test('an icon in an ink with alpha has one tone where its strokes cross', { start: { page: 'icon' } }, async (tc) => {
  const view = tc.app.view;
  await view.environment({ appearance: 'light', still: true });
  const { ground, ink, darkest } = await tones(view, await boxOf(view.testId('faded')));
  expect(sum(ink)).toBeLessThan(sum(ground));
  expect({ ink, darkest, alike: near(ink, darkest) }).toEqual(expect.objectContaining({ alike: true }));
});

test('a disabled token draws its remove mark in its label\'s tone', { start: { page: 'token' } }, async (tc) => {
  const view = tc.app.view;
  await view.environment({ appearance: 'light', still: true });
  const token = await boxOf(view.testId('locked'));
  const remove = await boxOf(view.testId('locked-remove'));
  // Inside the pill's rounded start, in the label's band: the page outside the pill is not counted.
  const inset = token.height / 2;
  const label = await tones(view, { x: token.x + inset, y: token.y + token.height / 4, width: remove.x - token.x - inset, height: token.height / 2 });
  const mark = await tones(view, remove);
  expect({ label: label.ink, mark: mark.ink, alike: near(label.ink, mark.ink, 3) }).toEqual(expect.objectContaining({ alike: true }));
  expect({ ink: mark.ink, darkest: mark.darkest, alike: near(mark.ink, mark.darkest) }).toEqual(expect.objectContaining({ alike: true }));
});
