import { expect, test } from '@lingxia/test';
import { harmonyDevice } from '@lingxia/test/showcase/helpers/harmony-device.js';

test.configure({ requires: { args: ['harmonyDevice'] } });

// Platform convention: a swipe from inside the system's edge zone is back, even across a
// horizontal control; a drag that starts on the control past that zone moves the control.
test('an edge swipe goes back across a pager; a drag starting on the pager pages it', {
  id: 'EDGE-BACK-CLAIM-001', covers: ['view:scroll', 'lx.nav.back'],
  reason: 'swipes through the system input path of the Harmony device fixture',
  start: { page: 'view' },
}, async (t) => {
  const app = t.app;
  const view = app.view;
  const device = harmonyDevice(t);
  // Page logical pixels to screen pixels, through the GPU page's XComponent node.
  const screenOf = async (testId: string) => {
    const surface = (await device.nodes()).filter(node => node.type === 'XComponent')
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

  await app.nav.go({ page: 'view-scrolling' });
  await view.testId('motion-pager-next').click();
  await onPage(1);

  // On the pager, past the system's edge zone and still inside the host's 40 vp back strip.
  const pager = await screenOf('motion-pager-scroll');
  const start = pager.left + Math.round(4 * pager.scale);
  expect(start / pager.scale, 'the drag starts past the system edge zone').toBeGreaterThan(16);
  await device.swipe({ x1: start, y1: pager.y, x2: pager.right - 10, y2: pager.y, velocity: 1500 });
  await onPage(0);
  expect((await app.nav.current()).name, 'a drag on the pager stays on the page').toBe('view-scrolling');

  await view.testId('motion-pager-next').click();
  await onPage(1);
  // From the screen edge, across the same pager: back.
  await device.swipe({ x1: 2, y1: pager.y, x2: pager.right - 10, y2: pager.y, velocity: 1500 });
  await expect.poll(async () => (await app.nav.current()).name).toBe('view');
});
