import { expect, test } from '@lingxia/test';
import { SHOWCASE_APP_ID } from '../helpers/app.js';
import { bindFixture, type Caught } from '../helpers/poll.js';

/**
 * Local media processing — info, thumbnail, and compression — needs real bytes,
 * not the deterministic filler `/file/*` serves: a native decoder rejects that.
 * The fixture's `/media/sample.{mp4,png}` are a valid clip and image. Without
 * `--arg httpBase=…` these register as pending rather than passing silently.
 */
const httpBase = (globalThis.__LINGXIA_AUTOMATION_HOST__?.args ?? {}).httpBase;
test.configure({ skip: httpBase ? undefined : 'needs --arg httpBase' });
const pending = { reason: 'needs the HTTP fixture: node tests/harness/http-fixture.mjs, then --arg httpBase=<url>' };

interface MediaResult {
  lastProgress: number;
  progressTicks: number;
  viaThen: number;
  finallyRan: boolean;
  caught: string | null;
  returnedDone: boolean;
  earlyCaught: string | null;
  video: { width: number; height: number; durationMs: number; size: number; type: string };
  thumb: { path: string; width: number; height: number };
  compressedImage: { path: string };
  compressedVideo: { path: string; size: number };
}

test('read info, thumbnail, and compress local media', {
  id: 'MEDIA-PROCESS-001',
  covers: [
    'lx.media.videoInfo',
    'lx.media.videoThumbnail',
    'lx.media.compressImage',
    'lx.media.compressVideo',
    'CompressVideoTask.result',
    'lx.network.download',
  ],
  app: SHOWCASE_APP_ID,
  timeout: 60_000,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'MEDIA-PROCESS-001');

  const result: MediaResult = await app.logic.eval({ timeout: 45_000 }, async ({ lx }, base) => {
    const vid = await lx.network.download(base + '/media/sample.mp4').result;
    const png = await lx.network.download(base + '/media/sample.png').result;
    const video = await lx.media.videoInfo(vid.uri);
    const thumb = await lx.media.videoThumbnail(vid.uri, { positionMs: 500 });
    const compressedImage = await lx.media.compressImage(png.uri, { quality: 60 });
    const compressedVideo = await lx.media.compressVideo(vid.uri, { quality: 'low' }).result;
    // Drain the task to prove progress flows before the result settles.
    let lastProgress = -1;
    let progressTicks = 0;
    const progressTask = lx.media.compressVideo(vid.uri, { quality: 'low' });
    for await (const tick of progressTask) {
      if (typeof tick.fraction === 'number') { lastProgress = tick.fraction; progressTicks += 1; }
    }
    await progressTask.result;
    // The result Promise supports then/finally on success and catch on a bad
    // path; leaving the iteration early cancels the transcode.
    let finallyRan: boolean = false;
    const viaThen = await lx.media.compressVideo(vid.uri, { quality: 'low' }).result
      .then((r) => r.size)
      .finally(() => { finallyRan = true; });
    // A missing source throws synchronously; an aborted task rejects, which
    // is the path .catch() is for.
    const cancelled = lx.media.compressVideo(vid.uri, { quality: 'high' });
    cancelled.cancel();
    const caught = await cancelled.result.then(() => null).catch((error: { code?: string } | null) => error?.code ?? null);
    const earlyTask = lx.media.compressVideo(vid.uri, { quality: 'low' });
    const iterator = earlyTask[Symbol.asyncIterator]();
    const returned = await iterator.return!();
    const earlyCaught = await earlyTask.result.then(() => null, (error: { code?: string } | null) => error?.code ?? null);
    return {
      lastProgress,
      progressTicks,
      viaThen,
      finallyRan: finallyRan as boolean,
      caught,
      returnedDone: returned.done === true,
      earlyCaught,
      video: { width: video.width, height: video.height, durationMs: video.durationMs, size: video.size, type: video.type ?? "" },
      thumb: { path: thumb.path, width: thumb.width, height: thumb.height },
      compressedImage: { path: compressedImage.path },
      compressedVideo: { path: compressedVideo.path, size: compressedVideo.size },
    };
  }, httpBase);

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
  expect(result.lastProgress).toBeLessThanOrEqual(1);
  // Thenable and iterator protocol.
  expect(result.viaThen).toBeGreaterThan(0);
  expect(result.finallyRan).toBe(true);
  expect(typeof result.caught).toBe('string');
  expect(result.returnedDone).toBe(true);
  expect(result.earlyCaught).toBe('E_ABORT');

  // A thumbnail is a real image the lxapp can read back.
  expect(result.thumb.path.startsWith('lx://')).toBeTruthy();
  expect(result.thumb.width).toBeGreaterThan(0);
  expect(result.thumb.height).toBeGreaterThan(0);

  expect(result.compressedImage.path.startsWith('lx://')).toBeTruthy();
  expect(result.compressedVideo.path.startsWith('lx://')).toBeTruthy();
  // Re-encoding at low quality yields a real, non-empty output file.
  expect(result.compressedVideo.size).toBeGreaterThan(0);

  // A compressed output the lxapp cannot read back is not an output.
  const sizes = await app.logic.eval(async ({ lx }, image, video, thumb) => ({
    image: (await lx.fs.stat(image)).size,
    video: (await lx.fs.stat(video)).size,
    thumb: (await lx.fs.stat(thumb)).size,
  }), result.compressedImage.path, result.compressedVideo.path, result.thumb.path);
  expect(sizes.image).toBeGreaterThan(0);
  expect(sizes.video).toBeGreaterThan(0);
  expect(sizes.thumb).toBeGreaterThan(0);
});

test('cancel an in-flight compressVideo and reject with E_ABORT', {
  id: 'MEDIA-PROCESS-CANCEL-001',
  covers: ['lx.media.compressVideo', 'CompressVideoTask.cancel'],
  app: SHOWCASE_APP_ID,
  timeout: 40_000,
  ...pending,
}, async (t) => {
  const { app } = bindFixture(t, 'MEDIA-PROCESS-CANCEL-001');

  const outcome: Caught = await app.logic.eval({ timeout: 30_000 }, async ({ lx }, base) => {
    try {
      const vid = await lx.network.download(base + '/media/sample.mp4').result;
      const task = lx.media.compressVideo(vid.uri, { quality: 'high' });
      task.cancel();
      return { ok: true, value: await task.result };
    } catch (error) {
      const { code, message, data } = error as { code?: string; message?: string; data?: unknown };
      return { ok: false, code: code ?? '', message: String(message ?? error), data: data ?? null };
    }
  }, httpBase);

  expect(outcome.ok).toBeFalsy();
  expect(outcome.code).toBe('E_ABORT');
});
