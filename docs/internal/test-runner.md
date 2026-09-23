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
- The host holds one active run per session. Validate anything local (output
  directory) before `session.test.start`; once a run exists, every way the
  client stops without a terminal poll cancels it (`ActiveRun` in lxdev).
- `session.test.active` names the run holding the slot (`run_id`, `age_ms`,
  `since_last_poll_ms`); a refused `session.test.start` carries the same as
  error `data` under code `automation_run_in_progress`. `--cancel-active`
  refuses a run polled within the last 15s: its client is alive.
- `session.test.start` sends `args` (user `--arg`/`--secret-arg`, the spec's
  `t.args`) and `control` (grep, id, ids, shard, retries, passWithNoTests,
  forbidOnly, platform, secretArgs) separately. The host exposes `control` on
  `__LINGXIA_AUTOMATION_HOST__` only when non-empty; without it the framework
  reads those keys from `args`, as older lxdev sent them. Reports put controls
  in `meta.run`. A new lxdev against a host that drops `control` loses its
  filters; the two ship together.
- `t.skip()` throws the internal `SkipSignal`; from `beforeEach` or the body it
  grades `skipped` (also under `spec.fail`) and skips failure forensics. During
  cleanup (`t.defer`, `afterEach`) it rejects, and the case fails in the
  `defer` phase. `spec.fail` without `expected` inverts any body failure; with
  `expected` (`code`, `message` string or RegExp) only a matching one, and a
  mismatch is `failed` with what was expected prepended. Setup failures and
  timeouts keep their status.
- Secrets: `--secret-arg` values (≥ 4 chars) are masked by `@lingxia/test` in
  events, case records, `meta` and `t.attach`/forensics attachments, at the
  data level before rendering. A `--arg` key whose last word names a
  credential (`redact.ts` `looksSecretKey`, the only copy of the heuristic)
  is masked in `meta.args` only, never searched for in content. The masked
  args ride on `run_started`; lxdev uses them for reports it writes itself and
  for rerun hints (`key=<key>` placeholders), hiding every value when it never
  saw them. lxdev also scrubs declared values from every polled event (JSON
  re-serialized, text replaced) and withholds an HTML/XML artifact that still
  carries one, then renders its own report pages from the scrubbed JSON.
- Retry artifacts have attempt-specific paths. Do not overwrite the first failure
  with the successful attempt. Terminal output is a presentation of the report,
  not an alternate result model.
- A Runner-side run holds the single automation slot. The run manager cancels
  a run nobody has polled for `CONTROLLER_LEASE` (180s, above lxdev's longest
  poll gap) through the normal cancel path. `lingxia dev` likewise defers
  file-watch reloads while it relays an unfinished run.
- Fixture nav (`t.app.nav.*` and the `fresh` relaunch) sends `waitUntil: 'ready'`
  unless the spec chose `waitUntil`. The driver then polls the landed instance's
  `ready_dispatched` (`lxapp::automation::wait_page_runtime_ready`) off the Logic
  thread; Logic-side `lx.automation()` rejects `'ready'` because awaiting its own
  `onReady` would deadlock. The raw driver default stays `'commit'`. `fresh`
  ignores only the "disposed before ready" rejection (the home page's own
  hand-off); a ready timeout still fails the spec.
- Raw `page.waitFor` states follow `lxdev lxapp page wait` on the first match
  (`hidden` = exists and not visible). Locator states in `@lingxia/test` are
  uniqueness-aware (`attached`/`visible` need exactly one match, `hidden`
  includes no match). Keep both documented contracts when changing either.
- The spec timeout is a timer on the test JS worker; it cannot fire while a
  driver call blocks that thread in native code. The manager's run deadline and
  lease run off-worker and interrupt JS, and a worker still stuck after the
  grace period is declared wedged.
- Check actionability before dispatch. Never retry an ambiguous input transport
  failure: the original click may already have had its side effect.
- Hook scope: `spec.reset/beforeEach/afterEach` keep their raw registration
  frames (the bundle map is installed after the modules run). At run start the
  owner is the nearest mapped frame in a file that declares specs
  (`resolveOwner` in `ids.ts`), so a helper module's `installHooks()` belongs to
  the calling spec file, and a spec file exporting a hook helper keeps it. With
  no spec file on the stack the hook falls back to its first authored frame and
  a `diagnostic` event (`phase: "collect"`) says it never runs.
  `captureFrames` raises V8's `Error.stackTraceLimit` to 50 while capturing.
  Specs themselves are still attributed to their first authored frame.
- Generated ids (`<file stem>-<n>`) apply to specs with no `id` and no ASCII
  slug; `n` counts only those specs, per file, in registration order, assigned
  at run start once files are known. `n` used to be a run-wide counter over
  every spec, so ids from older reports can differ for suites with several
  files or with ASCII specs before a non-ASCII one.
