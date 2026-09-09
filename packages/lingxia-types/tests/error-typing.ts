import { isKnownLxErrorCode, type LxApiError, type LxErrorCode } from "../src/index.js";

declare const error: LxApiError;

function branchesOnAClosedCodeUnion(): string {
  if (error.code === 12000) return error.message;
  // @ts-expect-error 999999 is not a LingXia error code
  if (error.code === 999999) return error.message;
  return "";
}

function narrowsAnUnknownNumber(code: number): LxErrorCode | null {
  return isKnownLxErrorCode(code) ? code : null;
}

export type ErrorTypingGate = [
  typeof branchesOnAClosedCodeUnion,
  typeof narrowsAnUnknownNumber,
];
