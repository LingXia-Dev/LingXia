import type { JinshuProgress, Observation } from './types';

const MAX_TRACKED_SESSIONS = 32;

/** Merges overlapping or adjacent `[startMs, endMs]` playback intervals. */
export function mergeIntervals(intervals: readonly (readonly [number, number])[]): [number, number][] {
  if (intervals.length === 0) return [];
  const sorted = intervals
    .filter(([a, b]) => Number.isFinite(a) && Number.isFinite(b) && b > a)
    .map(([a, b]): [number, number] => [a, b])
    .sort((x, y) => x[0] - y[0]);
  if (sorted.length === 0) return [];

  const merged: [number, number][] = [sorted[0]];
  for (let i = 1; i < sorted.length; i++) {
    const [start, end] = sorted[i];
    const last = merged[merged.length - 1];
    if (start <= last[1]) {
      last[1] = Math.max(last[1], end);
    } else {
      merged.push([start, end]);
    }
  }
  return merged;
}

/**
 * Pure arithmetic reducer for objective observations.
 * Deduplicates by `seenSeq[sessionId]`, merges playback ranges, accumulates dwell, and tracks latest position.
 */
export function reduceObservations(
  progress: JinshuProgress | undefined,
  events: readonly Observation[],
): JinshuProgress {
  const seenSeq: Record<string, number> = { ...(progress?.seenSeq ?? {}) };
  let position = progress?.position;
  const blocks: Record<string, { dwellMs: number; seenRatio: number; contentHash?: string }> = {
    ...(progress?.blocks ?? {}),
  };
  const media: Record<string, { src: string; played: [number, number][]; positionMs: number; durationMs: number }> = {};
  if (progress?.media) {
    for (const [k, v] of Object.entries(progress.media)) {
      media[k] = { ...v, played: v.played.map(([a, b]) => [a, b]) };
    }
  }

  for (const ev of events) {
    const prevSeq = seenSeq[ev.sessionId] ?? -1;
    if (ev.seq <= prevSeq) continue;
    seenSeq[ev.sessionId] = ev.seq;

    if (ev.kind === 'position') {
      position = { blockId: ev.blockId, offset: ev.offset };
    } else if (ev.kind === 'dwell') {
      const prev = blocks[ev.blockId];
      const dwellMs = Math.max(0, ev.dwellMs);
      const seenRatio = Math.min(1, Math.max(0, ev.seenRatio));
      blocks[ev.blockId] = {
        dwellMs: (prev?.dwellMs ?? 0) + dwellMs,
        seenRatio: Math.max(prev?.seenRatio ?? 0, seenRatio),
        ...(ev.contentHash !== undefined
          ? { contentHash: ev.contentHash }
          : prev?.contentHash !== undefined
            ? { contentHash: prev.contentHash }
            : {}),
      };
    } else if (ev.kind === 'played') {
      const prev = media[ev.blockId];
      const sameSrc = prev && prev.src === ev.src;
      const baseIntervals = sameSrc ? prev.played : [];
      const [startMs, endMs] = ev.range;
      const merged = mergeIntervals([...baseIntervals, [Math.max(0, startMs), Math.max(0, endMs)]]);
      media[ev.blockId] = {
        src: ev.src,
        played: merged,
        positionMs: ev.ended ? ev.durationMs : Math.max(0, endMs),
        durationMs: Math.max(ev.durationMs, sameSrc ? prev.durationMs : 0),
      };
    }
  }

  // Cap tracked sessions to prevent unbounded growth.
  const sessionKeys = Object.keys(seenSeq);
  if (sessionKeys.length > MAX_TRACKED_SESSIONS) {
    const drop = sessionKeys.length - MAX_TRACKED_SESSIONS;
    for (let i = 0; i < drop; i++) delete seenSeq[sessionKeys[i]];
  }

  return {
    seenSeq,
    ...(position ? { position } : {}),
    blocks,
    media,
  };
}

/** Returns the fraction [0, 1] of unique duration played for a media block (verifying `src` if provided). */
export function playedRatio(
  progress: JinshuProgress | undefined,
  blockId: string,
  expectedSrc?: string,
): number {
  const entry = progress?.media[blockId];
  if (!entry || entry.durationMs <= 0) return 0;
  if (expectedSrc !== undefined && entry.src !== expectedSrc) return 0;
  let total = 0;
  for (const [a, b] of entry.played) {
    total += Math.max(0, b - a);
  }
  return Math.min(1, total / entry.durationMs);
}

/** Returns accumulated dwell time in ms for a block. */
export function dwellOf(progress: JinshuProgress | undefined, blockId: string): number {
  return progress?.blocks[blockId]?.dwellMs ?? 0;
}

/** Returns maximum seen ratio [0, 1] for a block. */
export function seenRatioOf(progress: JinshuProgress | undefined, blockId: string): number {
  return progress?.blocks[blockId]?.seenRatio ?? 0;
}
