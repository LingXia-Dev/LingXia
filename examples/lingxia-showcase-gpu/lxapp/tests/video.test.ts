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

const seconds = (text: string) => Number(/([\d.]+)s\s*$/.exec(text)?.[1] ?? NaN);

spec('a quality switch keeps the position and the frames moving', { start: { page: 'video' } }, async (t) => {
  const view = t.app.view;
  const state = view.testId('video-quality-state');
  await expect(state).toContainText('auto · 360p');
  await expect.poll(async () => seconds(await state.textContent()), { timeout: 5_000 }).toBeGreaterThan(1);
  const before = seconds(await state.textContent());
  const asked = Date.now();
  await view.testId('video-quality-720p').click();
  await expect(state).toContainText('720p · 720p', { timeout: 5_000 });
  const after = seconds(await state.textContent());
  // The clip loops every 8 s; the position follows the wall clock across the switch.
  const expected = (before + (Date.now() - asked) / 1000) % 8;
  expect(Math.abs(after - expected) < 0.5 || Math.abs(after - expected) > 7.5).toBe(true);
  await expect.poll(async () => seconds(await state.textContent()), { timeout: 3_000 }).not.toBe(after);
  await view.testId('video-quality-logic-360p').click();
  await expect(state).toContainText('360p · 360p', { timeout: 5_000 });
});

spec('lx.createVideoContext pauses and plays the page player from Logic', { start: { page: 'video' } }, async (t) => {
  const view = t.app.view;
  const state = view.testId('video-hero-state');
  await expect(state).toContainText('playing');
  await view.testId('video-hero-logic-pause').click();
  await expect(state).toContainText('paused');
  await view.testId('video-hero-logic-play').click();
  await expect(state).toContainText('playing');
});

// Needs the network: the row streams Apple's public HLS test stream, so an unreachable stream skips.
spec('an HLS source offers its renditions as qualities', { start: { page: 'video' } }, async (t) => {
  const view = t.app.view;
  const state = view.testId('video-stream-state');
  const offline = () => t.skip('needs the network: the public HLS test stream is unreachable');
  const deadline = Date.now() + 20_000;
  let text = await state.textContent();
  while (!text.includes('1080p/720p/540p/360p') && Date.now() < deadline) {
    if (text.includes('VIDEO_NETWORK')) offline();
    await new Promise(resolve => setTimeout(resolve, 250));
    text = await state.textContent();
  }
  if (!text.includes('360p')) offline();
  await view.testId('video-stream-360p').click();
  await expect(state).toContainText('360p · 360p', { timeout: 30_000 });
});
