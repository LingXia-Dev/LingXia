import { ERR_CODE_INFO_BY_CODE, type LxErrorCodeInfo } from "./generated/error";

const ERR_CODE_INDEX = ERR_CODE_INFO_BY_CODE as Record<number, LxErrorCodeInfo>;

export interface LxApiError {
  readonly code: number;
  readonly key: LxErrorCodeInfo["key"];
  readonly message: string;
  readonly raw: unknown;
}

export function isLxApiError(error: unknown): error is LxApiError {
  return parseLxApiError(error) !== null;
}

function readMessage(error: unknown): string {
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
