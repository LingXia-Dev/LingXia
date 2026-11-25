import type { LxErrorReason } from './generated/logic.js';

/** Every code an `lx` call documents with `@throws`. */
export const LX_ERROR_CODES = [
  'E_ABORT',
  'E_ALREADY_EXISTS',
  'E_CAPABILITY_MISSING',
  'E_INTERNAL',
  'E_INVALID_ARG',
  'E_INVALID_DATA',
  'E_INVALID_STATE',
  'E_IO',
  'E_NETWORK',
  'E_NOT_FOUND',
  'E_NOT_SUPPORTED',
  'E_OUT_OF_RANGE',
  'E_PERMISSION_DENIED',
  'E_QUOTA',
  'E_STREAM',
  'E_SURFACE_CLOSED',
  'E_SURFACE_CONFLICT',
  'E_SURFACE_ROLE_LOCKED',
  'E_TIMEOUT',
  'E_TYPE',
] as const;

/** A closed union, so a mistyped literal in `error.code === …` is a type error. */
export type LxErrorCode = (typeof LX_ERROR_CODES)[number];

/**
 * What an `lx` call throws or rejects with, argument refusals included, so one `catch` handles it.
 * Branch on `code`, never on `message`; a refusal is `E_INVALID_ARG` with `data.argument`, `path`,
 * `expected` and `got`:
 *
 * ```ts
 * async function pickFolder() {
 *   try {
 *     return await lx.pick.directory()
 *   } catch (e) {
 *     if (e instanceof LxError && e.code === 'E_NOT_SUPPORTED') return null
 *     throw e
 *   }
 * }
 * ```
 */
export interface LxError extends Error {
  /** The JavaScript error kind, such as `TypeError`; branch on `code` instead. */
  readonly name: string;
  /** Plain English for a log line; it can change between releases. Show users text chosen by `code` and `reason`. */
  readonly message: string;
  /** What went wrong, stable across releases. */
  readonly code: LxErrorCode;
  /** Why it failed, where `code` alone does not say, such as `'locationPermissionDenied'`. */
  readonly reason?: LxErrorReason | (string & {});
  /** Structured facts about the failure, such as the refused argument. */
  readonly data?: unknown;
}

/** What an `LxError` carries besides its message; `code` is what a caller branches on. */
export interface LxErrorOptions {
  /** What went wrong. */
  code: LxErrorCode;
  /** Why, where `code` alone does not say. */
  reason?: LxErrorReason | (string & {});
  /** Structured facts about the failure. */
  data?: unknown;
  /** The error that caused this one. */
  cause?: unknown;
}

interface LxErrorConstructor {
  /** An error as an `lx` call would raise it, such as a mock's rejection. */
  new (message: string, options: LxErrorOptions): LxError;
  /** The prototype every `lx` error shares, which `instanceof LxError` checks. */
  readonly prototype: LxError;
}

/** The class the Logic runtime builds every host error from; the same class elsewhere (Node tests). */
export const LxError: LxErrorConstructor =
  ((globalThis as Record<symbol, unknown>)[Symbol.for('lingxia.LxError')] as LxErrorConstructor | undefined) ??
  (class LxError extends Error {
    // The host builds its errors as `new LxError(message)` and sets the fields after, so the
    // options are optional at run time though the type requires them.
    constructor(message: string, options?: LxErrorOptions) {
      super(message);
      if (!options) return;
      const self = this as unknown as Record<string, unknown>;
      self.code = options.code;
      if (options.reason !== undefined) self.reason = options.reason;
      if (options.data !== undefined) self.data = options.data;
      if ('cause' in options) self.cause = options.cause;
    }
  } as unknown as LxErrorConstructor);

/** Whether `error` is an `lx` rejection, including one from another context or a mock, where `instanceof` cannot see it. */
export function isLxError(error: unknown): error is LxError {
  if (error instanceof LxError) return true;
  const target = toRecord(error);
  return Boolean(
    target &&
      typeof target.code === "string" &&
      /^E_[A-Z0-9_]+$/.test(target.code) &&
      typeof target.message === "string",
  );
}

function toRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null) return null;
  return value as Record<string, unknown>;
}