- Action deadlines (`deadline.ts` `ActionDeadline`): locator `click/fill/type/
  press/waitFor`, locator matchers and `t.expect.poll` clamp their timeout to
  `LiveFixture.budgetRoom()` — the spec deadline less a reporting margin
  (`min(250ms, 5%)`), or the cleanup deadline during cleanup — and put the
  clamp in the failure message. Every driver call in those loops (query,
  actionability eval, dispatch, the poll `read`) is raced against the rest of
  the action budget and rejects with a `TimeoutError` naming the call, the
  action and its location, so the action fails before the spec timer. Raw
  driver calls (`t.app.eval`, nav) carry their own driver timeouts instead.
  The race only stops waiting: a native call that blocks the JS thread cannot
  be preempted from JS, and an abandoned call may still complete in the app.
- Transient page errors (`isTransientPageError`, mirroring
  `is_transient_page_error` in `lingxia-automation/src/page.rs`: `page is not
  active:`, `page WebView is not ready`, `WebView not ready`, `no current page`,
  WebView2 `0x8007139F`) are retried by locator actions and `waitFor` until the
  action budget ends; other errors fail at once. Dispatch retries only the
  page-resolution subset (`isPreDispatchPageError`): the WebView2 code can come
  from a script that already ran. Keep the list in step with the Rust helper.

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
- Function-form eval (`t.app.eval(fn, ...args)`, `t.app.page.eval(fn, ...)`,
  and `pageData`/`callPage`, built on it) lives in `@lingxia/test`
  (`remote.ts`); the drivers still receive `{ script }`. The script is one
  call expression, `((__lxFn, __lxArgs) => __lxFn(scope, ...__lxArgs))(<fn
  source>, <JSON args>)`, so the Logic expression-first eval and the WebView
  `await (expr)` path read it the same way and neither body heuristic
  applies. The Logic scope names `lx` unqualified, so the runtime's local
  recording `lx` still observes coverage. Methods and bound/native functions
  are rejected before sending (their source is not an expression). A remote
  `ReferenceError` is rethrown named `ReferenceError`, with its code/data and
  a note that `fn` cannot close over spec state; `t.waitFor` then fails fast.
- `t.waitFor` records one `waitFor` action and silences the reads inside it,
  like `t.expect.poll`. Its timeout is clamped to the spec budget left since
  the fixture was built, minus a 100 ms margin, so its own error (last
  value/error) wins over the bare spec timeout.
  Browser navigation eval returns `{ value, navigation }`; waits require exactly
  one condition. Preserve automation error codes and JSON data in test reports.
- `LxAppDriver.network` routes are owned by one host run: installation needs
  the context's run scope and checks the run is non-terminal under the route
  table lock; `RunShared::finalize` clears them after releasing its state lock
  (lock order: routes, then run state). Logic `fetch` is wrapped only when the
  `runtime` feature is built, and its no-route path is one atomic load.
  Fulfillments go through the app's domain policy; the fixture removes a
  spec's routes in its cleanup. Patterns compile to the Rust `regex` crate
  (globs are translated), so JS lookaround and backreferences are rejected at
  `route()`.
- Route handlers are validated in Rust (`parse_handler_value`), mirroring the
  exclusive `NetworkRouteHandler` union: fulfill keys, `abort`, and `continue`
  never mix. `abort` accepts only kinds the wrapper reproduces faithfully
  (`AbortKind`, today `failed`); add a kind only with a matching emulation.
  Binary `body` is read from the handler object before its JSON view, which
  would turn a `Uint8Array` into an index map. `delay` (at most 30 s) is a
  `setTimeout` in the Logic wrapper that the request's `AbortSignal` cancels.
- Request records are captured by the wrapper without consuming anything:
  headers via `Headers`, and the body only when it is a string,
  `URLSearchParams`, `ArrayBuffer`, or typed array (decoded as UTF-8). The
  wrapper pre-cuts to the limit and Rust cuts again on a UTF-8 boundary to
  64 KiB (`MAX_REQUEST_BODY_BYTES`); the process log also caps total body
  bytes. `NetworkDriver.requests()` reads the run-wide log; the fixture's
  filters it to the spec's route ids.
- `LxAppDriver.network` never throws on access: the driver authorizes per
  call. Builds without the `runtime` feature compile `network/unavailable.rs`,
  a driver whose calls all reject; from app Logic, calls reject for lack of a
  run scope.
- Dev WebSocket frame/message limits must fit both poll events (24 MiB) and the
  final result (8 MiB). A valid 16 MiB decoded attachment exceeds a 16 MiB frame
  after base64 encoding; both relay and CLI receiver need the shared limit.
