import { isLxError, LxError } from "../src/index.js";

declare const caught: unknown;

function branchesOnAClosedCodeUnion(): string {
  if (isLxError(caught) && caught.code === "E_NOT_SUPPORTED") return caught.message;
  // @ts-expect-error E_NOT_SUPPROTED is not a LingXia error code
  if (isLxError(caught) && caught.code === "E_NOT_SUPPROTED") return caught.message;
  // @ts-expect-error numeric codes are gone
  if (isLxError(caught) && caught.code === 1002) return "";
  return "";
}

// A mock builds its rejection as the host does: the code is required, the rest optional.
const denied = new LxError("no photos", { code: "E_PERMISSION_DENIED", reason: "photoPermissionDenied" });
void (denied instanceof LxError && denied.code === "E_PERMISSION_DENIED");
// @ts-expect-error an LxError without a code cannot be branched on
void new LxError("x");
// @ts-expect-error codes are the closed union
void new LxError("x", { code: "E_NOT_SUPPROTED" });

export type ErrorTypingGate = [typeof branchesOnAClosedCodeUnion];
