// The perf lab's pass bar, shared by its "Run budget" button and the perf-lab specs, so a scene
// has one verdict whoever drives it. Inputs are the renderer's own frame statistics.

/** Frames keep pace: p95 frame time within this many refresh periods. */
export const FRAME_P95_REFRESHES = 2.5;
/** Frames keep pace: at most this share of refreshes dropped. */
export const DROPPED_SHARE = 0.1;
/** Input shows within this many refresh periods (p95). */
export const INPUT_P95_REFRESHES = 5;
/** A returning or pushed page shows its first frame within this (p95), ms. */
export const REVEAL_P95_MS = 250;
/** Logic state set from an awaited loop: at least this share of its ticks reach a frame. */
export const TICK_SHARE = 0.35;

interface Percentiles { p50: number; p95: number; p99: number }
/** The fields of `FrameStats` (View) and `TestMetrics` (specs) the bar reads. */
export interface JudgedStats {
  refreshMs: number | null;
  samples: number;
  dropped: number;
  frameTimeMs: Percentiles | null;
  inputToPresent: { count: number; statsMs: Percentiles | null };
  revealMs: Percentiles | null;
  reveals: number;
}
export interface Verdict { line: string; pass: boolean }

const ms = (value: number | null | undefined) => (value == null ? '-' : value.toFixed(1));
const refreshOf = (stats: JudgedStats) => stats.refreshMs ?? 16.7;

export function judgeFrames(stats: JudgedStats): Verdict {
  const times = stats.frameTimeMs;
  const shown = stats.samples + stats.dropped;
  const pass = times !== null && times.p95 <= refreshOf(stats) * FRAME_P95_REFRESHES && stats.dropped <= shown * DROPPED_SHARE;
  return { pass, line: `frame p50 ${ms(times?.p50)} · p95 ${ms(times?.p95)} · p99 ${ms(times?.p99)} ms` };
}

export function judgeInput(stats: JudgedStats): Verdict {
  const input = stats.inputToPresent.statsMs;
  const pass = input !== null && input.p95 <= refreshOf(stats) * INPUT_P95_REFRESHES;
  return { pass, line: `input→present p50 ${ms(input?.p50)} · p95 ${ms(input?.p95)} · p99 ${ms(input?.p99)} ms (${stats.inputToPresent.count})` };
}

export function judgeReveal(stats: JudgedStats): Verdict {
  const reveal = stats.revealMs;
  const pass = reveal !== null && reveal.p95 <= REVEAL_P95_MS;
  return { pass, line: `first present p50 ${ms(reveal?.p50)} · p95 ${ms(reveal?.p95)} · p99 ${ms(reveal?.p99)} ms (${stats.reveals} reveals)` };
}
