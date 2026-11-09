import { expect, spec } from '@lingxia/test';
import { decodePng } from './png.js';

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
  // Playing and past its first frames, so a blank box now is a finding, not a race.
  const time = async () => Number(/([\d.]+)s \//.exec(await view.testId('video-hero-state').textContent())?.[1] ?? NaN);
  await expect.poll(time, { timeout: 5_000 }).toBeGreaterThan(0.3);
  await expectPicture(t, hero);
});

type Fixture = Parameters<Parameters<typeof spec>[2]>[0];
type View = Fixture['app']['view'];
type Viewport = Awaited<ReturnType<View['viewport']>>;

/** Frames that only count are not a picture: the box shows colour, not black or the page. Call it
 * once the player has shown frames, so a blank box is a finding, not a race. */
async function expectPicture(t: Fixture, video: ReturnType<View['testId']>) {
  const view = t.app.view;
  let box = await video.boundingBox();
  let viewport = await view.viewport();
  if (box.y < viewport.top || box.y + box.height > viewport.top + viewport.height) {
    const dy = box.y + box.height / 2 - (viewport.top + viewport.height / 2);
    const x = viewport.left + 12;
    const y = viewport.top + viewport.height / 2;
    await view.swipe({ from: [x, y + dy / 2], to: [x, y - dy / 2], durationMs: 600 });
    await new Promise<void>(resolve => setTimeout(resolve, 600));
    box = await video.boundingBox();
    viewport = await view.viewport();
  }
  const points: [number, number][] = [[box.x + box.width / 2, box.y - 6], ...[0.5, 0.3, 0.7].map(fx => [box.x + box.width * fx, box.y + box.height * 0.5] as [number, number])];
  const [page, ...samples] = await sample(t, points, viewport);
  const lit = samples.filter(([r, g, b]) => Math.max(r, g, b) > 40);
  const apart = samples.filter(rgb => rgb.some((value, i) => Math.abs(value - page[i]) > 30));
  expect(lit.length > 0 && apart.length > 0 ? 'picture' : `blank ${JSON.stringify({ samples, page })}`).toBe('picture');
}

/** Colours at page points from the page screenshot, decoded only down to the lowest point. */
async function sample(t: Fixture, points: [number, number][], viewport: Viewport) {
  let scale = 1;
  const at = ([x, y]: [number, number]) => [Math.round((x - viewport.left) * scale), Math.round((y - viewport.top) * scale)] as const;
  const shot = await decodePng((await t.app.view.screenshot()).base64, width => {
    scale = width / viewport.width;
    return Math.max(...points.map(point => at(point)[1]));
  });
  return points.map(point => shot.at(...at(point)));
}

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
  // The clip loops every 8 s; the position follows the wall clock across the switch. Each label
  // reading may be a 250 ms status tick old, so the seam itself gets the 0.5 s left over.
  const expected = (before + (Date.now() - asked) / 1000) % 8;
  const off = Math.abs(after - expected);
  expect(Math.min(off, 8 - off)).toBeLessThan(0.75);
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

spec('a page under another holds its videos still and plays the playing ones again on return', { start: { page: 'video' } }, async (t) => {
  const view = t.app.view;
  const state = view.testId('video-hero-state');
  const at = async () => Number(/([\d.]+)s \//.exec(await state.textContent())?.[1] ?? NaN);
  const away = async () => {
    await t.app.nav.to({ page: 'todo' });
    await new Promise<void>(resolve => setTimeout(resolve, 2_500));
    await t.app.nav.back();
    await expect(state).toBeVisible();
  };
  await expect(state).toContainText('playing');
  // Early in the 6 s loop, so 2.5 s of hidden playback could not wrap back near the start.
  await expect.poll(at, { timeout: 8_000 }).toBeLessThan(2.5);
  const before = await at();
  await away();
  const back = await at();
  expect(back - before).toBeGreaterThan(-0.1);
  expect(back - before).toBeLessThan(0.8);
  await expect(state).toContainText('playing');
  await expect.poll(at, { timeout: 5_000 }).toBeGreaterThan(back);
  // One the user paused stays paused.
  await view.testId('video-hero-toggle').click();
  await expect(state).toContainText('paused');
  // The player stops within a frame or two of the state flipping; read where it settled.
  await new Promise<void>(resolve => setTimeout(resolve, 600));
  const paused = await at();
  await away();
  await expect(state).toContainText('paused');
  expect(await at()).toBe(paused);
});

// Needs the network: the row streams Apple's public HLS test stream, so an unreachable stream skips.
spec('an HLS source offers its renditions as qualities', { start: { page: 'video' } }, async (t) => {
  const view = t.app.view;
  const state = view.testId('video-stream-state');
  const offline = () => t.skip('needs the network: the public HLS test stream is unreachable');
  // HarmonyOS's player plays the stream but does not list or pin its renditions yet.
  const harmony = await t.app.logic.eval(({ lx }) => String(lx.host.getBaseInfo().os || '').toLowerCase()) === 'harmony';
  const done = (text: string) => harmony ? /· [1-9]\d*p ·/.test(text) : text.includes('1080p/720p/540p/360p');
  const deadline = Date.now() + 20_000;
  let text = await state.textContent();
  while (!done(text) && Date.now() < deadline) {
    if (text.includes('VIDEO_NETWORK')) offline();
    await new Promise<void>(resolve => setTimeout(resolve, 250));
    text = await state.textContent();
  }
  if (text.includes('VIDEO_NETWORK')) offline();
  if (harmony) {
    expect(text).toMatch(/· [1-9]\d*p ·/);
    await expectPicture(t, view.testId('video-stream'));
    t.skip('HarmonyOS does not list HLS renditions yet');
  }
  expect(text).toContain('1080p/720p/540p/360p');
  await view.testId('video-stream-360p').click();
  await expect(state).toContainText('360p · 360p', { timeout: 30_000 });
});
