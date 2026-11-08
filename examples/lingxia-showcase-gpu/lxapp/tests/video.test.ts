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

spec('useVideo reports playback and pauses on command', { start: { page: 'video' } }, async (t) => {
  const view = t.app.view;
  const state = view.testId('video-hero-state');
  const seconds = async () => Number(/([\d.]+)s \//.exec(await state.textContent())?.[1] ?? NaN);
  await expect(state).toContainText('playing');
  const start = await seconds();
  await expect.poll(seconds, { timeout: 5_000 }).toBeGreaterThan(start);
  await view.testId('video-hero-toggle').click();
  await expect(state).toContainText('paused');
  const stopped = await seconds();
  await new Promise<void>(resolve => setTimeout(resolve, 600));
  expect(await seconds()).toBe(stopped);
  await view.testId('video-hero-toggle').click();
  await expect(state).toContainText('playing');
});
