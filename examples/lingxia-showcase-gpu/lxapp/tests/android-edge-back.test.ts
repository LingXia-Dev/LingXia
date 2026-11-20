import { expect, test } from '@lingxia/test';
import { androidDevice } from '@lingxia/test/showcase/helpers/android-device.js';

test.configure({ requires: { args: ['androidDevice'] } });

// Platform convention: a swipe from inside the system's edge zone is back, even across a
// horizontal control; a drag that starts on the control past that zone moves the control.
test('an edge swipe goes back across a pager; a drag starting on the pager pages it', {
  id: 'EDGE-BACK-CLAIM-ANDROID-001', covers: ['view:scroll', 'lx.nav.back'],
  reason: 'swipes through the OS input path of the Android device fixture',
  start: { page: 'view' },
}, async (t) => {
  const app = t.app;
  const view = app.view;
  const device = androidDevice(t);
  if (!(await device.canInject())) t.skip('the device rejects injected input (MIUI: turn on "USB debugging (Security settings)")');
  // Page logical pixels to screen pixels, through the GPU page's view.
  const screenOf = async (testId: string) => {
    const surface = (await device.nodes()).filter(node => node.className?.endsWith('GpuView'))
      .reduce((a, b) => ((b.bounds[2] - b.bounds[0]) * (b.bounds[3] - b.bounds[1]) > (a.bounds[2] - a.bounds[0]) * (a.bounds[3] - a.bounds[1]) ? b : a));
    const [left, top, right] = surface.bounds;
    const scale = (right - left) / (await view.viewport()).width;
    const box = await view.testId(testId).boundingBox();
    return { left: Math.round(left + box.x * scale), right: Math.round(left + (box.x + box.width) * scale),
      y: Math.round(top + (box.y + box.height / 2) * scale), scale };
  };
  const onPage = async (page: number) => {
    await expect(view.testId('motion-pager-state')).toHaveAttribute('data-state', `page=${page}`);
  };

  await app.nav.to({ page: 'view-scrolling' });
  await view.testId('motion-pager-next').click();
  await onPage(1);

  // On the pager, past the 16 dp hard back zone.
  const pager = await screenOf('motion-pager-scroll');
  const start = pager.left + Math.round(4 * pager.scale);
  expect(start / pager.scale, 'the drag starts past the hard back zone').toBeGreaterThan(16);
  await device.swipe({ x1: start, y1: pager.y, x2: pager.right - 10, y2: pager.y, duration: 300 });
  await onPage(0);
  expect((await app.nav.current()).name, 'a drag on the pager stays on the page').toBe('view-scrolling');

  await view.testId('motion-pager-next').click();
  await onPage(1);
  // From the screen edge, across the same pager: back.
  await device.swipe({ x1: 1, y1: pager.y, x2: pager.right - 10, y2: pager.y, duration: 300 });
  await expect.poll(async () => (await app.nav.current()).name).toBe('view');
});
