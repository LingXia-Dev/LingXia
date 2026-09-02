import { expect, spec } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture, evalCaught } from '../helpers/poll.js';

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
  thumb: { tempFilePath: string; width: number; height: number };
  compressedImage: { tempFilePath: string };
  compressedVideo: { tempFilePath: string; size: number };
}

mediaSpec('read info, thumbnail, and compress local media', {
  id: 'MEDIA-PROCESS-001',
  covers: [
    'lx.getVideoInfo',
    'lx.extractVideoThumbnail',
    'lx.compressImage',
    'lx.compressVideo',
    'CompressVideoTask.wait',
    'CompressVideoTask.next',
    'CompressVideoTask.return',
    'CompressVideoTask.then',
    'CompressVideoTask.catch',
    'CompressVideoTask.finally',
    'lx.downloadFile',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'MEDIA-PROCESS-001');

  const result = await app.eval({
    timeoutMs: 45_000,
    script: `
      const vid = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/media/sample.mp4`)} });
      const png = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/media/sample.png`)} });
      const video = await lx.getVideoInfo({ path: vid.tempFilePath });
      const thumb = await lx.extractVideoThumbnail({ path: vid.tempFilePath, timeMs: 500 });
      const compressedImage = await lx.compressImage({ path: png.tempFilePath, quality: 60 });
      const compressedVideo = await lx.compressVideo({ path: vid.tempFilePath, quality: 'low' }).wait();
      // The task is also an async iterator of progress; drain it to prove next().
      let lastProgress = -1;
      let progressTicks = 0;
      for await (const tick of lx.compressVideo({ path: vid.tempFilePath, quality: 'low' })) {
        if (typeof tick.progress === 'number') { lastProgress = tick.progress; progressTicks += 1; }
      }
      // The task is a thenable too: then/finally on success, catch on a bad
      // path, and an early iterator return leaves nothing dangling.
      let finallyRan = false;
      const viaThen = await lx.compressVideo({ path: vid.tempFilePath, quality: 'low' })
        .then((r) => r.size)
        .finally(() => { finallyRan = true; });
      // A missing source throws synchronously; an aborted task rejects, which
      // is the path .catch() is for.
      const cancelled = lx.compressVideo({ path: vid.tempFilePath, quality: 'high' });
      cancelled.cancel();
      const caught = await cancelled.then(() => null).catch((error) => error && error.code);
      const iterator = lx.compressVideo({ path: vid.tempFilePath, quality: 'low' })[Symbol.asyncIterator]();
      const returned = await iterator.return();
      return {
        lastProgress,
        progressTicks,
        viaThen,
        finallyRan,
        caught,
        returnedDone: returned.done === true,
        video: { width: video.width, height: video.height, durationMs: video.durationMs, size: video.size, type: video.type },
        thumb: { tempFilePath: thumb.tempFilePath, width: thumb.width, height: thumb.height },
        compressedImage: { tempFilePath: compressedImage.tempFilePath },
        compressedVideo: { tempFilePath: compressedVideo.tempFilePath, size: compressedVideo.size },
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
  expect(result.thumb.tempFilePath.startsWith('lx://')).toBeTruthy();
  expect(result.thumb.width).toBeGreaterThan(0);
  expect(result.thumb.height).toBeGreaterThan(0);

  expect(result.compressedImage.tempFilePath.startsWith('lx://')).toBeTruthy();
  expect(result.compressedVideo.tempFilePath.startsWith('lx://')).toBeTruthy();
  // Re-encoding at low quality yields a real, non-empty output file.
  expect(result.compressedVideo.size).toBeGreaterThan(0);

  // A compressed output the lxapp cannot read back is not an output.
  const sizes = await app.eval({
    script: `
      const image = await lx.fs.stat(${JSON.stringify(result.compressedImage.tempFilePath)});
      const video = await lx.fs.stat(${JSON.stringify(result.compressedVideo.tempFilePath)});
      const thumb = await lx.fs.stat(${JSON.stringify(result.thumb.tempFilePath)});
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

  const outcome = await evalCaught(app, `
    const vid = await lx.downloadFile({ url: ${JSON.stringify(`${httpBase}/media/sample.mp4`)} });
    const task = lx.compressVideo({ path: vid.tempFilePath, quality: 'high' });
    task.cancel();
    return await task;
  `);

  expect(outcome.ok).toBeFalsy();
  expect(outcome.code).toBe('E_ABORT');
});
