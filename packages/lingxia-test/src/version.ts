export const VERSION = "0.11.2";
export const PACKAGE_NAME = "@lingxia/test";

export const DEFAULT_ACTION_TIMEOUT_MS = 5_000;
export const DEFAULT_SPEC_TIMEOUT_MS = 30_000;
/** Ceiling for an inherited eval budget, so one call cannot eat a long spec. */
export const MAX_EVAL_BUDGET_MS = 30_000;
/** Cleanup budget after a spec timed out — the app may be wedged, so bail fast. */
export const WEDGED_DEFER_BUDGET_MS = 2_000;
/** Cleanup budget on the normal path, capped so a leak cannot stall the run. */
export const MAX_DEFER_BUDGET_MS = 15_000;
export const DEFAULT_POLL_INTERVAL_MS = 50;
