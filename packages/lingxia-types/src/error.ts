import type { SurfaceErrorCode } from './generated/logic.js';
import { ERR_CODE_INFO_BY_CODE, type LxErrorCodeInfo } from "./generated/error";

const ERR_CODE_INDEX = ERR_CODE_INFO_BY_CODE as Record<number, LxErrorCodeInfo>;

export interface LxApiError {
  readonly code: number;
  readonly key: LxErrorCodeInfo["key"];
  readonly message: string;
  readonly raw: unknown;
}

/** Check an already-normalized error without modifying the caught value. */
export function isLxApiError(error: unknown): error is LxApiError {
  const target = toRecord(error);
  if (!target) return false;
  const code = parseIntegerCode(target.code);
  if (code === null) return false;
  const info = infoForLxErrorCode(code);
  return Boolean(
    info &&
      target.code === code &&
      target.key === info.key &&
      typeof target.message === "string" &&
      Object.prototype.hasOwnProperty.call(target, "raw"),
  );
}

function readMessage(error: unknown): string {
  const root = toRecord(error);
  const data = root && toRecord(root.data);
  const detail = data?.detail;
  if (typeof detail === "string" && detail.trim() !== "") return detail;
  if (typeof error === "string") return error;
  if (error instanceof Error && typeof error.message === "string") return error.message;
  if (typeof error === "object" && error !== null) {
    const value = (error as { message?: unknown }).message;
    if (typeof value === "string") return value;
  }
  return "Unknown LingXia error";
}

function toRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null) return null;
  return value as Record<string, unknown>;
}

function parseIntegerCode(value: unknown): number | null {
  if (typeof value === "number" && Number.isInteger(value)) return value;
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    if (Number.isInteger(parsed)) return parsed;
  }
  return null;
}

export function extractLxErrorCode(error: unknown): number | null {
  const root = toRecord(error);
  if (!root) return null;

  const rootCode = parseIntegerCode(root.code);
  if (rootCode !== null) return rootCode;

  const data = toRecord(root.data);
  if (!data) return null;

  return parseIntegerCode(data.bizCode) ?? parseIntegerCode(data.code);
}

/**
 * The surface code carried by a `lx.surface.*` / `lx.shell.*` rejection.
 *
 * A rejection's `code` is the transport-level host code shared with every
 * other `lx` API; the surface-specific member of `SurfaceErrorCode` rides on
 * `data.code`. Reading it through this helper keeps callers off both the
 * message text and the shape.
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
 *   await lx.navigateToApp({ appId })
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

/** Every member of `SurfaceErrorCode`, for runtime narrowing. */
export const SURFACE_ERROR_CODES = [
  'unsupported_placement',
  'denied',
  'not_declared',
  'invalid_arg',
  'already_open_other_role',
  'closed',
  'capability_missing',
  'failed',
] as const satisfies readonly SurfaceErrorCode[];

export function isKnownLxErrorCode(code: number): boolean {
  return Number.isInteger(code) && Object.prototype.hasOwnProperty.call(ERR_CODE_INDEX, code);
}

export function infoForLxErrorCode(code: number): LxErrorCodeInfo | null {
  if (!isKnownLxErrorCode(code)) return null;
  return ERR_CODE_INDEX[code];
}

export function parseLxApiError(error: unknown): LxApiError | null {
  const code = extractLxErrorCode(error);
  if (code === null) return null;
  const info = infoForLxErrorCode(code);
  if (!info) return null;
  return {
    ...info,
    message: readMessage(error),
    raw: error,
  };
}

export function requireLxApiError(error: unknown): LxApiError {
  const parsed = parseLxApiError(error);
  if (parsed) return parsed;
  throw new Error(`Unknown LingXia API error: ${readMessage(error)}`);
}

export function formatLxApiError(error: LxApiError): string {
  return `[${error.code}] ${error.message}`;
}
