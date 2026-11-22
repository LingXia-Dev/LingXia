import type { SurfaceErrorCode } from './generated/logic.js';

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
 * What an `lx` call rejects with. Branch on `code`, never on `message`:
 *
 * ```ts
 * async function pickFolder() {
 *   try {
 *     return await lx.pick.directory()
 *   } catch (e) {
 *     if (isLxError(e) && e.code === 'E_NOT_SUPPORTED') return null
 *     throw e
 *   }
 * }
 * ```
 */
export interface LxError {
  /** The JavaScript error kind, such as `TypeError`; branch on `code` instead. */
  readonly name: string;
  /** For a log line; it can change between releases. */
  readonly message: string;
  /** What went wrong, stable across releases. */
  readonly code: LxErrorCode;
}

/** Whether `error` is an `lx` rejection; works on every engine, where `instanceof` does not yet. */
export function isLxError(error: unknown): error is LxError {
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
  return Number(toRecord(toRecord(error)?.data)?.bizCode) === HOST_UPGRADE_REQUIRED;
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

const HOST_UPGRADE_REQUIRED = 6002;

function toRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null) return null;
  return value as Record<string, unknown>;
}

