import type { Local } from '@lingxia/view';

// The page runtime has a console; Liuli's types leave out the DOM's.
declare const console: { error(...parts: unknown[]): void };

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

/** Logs an async action's rejection with the component and its label, so a failed action is never silent. */
export function reportFailure(component: string, label: string, error: unknown): void {
  console.error(`Liuli ${component} "${label}": the action failed`, error);
}
