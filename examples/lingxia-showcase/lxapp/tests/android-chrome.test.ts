import { expect, test, type Fixture } from '@lingxia/test';
import { openUnderTabs } from './open-demo';
import { androidDevice, type AndroidNode } from '@lingxia/test/showcase/helpers/android-device.js';

const centre = ({ bounds: [l, t, r, b] }: AndroidNode): [number, number] => [Math.round((l + r) / 2), Math.round((t + b) / 2)];
const sum = (rgb: number[]) => rgb[0] + rgb[1] + rgb[2];

test.configure({ requires: { args: ['androidDevice'] } });

const area = ({ bounds: [l, t, r, b] }: AndroidNode) => (r - l) * (b - t);

/** The nav title (topmost) and a tab mark (bottommost), host views both, and the GPU page's view. */
async function bars(device: ReturnType<typeof androidDevice>) {
  const layout = await device.nodes();
  const one = (what: string, nodes: AndroidNode[], order: (a: AndroidNode, b: AndroidNode) => number) => {
    const node = nodes.filter(node => area(node) > 0).sort(order)[0];
    if (!node) throw new Error(`no ${what} on screen`);
    return node;
  };
  const title = one('nav title', layout.filter(node => ['View lab', '视图实验'].includes(node.text)), (a, b) => a.bounds[1] - b.bounds[1]);
  // This page's tab label, or the tab item just before it when the label reports no size.
  const tab = one('tab mark', layout.flatMap((node, i) =>
    !['View Lab', '实验室'].includes(node.text) ? [] : area(node) > 0 ? [node] : layout.slice(Math.max(0, i - 1), i)), (a, b) => b.bounds[1] - a.bounds[1]);
  const width = Math.max(...layout.map(node => node.bounds[2]));
  // The GPU view reports itself as a plain View; it spans the window under the bars.
  const surface = one('GPU page view', layout.filter(node => node.className === 'android.view.View' || node.className?.endsWith('GpuView')), (a, b) => area(b) - area(a));
  return { title, tab, width, surface };
}

/** A page element's box in screen pixels, through the GPU page's view. */
async function screenBox(t: Fixture, surface: AndroidNode, testId: string) {
  const [left, top, right] = surface.bounds;
  const scale = (right - left) / (await t.app.view.viewport()).width;
  const box = await t.app.view.testId(testId).boundingBox();
  return { left: left + box.x * scale, top: top + box.y * scale, right: left + (box.x + box.width) * scale, scale };
}

// Around a GPU page the nav bar takes the page palette's surface colour, which a card on the page
// also uses, in either scheme; the page config names no colour, so its defaults would differ in dark.
test('the nav bar takes the GPU page palette in light and in dark on Android', {
  id: 'CHROME-NAV-PALETTE-ANDROID-001', covers: ['lx.host.appearance.setPreference'],
  reason: 'reads the composited screen through the Android device fixture',
  start: openUnderTabs('dialog'),
}, async (t) => {
  const app = t.app;
  const device = androidDevice(t);
  const previous = await app.logic.eval(({ lx }) => lx.host.appearance.getPreference() ?? null);
  if (previous === null) t.skip('only the Control app sets the appearance');
  const colours = async () => {
    const { title, width, surface } = await bars(device);
    // The Overlays card's heading row, short of its right edge: the card's own surface.
    const card = await screenBox(t, surface, 'lab-dialog-demo');
    const [nav, own] = await device.pixels([
      [Math.round(width * 0.1), centre(title)[1]],
      [Math.round(card.right - 20 * card.scale), Math.round(card.top + 26 * card.scale)],
    ]);
    return { nav, surface: own };
  };
  try {
    for (const scheme of ['dark', 'light'] as const) {
      await app.logic.eval(async ({ lx }, scheme) => { await lx.host.appearance.setPreference(scheme); }, scheme);
      let last = { nav: [0, 0, 0], surface: [0, 0, 0] };
      await expect.poll(async () => {
        last = await colours();
        return (scheme === 'dark') === (sum(last.surface) < 200) && last.nav.every((v, i) => Math.abs(v - last.surface[i]) <= 6)
          ? 'match' : `nav ${last.nav} surface ${last.surface}`;
      }, { timeout: 10_000, message: `in ${scheme} the nav bar is the page's surface colour` }).toBe('match');
    }
  } finally {
    if (previous !== null) await app.logic.eval(async ({ lx }, previous) => { await lx.host.appearance.setPreference(previous); }, previous);
  }
});

