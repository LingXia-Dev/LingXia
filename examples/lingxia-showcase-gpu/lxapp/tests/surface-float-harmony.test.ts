import { expect, spec } from '@lingxia/test';
import { harmonyDevice } from '@lingxia/test/showcase/helpers/harmony-device.js';

const same = (a: number[], b: number[]) => a.every((channel, i) => Math.abs(channel - b[i]) <= 24);

// A Harmony float is its own sub-window, so its GPU page must draw there: inside the card the screen
// shows the float's page, not the opener under the scrim. Read from the composited screen, since the
// host capture snapshots only the main window.
spec('a modal float draws its GPU page in its own window', {
  id: 'SURFACE-FLOAT-HARMONY-001', covers: ['lx.surface.openPage'],
  requires: { args: ['harmonyDevice'] }, reason: 'reads the composited screen through the Harmony device fixture',
  start: { page: 'view' },
}, async (t) => {
  const app = t.app;
  const device = harmonyDevice(t);
  const key = `surface-float-harmony-${Date.now()}`;
  const nodes = await device.nodes();
  const width = Math.max(...nodes.map(node => node.bounds[2]));
  const height = Math.max(...nodes.map(node => node.bounds[3]));
  // An 8 x 4 grid (the fixture's 32-point cap) over the middle of a centred 60% x 40% card.
  const card = [0.37, 0.45, 0.53, 0.61].flatMap(y => [0.26, 0.33, 0.4, 0.47, 0.54, 0.61, 0.68, 0.74]
    .map(x => [Math.round(width * x), Math.round(height * y)] as [number, number]));
  const before = await device.pixels(card);
  try {
    await app.logic.eval(async ({ lx }, key) => {
      await lx.surface.openPage('surface', { as: 'float', key, size: { width: '60%', height: '40%' }, interaction: { modal: true } });
    }, key);
    await expect((await app.page({ name: 'surface' })).view.testId('surface-counter')).toBeAttached();
    await new Promise<void>(resolve => setTimeout(() => resolve(), 600));
    const after = await device.pixels(card);
    // Not the opener under a 45% scrim, which is all an undrawn float window leaves.
    const replaced = after.filter((rgb, i) => !same(rgb, before[i].map(v => v * 0.55))).length;
    expect(replaced, `card points that are not the dimmed opener: ${replaced}/${card.length}`).toBeGreaterThan(card.length / 2);
    // The page's text, not only its floor: dark points in a dense scan, read 32 at a time.
    const scan: [number, number][] = [];
    for (let y = 0.32; y < 0.68; y += 0.012) {
      for (let x = 0.24; x < 0.76; x += 0.04) scan.push([Math.round(width * x), Math.round(height * y)]);
    }
    let ink = 0;
    for (let i = 0; i < scan.length; i += 32) {
      ink += (await device.pixels(scan.slice(i, i + 32))).filter(rgb => Math.max(...rgb) < 160).length;
    }
    expect(ink, `dark text points in the card: ${ink}/${scan.length}`).toBeGreaterThan(3);
  } finally {
    await app.logic.eval(async ({ lx }, key) => {
      const surface = lx.surface.getByKey(key);
      if (surface?.kind === 'page') await surface.close();
    }, key);
  }
});
