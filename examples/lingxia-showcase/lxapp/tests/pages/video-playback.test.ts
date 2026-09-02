import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { currentPageOrNull, waitForCurrentPage, waitForCurrentPageVisible, waitForElementText } from '../helpers/page.js';
import { bindFixture, eventually } from '../helpers/poll.js';

const httpBase = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {}).httpBase;
const playbackSpec = httpBase ? spec : spec.skip;
const VIDEO_ID = 'lx-video-source-fixture';

interface PlaybackState {
  eventLog: string;
  currentTime: number;
  duration: number;
}

playbackSpec('drive native playback through VideoContext against a local clip', {
  id: 'NATIVE-VIDEO-PLAYBACK-001',
  covers: ['lx.createVideoContext', 'VideoContext.play', 'VideoContext.seek', 'VideoContext.stop'],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>',
}, async (t) => {
  const { app, defer } = bindFixture(t, 'NATIVE-VIDEO-PLAYBACK-001');
  const readState = () => app.eval({
    script: `
      const page = getCurrentPages().find((candidate) => candidate.route.includes('/video/'));
      return {
        eventLog: String(page?.data?.eventLog ?? ''),
        currentTime: Number(page?.data?.currentTime ?? 0),
        duration: Number(page?.data?.duration ?? 0),
      };
    `,
  }) as Promise<PlaybackState>;
  const command = (body: string) => app.eval({
    script: `lx.createVideoContext(${JSON.stringify(VIDEO_ID)}).${body}; return true;`,
  });

  const current = await currentPageOrNull(app);
  if (current?.name !== 'home') await app.nav.relaunch({ page: 'home' });
  await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  defer(async () => {
    await command('stop()').catch(() => undefined);
    const active = await currentPageOrNull(app);
    if (active?.name !== 'home') await app.nav.relaunch({ page: 'home' });
    await waitForCurrentPageVisible(app, 'home', '[data-testid="home-page"]');
  });

  await app.nav.to({
    page: 'video',
    query: { automationFixture: 'video-source', src: `${httpBase}/media/sample.mp4` },
  });
  await waitForCurrentPage(app, 'video');
  await app.page.waitFor({ page: 'video', css: '[data-testid="video-page"]', state: 'visible' });
  await app.page.waitFor({ page: 'video', css: `#${VIDEO_ID}`, state: 'visible' });
  // Autoplay is off for this fixture, so the clock has not started. The event
  // label is not the invariant: a native player may already report buffering
  // as it opens the source.
  expect((await readState()).currentTime).toBe(0);

  await t.step('play() starts the clock and reports the clip length', async () => {
    await command('play()');
    await waitForElementText(app, 'video', '[data-testid="video-event"]', (text) => text.includes('Playing'), 15_000);
    const playing = await eventually(readState, (state) => state.currentTime > 0 && state.duration > 0, {
      describe: 'timeupdate to advance after play()',
      timeoutMs: 15_000,
    });
    expect(playing.duration).toBeGreaterThan(3);
    expect(playing.duration).toBeLessThan(5);
  });

  await t.step('seek() moves the position', async () => {
    await command('seek(3)');
    const sought = await eventually(readState, (state) => state.currentTime >= 2.5, {
      describe: 'currentTime to reach the seek target',
      timeoutMs: 10_000,
    });
    expect(sought.currentTime).toBeGreaterThanOrEqual(2.5);
  });

  await t.step('stop() ends playback', async () => {
    await command('stop()');
    await waitForElementText(
      app,
      'video',
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
