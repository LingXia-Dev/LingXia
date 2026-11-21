import { isLxError, type LxError } from "../src/index.js";

declare const caught: unknown;

function branchesOnAClosedCodeUnion(): string {
  if (isLxError(caught) && caught.code === "E_NOT_SUPPORTED") return caught.message;
  // @ts-expect-error E_NOT_SUPPROTED is not a LingXia error code
  if (isLxError(caught) && caught.code === "E_NOT_SUPPROTED") return caught.message;
  // @ts-expect-error numeric codes are gone
  if (isLxError(caught) && caught.code === 1002) return "";
  return "";
}

// No class yet: `instanceof LxError` is not offered until every engine makes it true.
// @ts-expect-error LxError is a type only
void (caught instanceof LxError);

export type ErrorTypingGate = [typeof branchesOnAClosedCodeUnion];
