import type { TestErrorCode } from "./types.js";

/**
 * Every `TestErrorCode`, for code that lists or checks them at run time. The
 * driver codes mirror `AUTOMATION_ERROR_CODES` in `@lingxia/types/automation`,
 * less its driver timeout codes: a fixture call that times out rejects with
 * `E_TIMEOUT`.
 */
export const TEST_ERROR_CODES = [
  "E_AUTOMATION",
  "E_SCENARIO_STATE_UNKNOWN",
  "E_AUTOMATION_PRIVILEGE",
  "E_PAGE_NOT_ACTIVE",
  "E_PAGE_NOT_READY",
  "E_PAGE_ACTION",
  "E_ELEMENT_NOT_FOUND",
  "E_ELEMENT_NOT_INTERACTABLE",
  "E_EVAL_SCRIPT",
  "E_PROFILE_NOT_ISOLATED",
  "E_CLOCK_NOT_INSTALLED",
  "E_CLOCK_INSTALLED",
  "E_DESKTOP_USAGE",
  "E_DESKTOP_NOT_FOUND",
  "E_DESKTOP_AMBIGUOUS",
  "E_DESKTOP_PERMISSION",
  "E_DESKTOP_UNSUPPORTED",
  "E_DESKTOP_UNAVAILABLE",
  "E_DESKTOP_STALE",
  "E_DESKTOP_FAILED",
  /** A routed response broke the run's OpenAPI contract (`lxdev test --openapi`). */
  "E_OPENAPI_CONTRACT",
  /** A fixture call ran out of time (a wait, an action budget, an eval, a nav), or the spec did. */
  "E_TIMEOUT",
  /** `t.skip()` stopped the spec. */
  "E_SKIPPED",
] as const satisfies readonly TestErrorCode[];

// Fails to compile when `TestErrorCode` gains a code the list lacks.
type Unlisted = Exclude<TestErrorCode, (typeof TEST_ERROR_CODES)[number]>;
const listsEveryCode: [Unlisted] extends [never] ? true : Unlisted = true;
void listsEveryCode;
