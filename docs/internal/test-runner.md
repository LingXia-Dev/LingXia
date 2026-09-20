# Test runner

`@lingxia/test` executes sequential specs in the host automation context.
`session.test` validates the framework result; `lxdev` journals events and writes
received artifacts. Keep the following invariants when changing these layers:

- New test contexts do not reset product state. Retrying requires a file-scoped
  `spec.reset` hook. A still-pending body or teardown makes the run partial and
  prevents subsequent bodies; reopening that fixture would admit zombie actions.
- The spec, cleanup, evidence, run, and transport have separate deadlines.
  Cleanup has a hard timer around arbitrary user promises, not just checks in
  driver methods. JS deadlines stop waiting, not arbitrary external side effects.
- `schema_version: 1` results retain the six case statuses, source identity,
  assertions, steps, attachments, attempts, and structured error fields through
  the host adapter. Legacy three-status results remain readable. Validate counts
  by status before grading; partial results never pass.
- Persist events before consuming them. Missing final reports, terminal timeout,
  cancellation and connection loss need client-side partial reports. A JUnit run
  error represents an incomplete run even when every completed case passed.
- Retry artifacts have attempt-specific paths. Do not overwrite the first failure
  with the successful attempt. Terminal output is a presentation of the report,
  not an alternate result model.
- Check actionability before dispatch. Never retry an ambiguous input transport
  failure: the original click may already have had its side effect.

## Automation JS boundary

- `@lingxia/types/automation` describes the raw host drivers. Keep Logic eval,
  WebView eval, browser tabs, and OS input as separate targets. Locators and
  retry policy belong to `@lingxia/test`, not the product automation runtime.
- `t.app`, `t.apps`, and `t.automation` share fixture guards and tracing.
  Test helpers must retain these wrappers instead of acquiring raw drivers.
  Code deliberately evaluated inside product Logic still uses `lx.automation()`.
- Native Rong class instances are callable (`typeof === "function"`). Keep the
  original receiver for native getters and methods; getter-returned drivers must
  be wrapped as namespaces, not rebound as functions.
- Locator queries, probes, and dispatch carry the same page and match index.
  Multiple matches remain ambiguous even if only one is visible. Named pages
  survive remounts; an instance id targets one live instance. Omitted page targets
  follow the current page on every operation.
- `eval<T>` declares the caller's expected result, not runtime validation.
  Browser navigation eval returns `{ value, navigation }`; waits require exactly
  one condition. Preserve automation error codes and JSON data in test reports.
- Dev WebSocket frame/message limits must fit both poll events (24 MiB) and the
  final result (8 MiB). A valid 16 MiB decoded attachment exceeds a 16 MiB frame
  after base64 encoding; both relay and CLI receiver need the shared limit.
