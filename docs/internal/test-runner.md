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
