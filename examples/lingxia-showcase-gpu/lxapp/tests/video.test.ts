import { expect, spec } from '@lingxia/test';

spec('a video plays in a 16:9 box and under its overlay', { start: { page: 'video' } }, async (t) => {
  const view = t.app.view;
  const hero = view.testId('video-hero');
  await expect(hero).toBeVisible();
  const box = await hero.boundingBox();
  expect(Math.abs(box.width / box.height - 16 / 9)).toBeLessThan(0.02);
  await expect(view.testId('video-row-1')).toContainText('overlay above video');
  // Nothing else on the page moves: a changed screenshot is a new video frame.
  const first = (await view.screenshot()).base64;
  await expect.poll(async () => (await view.screenshot()).base64 !== first, { timeout: 10_000 }).toBe(true);
});
