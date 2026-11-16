import { expect, spec, type Fixture } from '@lingxia/test';
import { openDemo } from './open-demo';
import { harmonyDevice, type HarmonyNode } from '@lingxia/test/showcase/helpers/harmony-device.js';

spec.configure({ requires: { args: ['harmonyDevice'] } });

const centre = ({ bounds: [l, t, r, b] }: HarmonyNode): [number, number] => [Math.round((l + r) / 2), Math.round((t + b) / 2)];
const sum = (rgb: number[]) => rgb[0] + rgb[1] + rgb[2];

/** The page surface on screen, and its device pixels per page pixel. */
async function surface(t: Fixture) {
  const device = harmonyDevice(t);
  const node = (await device.nodes()).filter(n => n.type === 'XComponent')
    .reduce((a, b) => ((b.bounds[2] - b.bounds[0]) * (b.bounds[3] - b.bounds[1]) > (a.bounds[2] - a.bounds[0]) * (a.bounds[3] - a.bounds[1]) ? b : a));
  const viewport = await t.app.view.viewport();
  const scale = (node.bounds[2] - node.bounds[0]) / viewport.width;
  // The node's top excludes the band the page draws under the title bar; its bottom is the page's.
  return { left: node.bounds[0], top: node.bounds[3] - viewport.height * scale, scale };
}

/** The topmost layout node whose text is one of `labels`, waiting up to `timeout` for one. */
async function waitForText(t: Fixture, labels: string[], timeout = 3000): Promise<HarmonyNode> {
  const device = harmonyDevice(t);
  for (const end = Date.now() + timeout; ;) {
    // Pages below the current one stay in the layout; the current page comes last.
    const found = (await device.nodes()).filter(node => labels.includes(node.text)).pop();
    if (found) return found;
    if (Date.now() > end) throw new Error(`none of ${labels.join(', ')} on screen`);
    await new Promise<void>(resolve => setTimeout(() => resolve(), 200));
  }
}

// The host draws the selection chrome a GPU surface cannot: handles and the floating menu, whose
// actions the core runs. Driven by a real finger, read from the ArkUI layout.
spec('a long press selects a word, and the host menu selects all and copies it', {
  id: 'SELECT-HARMONY-001', covers: ['view:text'],
  reason: 'long-presses through the Harmony device fixture and reads the host menu from the layout',
  start: openDemo('selecting'),
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  const text = view.testId('lab-select-text');
  const page = await surface(t);
  const half = (await view.viewport()).height / 2;
  // The sentence sits below the fold; a finger scrolls it to mid-screen, clear of the gesture bar.
  // It swipes in the page's left margin: a card under it may be a scroller that takes the swipe.
  const margin = Math.round(page.left + 8 * page.scale);
  for (let i = 0; i < 6 && (await text.boundingBox()).y > half; i++)
    await device.swipe({ x1: margin, y1: 1800, x2: margin, y2: 1100, velocity: 2000 });
  // A swipe flings: wait for the scroll to come to rest before aiming.
  let box = await text.boundingBox();
  for (let last = NaN; box.y !== last; box = await text.boundingBox()) {
    last = box.y;
    await new Promise<void>(resolve => setTimeout(() => resolve(), 300));
  }
  // The first word, "Long".
  await device.longClick(Math.round(page.left + (box.x + 12) * page.scale), Math.round(page.top + (box.y + 10) * page.scale));
  const all = await waitForText(t, ['Select All', '全选']);
  // Knobs hang just below the selected lines; other circles (the tab bar's) sit elsewhere.
  const knobs = async () => (await device.nodes()).filter(node => node.type === 'Circle'
    && centre(node)[1] > page.top + box.y * page.scale && centre(node)[1] < page.top + (box.y + box.height + 30) * page.scale);
  const word = await knobs();
  expect(word.length, 'both ends of the word carry a handle').toBe(2);
  expect(centre(word[1])[1], 'one word stays on one line').toBe(centre(word[0])[1]);
  await device.click(...centre(all));
  // The whole two-line sentence: the end handle drops to the second line.
  await expect.poll(async () => {
    const ends = await knobs();
    return ends.length === 2 ? centre(ends[1])[1] - centre(ends[0])[1] : 0;
  }, { timeout: 8000 }).toBeGreaterThan(box.height / 2 * page.scale);
  // Reading the pasteboard back needs an ACL permission apps do not get; the menu closing is
  // the host's evidence that the core ran the copy. Pages left below this one keep their own
  // menus in the layout, so count rather than look for none.
  const copies = async () => (await device.nodes()).filter(node => ['Copy', '复制'].includes(node.text)).length;
  const open = await copies();
  await device.click(...centre(await waitForText(t, ['Copy', '复制'])));
  await expect.poll(copies, { timeout: 8000 }).toBe(open - 1);
});

// Press feedback is host chrome over the surface: while a finger rests on a control, its box darkens.
spec('a finger held on a control shows the press overlay until it lifts', {
  id: 'PRESS-HARMONY-001', covers: ['view:button'],
  reason: 'holds a finger through the Harmony device fixture and reads composited pixels',
  start: openDemo('jump'),
}, async (t) => {
  const view = t.app.view;
  const device = harmonyDevice(t);
  // A button whose click changes nothing here, so the pixels can settle back.
  const chip = view.testId('lab-reveal-first');
  await chip.click();
  const page = await surface(t);
  const box = await chip.boundingBox();
  // Inside the chip's fill, clear of its label.
  const point: [number, number] = [Math.round(page.left + (box.x + 6) * page.scale), Math.round(page.top + (box.y + box.height / 2) * page.scale)];
  const [before] = await device.pixels([point]);
  const [during] = await device.heldPixels(...point, [point]);
  await new Promise<void>(resolve => setTimeout(() => resolve(), 500));
  const [after] = await device.pixels([point]);
  expect(Math.abs(sum(during) - sum(before)), `pressed differs: ${before} -> ${during}`).toBeGreaterThan(15);
  expect(Math.abs(sum(after) - sum(before)), `released restores: ${before} -> ${after}`).toBeLessThan(10);
});