// A page dialog's backdrop covers the host's bars as well as the page: a bar point dims as much as
// a page point, and the bars' own text stays visible through it.
test('a page dialog dims the nav and tab bars as it dims the page on Android', {
  id: 'TOP-LAYER-BARS-ANDROID-001', covers: ['view:dialog'],
  reason: 'reads the composited screen through the Android device fixture',
  start: openUnderTabs('dialog'),
}, async (t) => {
  const view = t.app.view;
  const device = androidDevice(t);
  // The dimming is measured on lit bars: a phone in dark mode runs the page light for the check.
  const previous = await t.app.logic.eval(({ lx }) => lx.host.appearance.getPreference() ?? null);
  if (previous !== null && previous !== 'light') {
    await t.app.logic.eval(async ({ lx }) => { await lx.host.appearance.setPreference('light'); });
    t.defer(async () => {
      await t.app.logic.eval(async ({ lx }, previous) => { await lx.host.appearance.setPreference(previous); }, previous);
    });
  }
  const { title, tab, surface } = await bars(device);
  // Points across the page's own view: in landscape a cutout keeps the window off the screen's edge.
  const [left, , right] = surface.bounds;
  const across = (f: number) => Math.round(left + (right - left) * f);
  const nav: [number, number] = [across(0.1), centre(title)[1]];
  const bar: [number, number] = [across(0.02), centre(tab)[1]];
  const page: [number, number] = [across(0.9), Math.round(centre(title)[1] + (centre(tab)[1] - centre(title)[1]) * 0.8)];
  // A glyph pixel of each label: the point on its centre row farthest from its bar's background.
  const row = ({ bounds: [l, , r] }: AndroidNode, y: number) =>
    Array.from({ length: 16 }, (_, i) => [Math.round(l + (r - l) * (i + 0.5) / 16), y] as [number, number]);
  const glyph = async (node: AndroidNode, background: number[]) => {
    const points = row(node, centre(node)[1]);
    const colours = await device.pixels(points);
    const distance = colours.map(rgb => Math.abs(sum(rgb) - sum(background)));
    return points[distance.indexOf(Math.max(...distance))];
  };
  const before = await device.pixels([nav, bar, page]);
  for (const rgb of before) expect(sum(rgb), `lit before the dialog: ${rgb}`).toBeGreaterThan(150);
  const text = [await glyph(title, before[0]), await glyph(tab, before[1])];
  const textBefore = await device.pixels(text);

  await view.testId('lab-overlay-confirm').click();
  const dialog = view.getByRole('dialog', { name: 'Delete note?' });
  await expect(dialog).toBeVisible();
  await new Promise<void>(resolve => setTimeout(() => resolve(), 500));
  const after = await device.pixels([nav, bar, page]);
  const textAfter = await device.pixels(text);
  const ratio = (i: number) => sum(after[i]) / sum(before[i]);
  expect(ratio(2), `the backdrop dims the page: ${before} -> ${after}`).toBeLessThan(0.9);
  expect(Math.abs(ratio(0) - ratio(2)), `the nav bar dims as the page does: ${before[0]} -> ${after[0]}`).toBeLessThan(0.1);
  expect(Math.abs(ratio(1) - ratio(2)), `the tab bar dims as the page does: ${before[1]} -> ${after[1]}`).toBeLessThan(0.1);
  // The label's contrast with its bar, dimmed by the page's ratio, survives the backdrop.
  const shown = [0, 1].map(i => Math.abs(sum(textAfter[i]) - sum(after[i])) >= 0.5 * ratio(2) * Math.abs(sum(textBefore[i]) - sum(before[i])));
  await view.testId('lab-dialog-cancel').click();
  await expect(dialog).not.toBeAttached();
  if (!shown.every(Boolean)) {
    t.fail(`bar labels hidden under the dialog (nav ${textBefore[0]} -> ${textAfter[0]}, tab ${textBefore[1]} -> ${textAfter[1]}); the surface covers the bars until it composites above the window`);
  }
});
