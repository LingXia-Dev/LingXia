import { expect, spec, type Fixture } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { currentPageOrNull, waitForCurrentPage, waitForCurrentPageVisible, waitForElementText } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

const httpBase = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {}).httpBase;
spec.configure({ skip: httpBase ? undefined : 'needs --arg httpBase' });
const VIDEO_ID = 'lx-video-source-fixture';

interface PlaybackState {
  eventLog: string;
  currentTime: number;
  duration: number;
}

spec('drive native playback through VideoContext against a local clip', {
  id: 'NATIVE-VIDEO-PLAYBACK-001',
  covers: ['lx.createVideoContext', 'VideoContext.play', 'VideoContext.seek', 'VideoContext.stop'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
}, async (t) => {
  const { app, defer } = bindFixture(t, 'NATIVE-VIDEO-PLAYBACK-001');
  const readState = () => app.logic.eval(({ getCurrentPages }): PlaybackState => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/video/'));
    const data = (page?.data ?? {}) as { eventLog?: unknown; currentTime?: unknown; duration?: unknown };
    return {
      eventLog: String(data.eventLog ?? ''),
      currentTime: Number(data.currentTime ?? 0),
      duration: Number(data.duration ?? 0),
    };
  });
  const command = (action: 'play' | 'seek' | 'stop', position = 0) => app.logic.eval(({ lx }, id, action, position) => {
    const context = lx.createVideoContext(id);
    if (action === 'seek') context.seek(position);
    else if (action === 'play') context.play();
    else context.stop();
    return true;
  }, VIDEO_ID, action, position);

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    await command('stop').catch(() => undefined);
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  await app.nav.to({
    page: 'video',
    query: { automationFixture: 'video-source', src: `${httpBase}/media/sample.mp4` },
  });
  await waitForCurrentPage(app, 'video');
  const video = (await app.page({ name: 'video' }, { timeout: 30_000 })).view;
  await video.testId('video-page').waitFor({ state: 'visible', timeout: 30_000 });
  await video.css(`#${VIDEO_ID}`).first().waitFor({ state: 'visible', timeout: 30_000 });
  // Autoplay is off for this fixture, so the clock has not started. The event
  // label is not the invariant: a native player may already report buffering
  // as it opens the source.
  expect((await readState()).currentTime).toBe(0);

  await t.step('play() starts the clock and reports the clip length', async () => {
    await command('play');
    await waitForElementText(t, video, '[data-testid="video-event"]', (text) => text.includes('Playing'), 15_000);
    const playing = await eventually(readState, (state) => state.currentTime > 0 && state.duration > 0, {
      describe: 'timeupdate to advance after play()',
      timeoutMs: 15_000,
    });
    expect(playing.duration).toBeGreaterThan(3);
    expect(playing.duration).toBeLessThan(5);
  });

  await t.step('seek() moves the position', async () => {
    await command('seek', 3);
    const sought = await eventually(readState, (state) => state.currentTime >= 2.5, {
      describe: 'currentTime to reach the seek target',
      timeoutMs: 10_000,
    });
    expect(sought.currentTime).toBeGreaterThanOrEqual(2.5);
  });

  await t.step('stop() ends playback', async () => {
    await command('stop');
    await waitForElementText(
      t,
      video,
      '[data-testid="video-event"]',
      (text) => text.includes('Stopped') || text.includes('Paused') || text.includes('Ended'),
      10_000,
    );
    const stopped = await readState();
    const settled = await new Promise<PlaybackState>((resolve) => setTimeout(() => resolve(readState()), 700));
    // Whatever label the platform emits, the clock no longer runs.
    expect(settled.currentTime).toBeLessThanOrEqual(stopped.currentTime + 0.05);
  });
});

async function openStoppedClip(t: Fixture, id: string) {
  const { app, defer } = bindFixture(t, id);
  const readState = () => app.logic.eval(({ getCurrentPages }): PlaybackState => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/video/'));
    const data = (page?.data ?? {}) as { eventLog?: unknown; currentTime?: unknown; duration?: unknown };
    return {
      eventLog: String(data.eventLog ?? ''),
      currentTime: Number(data.currentTime ?? 0),
      duration: Number(data.duration ?? 0),
    };
  });
  const play = () => app.logic.eval(({ lx }, videoId) => {
    lx.createVideoContext(videoId).play();
    return true;
  }, VIDEO_ID);

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    await app.logic.eval(({ lx }, videoId) => {
      lx.createVideoContext(videoId).stop();
      return true;
    }, VIDEO_ID).catch(() => undefined);
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  await app.nav.to({
    page: 'video',
    query: { automationFixture: 'video-source', src: `${httpBase}/media/sample.mp4` },
  });
  await waitForCurrentPage(app, 'video');
  const video = (await app.page({ name: 'video' }, { timeout: 30_000 })).view;
  await video.css(`#${VIDEO_ID}`).first().waitFor({ state: 'visible', timeout: 30_000 });
  // Let mount-time prop updates finish, so the play() below is the only cause.
  await new Promise<void>((resolve) => setTimeout(resolve, 1500));
  expect((await readState()).currentTime).toBe(0);
  return { app, readState, play };
}

spec('start a stopped native video with one play()', {
  id: 'NATIVE-VIDEO-PLAYBACK-002',
  covers: ['VideoContext.play'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
}, async (t) => {
  const { readState, play } = await openStoppedClip(t, 'NATIVE-VIDEO-PLAYBACK-002');
  await play();
  const playing = await eventually(readState, (state) => state.currentTime > 0.25, {
    describe: 'the clock to run after a single play()',
    timeoutMs: 10_000,
  });
  expect(playing.duration).toBeGreaterThan(3);
});

spec('keep a native video playing through a props update', {
  id: 'NATIVE-VIDEO-PLAYBACK-003',
  covers: ['VideoContext.play'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
}, async (t) => {
  const { app, readState, play } = await openStoppedClip(t, 'NATIVE-VIDEO-PLAYBACK-003');
  await play();
  // The first play() is allowed to be lost here; that fault has its own spec.
  await eventually(async () => {
    const state = await readState();
    if (state.currentTime === 0) await play();
    return state;
  }, (state) => state.currentTime > 0.25, { describe: 'the clip to start', timeoutMs: 10_000, intervalMs: 500 });

  // Same source, one other prop changed: the host receives the source again.
  const before = await app.logic.eval(({ getCurrentPages }) => {
    const page = getCurrentPages().find((candidate) => candidate.route.includes('/video/'));
    const data = (page?.data ?? {}) as { videos?: Record<string, unknown>[]; currentTime?: unknown };
    const [first] = data.videos ?? [];
    page?.setData({ videos: [{ ...first, playbackRates: [1.0, 1.5] }] });
    return Number(data.currentTime ?? 0);
  });

  const samples: number[] = [];
  for (let i = 0; i < 8; i += 1) {
    await new Promise<void>((resolve) => setTimeout(resolve, 150));
    samples.push((await readState()).currentTime);
  }
  // A reload would put the clock back at zero and, without autoplay, leave it there.
  expect(Math.min(...samples)).toBeGreaterThanOrEqual(before);
  expect(samples[samples.length - 1]).toBeGreaterThan(before + 0.4);
});
