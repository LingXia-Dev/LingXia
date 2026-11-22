import type { TransferProgress } from '../ProgressBar/ProgressBar';

/** The amount a progress control draws and reports: `now` out of `max`, or `now` undefined while it is unknown. */
export function amount(value: number | undefined, max: number, progress: TransferProgress | null | undefined): { now: number | undefined; max: number } {
  if (progress === undefined) return { now: value === undefined ? undefined : Math.min(max, Math.max(0, value)), max };
  if (progress === null || progress.fraction === undefined) return { now: undefined, max: 100 };
  // Bytes when the total is known, so a screen reader's value matches the transfer; else the fraction out of 100.
  if (progress.total !== undefined && progress.total > 0) return { now: Math.min(progress.total, Math.max(0, progress.loaded)), max: progress.total };
  return { now: Math.min(100, Math.max(0, progress.fraction * 100)), max: 100 };
}
