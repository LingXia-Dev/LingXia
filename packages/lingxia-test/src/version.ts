export const VERSION = "0.14.0";
export const PACKAGE_NAME = "@lingxia/test";

export const DEFAULT_ACTION_TIMEOUT_MS = 5_000;
export const DEFAULT_SPEC_TIMEOUT_MS = 30_000;
/** Per-spec cap on recorded actions, so a loop cannot bloat the report. */
export const MAX_ACTIONS = 300;
/**
 * Ceiling for an inherited eval budget. The Showcase's own trace puts a single
 * call at 14ms median and 5.3s at the 99th percentile, so 10s is roughly twice
 * the worst real call while still leaving a default spec two thirds of its
 * budget to retry in.
 */
export const MAX_EVAL_BUDGET_MS = 10_000;
/** Failure forensics run against a possibly wedged app; bound them too. */
export const FORENSICS_BUDGET_MS = 10_000;
/** Cleanup budget after a spec timed out — the app may be wedged, so bail fast. */
export const WEDGED_DEFER_BUDGET_MS = 2_000;
/** Cleanup budget on the normal path, capped so a leak cannot stall the run. */
export const MAX_DEFER_BUDGET_MS = 15_000;
export const DEFAULT_POLL_INTERVAL_MS = 50;
