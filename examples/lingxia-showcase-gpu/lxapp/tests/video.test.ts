import { expect, test } from '@lingxia/test';
import { decodePng } from './png.js';
import { skipWhenCovered } from './covered.js';

test('a video plays in a 16:9 box and under its overlay', { start: { page: 'video' } }, async (t) => {
  await skipWhenCovered(t, 'video');
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

type Fixture = Parameters<Parameters<typeof test>[2]>[0];
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

test('useVideo reports playback and pauses on command', { start: { page: 'video' } }, async (t) => {
  await skipWhenCovered(t, 'video');
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

test('a quality switch keeps the position and the frames moving', { start: { page: 'video' } }, async (t) => {
  await skipWhenCovered(t, 'video');
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

test('lx.createVideoContext pauses and plays the page player from Logic', { start: { page: 'video' } }, async (t) => {
  await skipWhenCovered(t, 'video');
  const view = t.app.view;
  const state = view.testId('video-hero-state');
  await expect(state).toContainText('playing');
  await view.testId('video-hero-logic-pause').click();
  await expect(state).toContainText('paused');
  await view.testId('video-hero-logic-play').click();
  await expect(state).toContainText('playing');
});

test('a page under another holds its videos still and plays the playing ones again on return', { start: { page: 'video' } }, async (t) => {
  await skipWhenCovered(t, 'video');
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

// Needs the network: one public row plays Big Buck Bunny as three progressive files, the other
// Apple's HLS test stream with five renditions; an unreachable host skips.
test('public renditions switch quality over the network', { start: { page: 'video' }, tags: ['network'], timeout: 90_000 }, async (t) => {
  await skipWhenCovered(t, 'video');
  const view = t.app.view;
  const offline = () => t.skip('needs the network: the public video hosts are unreachable');
  const settle = async (id: string, done: (text: string) => boolean) => {
    const state = view.testId(id);
    const deadline = Date.now() + 20_000;
    let text = await state.textContent();
    while (!done(text) && !text.includes('VIDEO_NETWORK') && Date.now() < deadline) {
      await new Promise<void>(resolve => setTimeout(resolve, 250));
      text = await state.textContent();
    }
    if (text.includes('VIDEO_NETWORK')) offline();
    return text;
  };
  // Progressive: each label is its own file, so a switch changes the decoded height.
  const files = await settle('video-public-state', text => /· [1-9]\d*p ·/.test(text));
  expect(files).toContain('360p/720p/1080p');
  for (const quality of ['1080p', '360p']) {
    await view.testId(`video-public-${quality}`).click();
    await expect(view.testId('video-public-state')).toContainText(`${quality} · ${quality}`, { timeout: 30_000 });
  }
  // HLS: the platform lists the stream's renditions and pins the one chosen.
  const renditions = await settle('video-stream-state', text => text.includes('1080p/720p/540p/360p/234p'));
  expect(renditions).toContain('1080p/720p/540p/360p/234p');
  // A pin keeps the position: the decoded height follows and the clock does not go back.
  const seconds = async () => Number((await view.testId('video-stream-state').textContent()).match(/([\d.]+)s/)?.[1]);
  for (const quality of ['540p', '360p', '1080p']) {
    const before = await seconds();
    await view.testId(`video-stream-${quality}`).click();
    await expect(view.testId('video-stream-state')).toContainText(`${quality} · ${quality}`, { timeout: 30_000 });
    expect(await seconds()).toBeGreaterThanOrEqual(before - 0.5);
  }
  await view.testId('video-stream-auto').click();
  await expect(view.testId('video-stream-state')).toContainText('auto · ', { timeout: 30_000 });
  await expect(view.testId('video-stream-state')).toContainText('1080p/720p/540p/360p/234p');
});

// A pin while the stream still prepares (its row revealed by the tap) waits for it, then plays the
// rendition from the start.
test('an HLS quality pinned before the first frame plays that rendition', { start: { page: 'video' }, tags: ['network'], timeout: 90_000 }, async (t) => {
  await skipWhenCovered(t, 'video');
  const view = t.app.view;
  const state = view.testId('video-stream-state');
  const deadline = Date.now() + 20_000;
  while (!(await state.textContent()).includes('234p') && Date.now() < deadline) {
    if ((await state.textContent()).includes('VIDEO_NETWORK')) t.skip('needs the network: the HLS host is unreachable');
    await new Promise<void>(resolve => setTimeout(resolve, 250));
  }
  await expect(state).not.toContainText('· 1080p ·');
  await view.testId('video-stream-360p').click();
  await expect(state).toContainText('360p · 360p', { timeout: 30_000 });
  const seconds = async () => Number((await state.textContent()).match(/([\d.]+)s/)?.[1]);
  const start = await seconds();
  await expect.poll(seconds, { timeout: 10_000 }).toBeGreaterThan(start + 1);
});

// HarmonyOS's player opens this stream slowly (about 12 s on a nova 13): from the tap until a frame
// or the 15 s error, the box shows the ring, never a blank picture.
test('an HLS start shows the ring from the tap until a frame or the error with Retry', { start: { page: 'spec-video' }, tags: ['network'], timeout: 90_000 }, async (t) => {
  await skipWhenCovered(t, 'spec-video');
  const view = t.app.view;
  const c = (part: string) => view.testId(`lab-stream-controls-${part}`);
  // The stream sits below the clip: scroll until its large play is on screen (aria-hidden, so the
  // tap is by point, and no locator scrolls to it).
  const area = await view.viewport();
  let box = await c('start').boundingBox();
  for (let i = 0; i < 6 && box.y + box.height > area.height - 48; i++) {
    const mid: [number, number] = [area.width / 2, area.height * 0.7];
    await view.swipe({ from: mid, to: [mid[0], mid[1] - 300], durationMs: 300 });
    box = await c('start').boundingBox();
  }
  const at: [number, number] = [box.x + box.width / 2, box.y + box.height / 2];
  await view.swipe({ from: at, to: at, durationMs: 60, hold: 0 });
  // The frame that takes the large play away answers the tap: the ring, unless a frame or the error came first.
  await expect.poll(() => c('start').count(), { timeout: 5_000 }).toBe(0);
  const answer = async () => ((await c('error').count()) ? 'error' : (await c('loading').count()) ? 'ring'
    : (await c('play').getAttribute('aria-label')) === 'Pause' ? 'playing' : 'blank');
  expect(await answer(), 'the tap shows the ring at once').not.toBe('blank');
  const deadline = Date.now() + 20_000;
  let outcome = '';
  while (!outcome && Date.now() < deadline) {
    const now = await answer();
    if (now === 'error' || now === 'playing') outcome = now;
    else expect(now, `the ring while nothing shows (start ${await c('start').count()}, play "${await c('play').getAttribute('aria-label')}", `
      + `busy ${await c('play').getAttribute('aria-busy')}, time "${await c('time').textContent()}", ${Date.now() - deadline + 20_000} ms after the tap)`).toBe('ring');
    await new Promise<void>(resolve => setTimeout(resolve, 250));
  }
  expect(outcome, 'a frame or the error within the start deadline').not.toBe('');
  if (outcome === 'error') await expect(c('retry')).toBeVisible();
});
