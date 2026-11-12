import { expect, spec, type Fixture } from '@lingxia/test';
import { rawAutomation } from '@lingxia/test/runner';
import { decodePng } from './png.js';

// The tab bar is a UIKit view: under a page dialog the page draws a copy of it beneath its backdrop.
// A tab icon dims channel by channel as the page does only if that copy is drawn; without it the
// raised page would show its own content there instead.
async function dimsTheBar(t: Fixture) {
  const app = t.app;
  const view = app.view;
  const os = await app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
  if (os !== 'ios') t.skip('reads the UIKit tab bar from the iOS window capture');
  await view.testId('lab-tab-feedback').click();
  // iOS does not expose its chrome to automation: the bar is where the page's own insets put it.
  const lab = view.testId('view-lab');
  const page = await lab.boundingBox();
  const inset = Number(await lab.getAttribute('data-inset-bottom'));
  const safe = Number(await lab.getAttribute('data-safe-bottom'));
  expect(inset, 'a tab bar on the bottom edge').toBeGreaterThan(safe);
  const barTop = page.y + page.height - inset;
  const shot = async () => decodePng((await rawAutomation().lxapps.screenshot()).base64);
  const before = await shot();
  const scale = before.width / page.width;
  const px = (x: number, y: number) => [Math.round(x * scale), Math.round(y * scale)] as [number, number];
  // A grid over the bar strip (icons, labels, background), and an empty page point below the centred dialog.
  // Labels sit in the home-indicator inset, so the rows span the whole inset.
  const bar = [0.35, 0.45, 0.55, 0.8].flatMap(fy => Array.from({ length: 64 }, (_, i) =>
    px(page.x + page.width * (i + 0.5) / 64, barTop + inset * fy)));
  const empty = px(page.x + page.width * 0.9, page.y + page.height * 0.7);
  const sum = (rgb: number[]) => rgb[0] + rgb[1] + rgb[2];
  expect(sum(before.at(...empty)), `page point lit: ${before.at(...empty)}`).toBeGreaterThan(150);

  await view.testId('lab-overlay-confirm').click();
  const dialog = view.getByRole('dialog', { name: 'Delete note?' });
  await expect(dialog).toBeVisible();
  try {
    await new Promise<void>(resolve => setTimeout(() => resolve(), 600));
    // The presented frame draws the host's bar snapshot under its dialog (absent on an older host).
    const drawing = (await rawAutomation().lxapp().page.uiFrame()).chromeSnapshot;
    if (drawing !== undefined) expect(drawing, 'the frame draws a bar snapshot').toBeGreaterThan(0);
    const after = await shot();
    const ratio = sum(after.at(...empty)) / sum(before.at(...empty));
    expect(ratio, `the backdrop dims the page: ${before.at(...empty)} -> ${after.at(...empty)}`).toBeLessThan(0.9);
    // Icon and label pixels differ from the bar background: the page has nothing like them there.
    const ink = bar.filter(([x, y]) => sum(before.at(x, y)) < 600);
    expect(ink.length, 'some samples land on tab icons or labels').toBeGreaterThan(2);
    console.log(`ink ${ink.length}/${bar.length}: ${ink.slice(0, 4).map(([x, y]) => `${before.at(x, y)}->${after.at(x, y)}`).join(' ')}`);
    console.log(`page ratio ${ratio.toFixed(2)}; bar row samples ${bar.slice(64, 68).map(([x, y]) => `${before.at(x, y)}->${after.at(x, y)}`).join(' ')}`);
    for (const [x, y] of bar) {
      const want = before.at(x, y).map(v => v * ratio);
      const got = after.at(x, y);
      expect(got.every((v, i) => Math.abs(v - want[i]) <= 18), `tab bar (${x},${y}) ${before.at(x, y)} -> ${got}, want ~${want.map(Math.round)}`).toBe(true);
    }
  } finally {
    if (await dialog.isVisible()) await view.testId('lab-dialog-cancel').click();
  }
  await expect(dialog).not.toBeAttached();
  await t.waitFor(async () => (await rawAutomation().lxapp().page.uiFrame()).chromeSnapshot ?? 0, { until: generation => generation === 0 });
  await new Promise<void>(resolve => setTimeout(() => resolve(), 400));
  const closed = await shot();
  for (const [x, y] of [...bar, empty]) {
    const got = closed.at(x, y);
    expect(got.every((v, i) => Math.abs(v - before.at(x, y)[i]) <= 12), `(${x},${y}) back after close: ${before.at(x, y)} -> ${got}`).toBe(true);
  }
}

spec('a page dialog dims the iOS tab bar with the page, and the real bar is back on close', {
  id: 'TOP-LAYER-BARS-IOS-001', covers: ['view:dialog'],
  start: { page: 'view-lab' },
}, dimsTheBar);

// A page unloaded with its dialog open hands the bars back: the next page's dialog still draws them.
spec('after a page is unloaded with its dialog open, the next page dialog still dims a drawn tab bar', {
  id: 'TOP-LAYER-BARS-IOS-002', covers: ['view:dialog'],
  start: { page: 'view-lab' },
}, async (t) => {
  const os = await t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase());
  if (os !== 'ios') t.skip('reads the UIKit tab bar from the iOS window capture');
  const view = t.app.view;
  await view.testId('lab-tab-feedback').click();
  await view.testId('lab-overlay-confirm').click();
  await expect(view.getByRole('dialog', { name: 'Delete note?' })).toBeVisible();
  await t.app.nav.relaunch({ page: 'view-lab' });
  await dimsTheBar(t);
});
