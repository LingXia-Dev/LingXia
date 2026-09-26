import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID, rawApp } from '../helpers/app.js';
import { bindFixture, evalCaught } from '../helpers/poll.js';

// String scripts and raw page reads go to the raw driver; see `rawApp`.
const raw = rawApp();

/**
 * Local media processing — info, thumbnail, and compression — needs real bytes,
 * not the deterministic filler `/file/*` serves: a native decoder rejects that.
 * The fixture's `/media/sample.{mp4,png}` are a valid clip and image. Without
 * `--arg httpBase=…` these register as pending rather than passing silently.
 */
const httpBase = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {}).httpBase;
const mediaSpec = httpBase ? spec : spec.skip;
const pending = { reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>' };

interface MediaResult {
  lastProgress: number;
  progressTicks: number;
  viaThen: number;
  finallyRan: boolean;
  caught: string | null;
  returnedDone: boolean;
  video: { width: number; height: number; durationMs: number; size: number; type: string };
  thumb: { uri: string; width: number; height: number };
  compressedImage: { uri: string };
  compressedVideo: { uri: string; size: number };
}

mediaSpec('read info, thumbnail, and compress local media', {
  id: 'MEDIA-PROCESS-001',
  covers: [
    'lx.getVideoInfo',
    'lx.extractVideoThumbnail',
    'lx.compressImage',
    'lx.compressVideo',
    'CompressVideoTask.result',
    'CompressVideoTask.progress',
    'lx.downloadFile',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'MEDIA-PROCESS-001');

  const result = await raw.eval({
    timeoutMs: 45_000,
    script: `
      const vid = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/media/sample.mp4`)} }).result;
      const png = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/media/sample.png`)} }).result;
      const video = await lx.getVideoInfo({ path: vid.uri });
      const thumb = await lx.extractVideoThumbnail({ path: vid.uri, timeMs: 500 });
      const compressedImage = await lx.compressImage({ path: png.uri, quality: 60 });
      const compressedVideo = await lx.compressVideo({ path: vid.uri, quality: 'low' }).result;
      // Drain task.progress to prove progress events flow before the result settles.
      let lastProgress = -1;
      let progressTicks = 0;
      const progressTask = lx.compressVideo({ path: vid.uri, quality: 'low' });
      for await (const tick of progressTask.progress) {
        if (typeof tick.progress === 'number') { lastProgress = tick.progress; progressTicks += 1; }
      }
      await progressTask.result;
      // The result Promise supports: then/finally on success, catch on a bad
      // path, and an early iterator return leaves nothing dangling.
      let finallyRan = false;
      const viaThen = await lx.compressVideo({ path: vid.uri, quality: 'low' }).result
        .then((r) => r.size)
        .finally(() => { finallyRan = true; });
      // A missing source throws synchronously; an aborted task rejects, which
      // is the path .catch() is for.
      const cancelled = lx.compressVideo({ path: vid.uri, quality: 'high' });
      cancelled.cancel();
      const caught = await cancelled.result.then(() => null).catch((error) => error && error.code);
      const earlyTask = lx.compressVideo({ path: vid.uri, quality: 'low' });
      const iterator = earlyTask.progress[Symbol.asyncIterator]();
      const returned = await iterator.return();
      await earlyTask.result;
      return {
        lastProgress,
        progressTicks,
        viaThen,
        finallyRan,
        caught,
        returnedDone: returned.done === true,
        video: { width: video.width, height: video.height, durationMs: video.durationMs, size: video.size, type: video.type },
        thumb: { uri: thumb.uri, width: thumb.width, height: thumb.height },
        compressedImage: { uri: compressedImage.uri },
        compressedVideo: { uri: compressedVideo.uri, size: compressedVideo.size },
      };
    `,
  }) as MediaResult;

  // The clip is a known 160×90, 4s sample — its metadata is exact, not a guess.
  expect(result.video.width).toBe(160);
  expect(result.video.height).toBe(90);
  expect(result.video.durationMs).toBeGreaterThanOrEqual(3_500);
  expect(result.video.durationMs).toBeLessThanOrEqual(4_500);
  expect(result.video.type).toContain('mp4');

  // The progress iterator ran and reported real progress. The last tick is not
  // required to be exactly 100: a platform encoder may report 98 and then end
  // the stream, and completion is what `wait()` above already proved.
  expect(result.progressTicks).toBeGreaterThanOrEqual(1);
  expect(result.lastProgress).toBeGreaterThan(0);
  expect(result.lastProgress).toBeLessThanOrEqual(100);
  // Thenable and iterator protocol.
  expect(result.viaThen).toBeGreaterThan(0);
  expect(result.finallyRan).toBe(true);
  expect(typeof result.caught).toBe('string');
  expect(result.returnedDone).toBe(true);

  // A thumbnail is a real image the lxapp can read back.
  expect(result.thumb.uri.startsWith('lx://')).toBeTruthy();
  expect(result.thumb.width).toBeGreaterThan(0);
  expect(result.thumb.height).toBeGreaterThan(0);

  expect(result.compressedImage.uri.startsWith('lx://')).toBeTruthy();
  expect(result.compressedVideo.uri.startsWith('lx://')).toBeTruthy();
  // Re-encoding at low quality yields a real, non-empty output file.
  expect(result.compressedVideo.size).toBeGreaterThan(0);

  // A compressed output the lxapp cannot read back is not an output.
  const sizes = await raw.eval({
    script: `
      const image = await lx.fs.stat(${JSON.stringify(result.compressedImage.uri)});
      const video = await lx.fs.stat(${JSON.stringify(result.compressedVideo.uri)});
      const thumb = await lx.fs.stat(${JSON.stringify(result.thumb.uri)});
      return { image: image.size, video: video.size, thumb: thumb.size };
    `,
  }) as { image: number; video: number; thumb: number };
  expect(sizes.image).toBeGreaterThan(0);
  expect(sizes.video).toBeGreaterThan(0);
  expect(sizes.thumb).toBeGreaterThan(0);
});

mediaSpec('cancel an in-flight compressVideo and reject with E_ABORT', {
  id: 'MEDIA-PROCESS-CANCEL-001',
  covers: ['lx.compressVideo', 'CompressVideoTask.cancel'],
  app: SHOWCASE_APP_ID,
  timeout: 40_000,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'MEDIA-PROCESS-CANCEL-001');

  const outcome = await evalCaught(raw, `
    const vid = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/media/sample.mp4`)} }).result;
    const task = lx.compressVideo({ path: vid.uri, quality: 'high' });
    task.cancel();
    return await task.result;
  `);

  expect(outcome.ok).toBeFalsy();
  expect(outcome.code).toBe('E_ABORT');
});
