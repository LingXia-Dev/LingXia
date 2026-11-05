// Frame pacing the renderer measures for this page. The host takes the requests after a
// View step, waits out each window on its own clock and answers with the window's stats.
let pending = [], waiting = new Map(), nextToken = 1;

const LONGEST_MS = 60000;

/** Measure this page's frames for `ms` from now; resolves with the window's frame stats. */
export function measureFrames(ms) {
  if (typeof ms !== 'number' || !Number.isFinite(ms) || ms <= 0 || ms > LONGEST_MS) {
    return Promise.reject(new RangeError(`UI_PROP: measureFrames takes 1–${LONGEST_MS} ms`));
  }
  const token = nextToken++;
  pending.push([token, Math.round(ms)]);
  return new Promise(resolve => waiting.set(token, resolve));
}

export function takeFrameMeasures() { const result = pending; pending = []; return JSON.stringify(result); }

/** The window of `token` ended; true when a measurement was waiting for it. */
export function frameMeasured(token, stats) {
  const resolve = waiting.get(token);
  if (!resolve) return false;
  waiting.delete(token);
  resolve(stats);
  return true;
}

/** The page went away; nothing answers its measurements any more. */
export function resetFrameMeasures() { pending = []; waiting = new Map(); }
