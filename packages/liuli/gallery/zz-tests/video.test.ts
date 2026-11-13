import { expect, spec } from '@lingxia/test';

// Liuli's VideoPlayer on a device: the first play, the bar over the picture, a drag that seeks, and
// full screen as a dialog that the close request leaves.
const pause = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));

spec('video player: plays, seeks by a drag, and full screen opens and closes', { start: { page: 'video-player' }, timeout: 90_000 }, async (t) => {
  const view = t.app.view;
  const id = (part: string) => view.testId(`standard-controls-${part}`);
  await t.attach('0-rest.png', await view.screenshot());
  await id('start').click();
  await expect(id('scrub')).toBeEnabled({ timeout: 15_000 });
  await id('play').click();
  await pause(400);
  await t.attach('1-paused.png', await view.screenshot());
  const box = await id('scrub').boundingBox();
  if (!box) throw Error('no scrubber');
  const y = box.y + box.height / 2;
  const finger = await view.touch([box.x + box.width * 0.2, y]);
  await finger.moveTo([box.x + box.width * 0.6, y], { durationMs: 300 });
  await t.attach('2-dragged.png', await view.screenshot());
  await finger.up();
  await pause(800);
  expect(await id('scrub').getAttribute('aria-valuetext')).toMatch(/^0:0[5-7] \/ 0:1\d$/);
  await id('settings').click();
  await expect(id('menu')).toBeAttached();
  await pause(300);
  await t.attach('2b-menu.png', await view.screenshot());
  await id('menu-dismiss').click();
  await expect(id('menu')).toHaveCount(0);
  await id('fullscreen').click();
  await expect(view.testId('standard-controls-fullscreen-view')).toBeAttached();
  await pause(600);
  await t.attach('3-fullscreen.png', await view.screenshot());
  await view.testId('standard-controls-full-close').click();
  await expect(view.testId('standard-controls-fullscreen-view')).toHaveCount(0);
});
