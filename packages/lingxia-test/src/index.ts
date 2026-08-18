export { spec, expect, run, reset } from "./runtime.js";
export { AssertionError } from "./expect.js";
export { TimeoutError } from "./fixture.js";
export {
  VERSION,
  PACKAGE_NAME,
  DEFAULT_ACTION_TIMEOUT_MS,
  DEFAULT_SPEC_TIMEOUT_MS,
} from "./version.js";
export type {
  Apps,
  AttachmentRef,
  CaseRecord,
  ExpectOptions,
  Fixture,
  FixtureExpect,
  JsonReport,
  LingxiaTestController,
  Locator,
  LocatorMatchers,
  Matchers,
  ProtocolReport,
  RejectExpected,
  RetryMatchers,
  SpecBody,
  SpecOptions,
  SpecStatus,
  StepRecord,
  TestApp,
  TestPage,
} from "./types.js";
