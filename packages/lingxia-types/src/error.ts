import type { LxErrorReason, SurfaceErrorCode } from './generated/logic.js';

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
  /** For a log line; it can change between releases. */
  readonly message: string;
  /** What went wrong, stable across releases. */
  readonly code: LxErrorCode;
  /** Why it failed, where `code` alone does not say, such as `'locationPermissionDenied'`. */
  readonly reason?: LxErrorReason | (string & {});
  /** Free-form detail for a log line. */
  readonly detail?: string;
  /** Structured facts about the failure, such as the refused argument. */
  readonly data?: unknown;
}

/** What an `LxError` carries besides its message; `code` is what a caller branches on. */
export interface LxErrorOptions {
  /** What went wrong. */
  code: LxErrorCode;
  /** Why, where `code` alone does not say. */
  reason?: LxErrorReason | (string & {});
  /** Free-form detail for a log line. */
  detail?: string;
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
      if (options.detail !== undefined) self.detail = options.detail;
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

/**
 * The surface code carried by a `lx.surface.*` / `lx.shell.*` rejection.
 *
 * A rejection's `code` is the host code (`E_SURFACE_CONFLICT`,
 * `E_SURFACE_ROLE_LOCKED`, `E_CAPABILITY_MISSING`, …); the finer
 * `SurfaceErrorCode` rides on `data.code`. Reading it through this helper
 * keeps callers off both the message text and the shape.
 */
export function surfaceErrorCode(error: unknown): SurfaceErrorCode | null {
  const root = toRecord(error);
  const data = root ? toRecord(root.data) : null;
  const code = data?.code;
  return typeof code === 'string' && SURFACE_ERROR_CODES.includes(code as SurfaceErrorCode)
    ? (code as SurfaceErrorCode)
    : null;
}

/**
 * Why an lxapp refused to open, when the operator took it down.
 *
 * Both values block the open and mean opposite things to a user, so a caller
 * that shows one message for both is showing the wrong one half the time.
 */
export type LxAppUnavailableReason = 'maintain' | 'suspended';

/** Every member of `LxAppUnavailableReason`, for runtime narrowing. */
export const LXAPP_UNAVAILABLE_REASONS = ['maintain', 'suspended'] as const;

/**
 * Read why an open was refused, or `null` when it failed for another reason.
 *
 * Navigation rejects like any other `lx` API; the reason rides on `data.code`,
 * the same way a surface error carries its own. Branch on it rather than on the
 * message: `maintain` deserves "try again later" and `suspended` does not.
 *
 * ```ts
 * try {
 *   await lx.nav.go({ app: appId })
 * } catch (error) {
 *   const reason = lxAppUnavailableReason(error)
 *   if (reason === 'maintain') showMaintenanceNotice()
 *   else if (reason === 'suspended') showUnavailableNotice()
 *   else throw error
 * }
 * ```
 */
export function lxAppUnavailableReason(error: unknown): LxAppUnavailableReason | null {
  const root = toRecord(error);
  const data = root ? toRecord(root.data) : null;
  const code = data?.code;
  return typeof code === 'string' &&
    (LXAPP_UNAVAILABLE_REASONS as readonly string[]).includes(code)
    ? (code as LxAppUnavailableReason)
    : null;
}

/**
 * Whether an open, navigation, or update was refused because the host is
 * older than the lxapp's `minRuntime`.
 *
 * The rejection's `code` is `E_NOT_SUPPORTED`, which also covers missing
 * capabilities; only this says the user needs a newer host app.
 *
 * ```ts
 * try {
 *   await lx.nav.go({ app: appId })
 * } catch (error) {
 *   if (hostUpgradeRequired(error)) showUpdateHostNotice()
 *   else throw error
 * }
 * ```
 */
export function hostUpgradeRequired(error: unknown): boolean {
  return toRecord(error)?.reason === 'hostUpgradeRequired';
}

/** Every member of `SurfaceErrorCode`, for runtime narrowing. */
export const SURFACE_ERROR_CODES = [
  'unsupported_placement',
  'denied',
  'not_declared',
  'invalid_arg',
  'already_open_other_role',
  'role_locked',
  'closed',
  'capability_missing',
  'failed',
] as const satisfies readonly SurfaceErrorCode[];


function toRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null) return null;
  return value as Record<string, unknown>;
}

