/**
 * `@lingxia/test/runner`: what runs specs and renders their results — lxdev
 * drives it through the runtime's global controller, and this package's own
 * tests import it. A suite rarely needs it: `rawAutomation()` for setup a
 * run needs before any spec, `trackPublicSurface` for a conformance suite.
 */
export { run, list, reset, rawAutomation, trackPublicSurface } from "./runtime.js";
export { renderJUnit } from "./junit.js";
export { PUBLIC_CAPABILITIES } from "./inventory.js";
export type { Capability, CapabilityLayer } from "./inventory.js";
export {
  VERSION,
  PACKAGE_NAME,
  DEFAULT_ACTION_TIMEOUT_MS,
  DEFAULT_SPEC_TIMEOUT_MS,
} from "./version.js";
export type { LingxiaTestController } from "./host-types.js";
