import type { TestErrorCode } from "./types.js";

/**
 * Every `TestErrorCode`, for code that lists or checks them at run time. The
 * driver codes mirror `AUTOMATION_ERROR_CODES` in `@lingxia/types/automation`.
 */
export const TEST_ERROR_CODES = [
  "E_AUTOMATION",
  "E_AUTOMATION_PRIVILEGE",
  "E_PAGE_NOT_ACTIVE",
  "E_PAGE_NOT_READY",
  "E_ELEMENT_NOT_FOUND",
  "E_ELEMENT_NOT_INTERACTABLE",
  "E_AUTOMATION_TIMEOUT",
  "E_EVAL_SCRIPT",
  "E_EVAL_TIMEOUT",
  "E_PROFILE_NOT_ISOLATED",
  "E_CLOCK_NOT_INSTALLED",
  "E_CLOCK_INSTALLED",
  /** A routed response broke the run's OpenAPI contract (`lxdev test --openapi`). */
  "E_OPENAPI_CONTRACT",
  /** A fixture wait (`t.waitFor`, `waitForCall`, an action budget) ran out of time, or the spec did. */
  "E_TIMEOUT",
  /** `t.skip()` stopped the spec. */
  "E_SKIPPED",
] as const satisfies readonly TestErrorCode[];

// Fails to compile when `TestErrorCode` gains a code the list lacks.
type Unlisted = Exclude<TestErrorCode, (typeof TEST_ERROR_CODES)[number]>;
const listsEveryCode: [Unlisted] extends [never] ? true : Unlisted = true;
void listsEveryCode;
