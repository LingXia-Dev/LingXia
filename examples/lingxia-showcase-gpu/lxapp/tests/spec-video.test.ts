import { expect, spec } from '@lingxia/test';
import { skipWhenCovered } from './covered.js';

// The media specs play the bundled clip (`media=packaged`): the lab's own player streams public video.
spec('kit video controls start a poster video on a tap, then pause, mute and scrub it', { start: { page: 'spec-video', query: { media: 'packaged' } } }, async (t) => {
  await skipWhenCovered(t, 'spec-video');
  const view = t.app.view;
  const button = (name: string) => view.getByRole('button', { name });
  const time = view.testId('lab-video-controls-time');
  const seconds = async () => {
    const [, m, s] = /^(\d+):(\d+)/.exec(await time.textContent()) ?? [];
    return Number(m) * 60 + Number(s);
  };
  // Not autoplaying: it waits on its poster with the length known. The spinner beside it commits
  // the page every frame, and the player's state must reach the bar all the same.
  await expect(view.testId('lab-video-spinner')).toBeVisible();
  await expect(button('Play')).toBeVisible();
  await expect(time).toHaveText('0:00 / 0:06', { timeout: 5_000 });
  await button('Play').click();
  await expect(button('Pause')).toBeVisible();
  await expect.poll(seconds, { timeout: 5_000 }).toBeGreaterThan(0);
  await button('Pause').click();
  await expect(button('Play')).toBeVisible();
  await button('Mute').click();
  await expect(button('Unmute')).toBeVisible();
  // A drag to the far end seeks there and stays paused (at the end, the button offers Replay). It
  // starts on the line: on a narrow bar the slider's touch box reaches down over the buttons.
  const seek = await view.getByRole('slider', { name: 'Seek' }).boundingBox();
  const finger = await view.touch([seek.x + seek.width / 10, seek.y + seek.height / 4]);
  await finger.moveTo([seek.x + seek.width + 40, seek.y + seek.height / 4], { durationMs: 400 });
  await finger.up();
  await expect.poll(seconds, { timeout: 5_000 }).toBeGreaterThanOrEqual(5);
  await expect(button('Pause')).toHaveCount(0);
});

spec('kit video controls show the player full screen and Escape brings it back', { start: { page: 'spec-video', query: { media: 'packaged' } } }, async (t) => {
  await skipWhenCovered(t, 'spec-video');
  const view = t.app.view;
  await view.testId('lab-video-controls-play').click();
  await view.testId('lab-video-controls-fullscreen').click();
  const layer = view.testId('lab-video-controls-fullscreen-view');
  await expect(layer).toBeVisible();
  // The same player, so the full screen bar reads its time as it plays on.
  const time = view.testId('lab-video-controls-full-time');
  const seconds = async () => Number(/^\d+:(\d+)/.exec(await time.textContent())?.[1] ?? NaN);
  const start = await seconds();
  await expect.poll(seconds, { timeout: 5_000 }).toBeGreaterThan(start);
  const area = await view.viewport();
  const box = await layer.boundingBox();
  expect(box.width).toBeGreaterThanOrEqual(area.width - 1);
  await layer.press('Escape');
  await expect(layer).not.toBeAttached();
  expect((await t.app.nav.current()).name).toBe('spec-video');
  await view.testId('lab-video-controls-fullscreen').click();
  await expect(layer).toBeVisible();
  await view.testId('lab-video-controls-full-fullscreen').click();
  await expect(layer).not.toBeAttached();
});

// A desktop window keeps the shape its user gave it; a phone turns to the picture.
spec('a landscape full-screen video turns the window and leaving gives it back', { start: { page: 'spec-video', query: { media: 'packaged' } }, requires: { input: ['touch'] } }, async (t) => {
  const view = t.app.view;
  const wide = async () => { const area = await view.viewport(); return area.width > area.height; };
  // Just after launch a window the sensor turned can still report its old shape: read it settled.
  let before = await wide();
  for (let i = 0; i < 6; i++) {
    await new Promise<void>(resolve => setTimeout(resolve, 500));
    const now = await wide();
    if (now === before) break;
    before = now;
  }
  await view.testId('lab-video-controls-play').click();
  await view.testId('lab-video-controls-fullscreen').click();
  const layer = view.testId('lab-video-controls-fullscreen-view');
  await expect(layer).toBeVisible();
  await expect.poll(wide, { timeout: 5_000 }).toBe(true);
  await view.testId('lab-video-controls-full-fullscreen').click();
  await expect(layer).not.toBeAttached();
  await expect.poll(wide, { timeout: 5_000 }).toBe(before);
});
