/**
 * `@lingxia/test/runner`: what runs specs and renders their results — lxdev
 * drives it through the runtime's global controller, and this package's own
 * tests import it. A suite rarely needs it; `trackPublicSurface` is the one
 * call a conformance suite makes.
 */
export { run, list, reset, trackPublicSurface } from "./runtime.js";
export { renderJUnit } from "./junit.js";
export { PUBLIC_CAPABILITIES } from "./inventory.js";
export type { Capability, CapabilityLayer } from "./inventory.js";
export {
  VERSION,
  PACKAGE_NAME,
  DEFAULT_ACTION_TIMEOUT_MS,
  DEFAULT_SPEC_TIMEOUT_MS,
} from "./version.js";
export type { LingxiaTestController } from "./types.js";
