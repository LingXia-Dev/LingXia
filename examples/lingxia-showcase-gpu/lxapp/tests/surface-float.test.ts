import { expect, spec } from '@lingxia/test';
import { androidDevice, type AndroidNode } from '@lingxia/test/showcase/helpers/android-device.js';

const centre = (node: AndroidNode): [number, number] =>
  [Math.round((node.bounds[0] + node.bounds[2]) / 2), Math.round((node.bounds[1] + node.bounds[3]) / 2)];
const same = (a: number[], b: number[]) => a.every((channel, i) => Math.abs(channel - b[i]) <= 8);

// What the docs promise about a float: it is not part of the page that opened it, so it stays open
// while that page navigates, and it sits above the page's own top layer. Read from the composited
// screen, since each sits on its own surface.
spec('a float outlives its opener navigating away and stays above a page dialog', {
  id: 'SURFACE-FLOAT-STACK-001', covers: ['lx.surface.openPage', 'view:dialog'],
  requires: { args: ['androidDevice'] }, reason: 'reads the composited screen through the Android device fixture',
  start: { page: 'view-lab' },
}, async (t) => {
  const app = t.app;
  const device = androidDevice(t);
  const key = `surface-float-stack-${Date.now()}`;
  try {
    await app.logic.eval(async ({ lx }, key) => {
      await lx.surface.openPage('surface', { as: 'float', key });
    }, key);
    await expect((await app.page({ name: 'surface' })).view.testId('surface-counter')).toBeVisible();

    await app.nav.to({ page: 'motion-lab' });
    await app.nav.back();
    expect(await app.logic.eval(({ lx }, key) => lx.surface.getByKey(key)?.alive === true, key)).toBe(true);
    await expect((await app.page({ name: 'surface' })).view.testId('surface-counter')).toBeVisible();

    const float = (await device.nodes()).filter(node => /:id\/surface-(query|show-count|hide-count|lifecycle)$/.test(node.resource));
    expect(float.length).toBeGreaterThanOrEqual(3);
    const left = Math.min(...float.map(node => node.bounds[0]));
    const middle = Math.round(float.reduce((sum, node) => sum + centre(node)[1], 0) / float.length);
    // A page point beside the float, which the full-width sheet below covers.
    expect(left).toBeGreaterThan(20);
    const page: [number, number] = [Math.round(left / 4), middle];
    const points = [...float.map(centre), page];
    const before = await device.pixels(points);

    const lab = (await app.page({ name: 'view-lab' })).view;
    await lab.testId('lab-tab-gesture').click();
    await lab.testId('lab-detents-open').click();
    await expect(lab.getByRole('dialog', { name: 'Places' })).toBeVisible();
    const after = await device.pixels(points);

    expect(same(before.at(-1)!, after.at(-1)!), 'the page dialog shows where the float is not').toBe(false);
    for (let i = 0; i < float.length; i++)
      expect(same(before[i], after[i]), `float pixel ${i} unchanged by the page dialog`).toBe(true);
  } finally {
    await app.logic.eval(async ({ lx }, key) => {
      const surface = lx.surface.getByKey(key);
      if (surface?.kind === 'page') await surface.close();
    }, key);
  }
});
