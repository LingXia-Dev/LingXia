import type { Local } from '@lingxia/view';

/**
 * Sets a flag cell once a pending action settles, unless the component went away meanwhile (a
 * dialog closed under its own button). The View throws `UI_LOCAL … is no longer mounted` then; that
 * one error is expected here and nothing else is swallowed.
 */
export function settleFlag(cell: Local<boolean>, value: boolean): void {
  try {
    cell.set(value);
  } catch (error) {
    if (!(error instanceof Error) || !error.message.startsWith('UI_LOCAL:')) throw error;
  }
}
