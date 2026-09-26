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
  poll gap) through the normal cancel path. `lingxia dev` likewise holds its
  source watcher while a run is on (below).
- Fixture nav (`t.app.nav.*` and the `fresh` relaunch) sends `waitUntil: 'ready'`
  unless the spec chose `waitUntil`. The driver then polls the landed instance's
  `ready_dispatched` (`lxapp::automation::wait_page_runtime_ready`) off the Logic
  thread; Logic-side `lx.automation()` rejects `'ready'` because awaiting its own
  `onReady` would deadlock. The raw driver default stays `'commit'`. `fresh`
  ignores only the "disposed before ready" rejection (the home page's own
  hand-off); a ready timeout still fails the spec.
- Raw `page.waitFor` states follow `lxdev lxapp page wait` on the first match
  (`hidden` = exists and not visible). Locator states in `@lingxia/test` are
  uniqueness-aware (`attached`/`visible`/`inViewport` need exactly one match,
  `hidden` includes no match). Keep both documented contracts when changing
  either.
- The page query payload (`lxapp::automation::build_query_script`) reports
  `visible` as rendered — non-empty box, not `display:none`,
  `visibility:hidden` or `opacity:0` — independent of scroll, and
  `inViewport` as rendered and intersecting `window.inner*` (the one
  camelCase field of that otherwise 0.18-shaped snake_case record; it was
  `in_viewport` before 0.19, which the fixture still reads). Before this
  split `visible` was viewport-aware; `@lingxia/test` treats a missing
  `inViewport` (older runtime) as `visible`. The Browser driver's own query
  script still reports the viewport-aware `visible`. Native input paths keep
  their own viewport + hit-test refusal, which is actionability, not
  visibility.
- `force` on click/fill: the locator skips stability and the actionability
  eval and requires only one enabled (for fill, editable) match. The native
  side routes a forced click through `WebView::click_via_js(…, force)` on
  every platform — DOM events on the element, no viewport or
  `elementFromPoint` check, still refusing a disabled element — and a forced
  fill through `type_via_js` (Windows otherwise types through a native click
  at the element center).
- `filter({ hasText })`/`first()`/`last()` narrow the `all: true` query
  client-side; each item keeps its DOM index, which is what query, the
  actionability probe and dispatch address. `toHaveAttribute` reads
  attributes with a read-only page eval inside the retry loop; nothing is
  written to the page.
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
  press/waitFor`, locator matchers and `t.expect(fn)` clamp their timeout to
  `LiveFixture.budgetRoom()` — the spec deadline less a reporting margin
  (`min(250ms, 5%)`), or the cleanup deadline during cleanup — and put the
  clamp in the failure message. Every driver call in those loops (query,
  actionability eval, dispatch, the poll `read`) is raced against the rest of
  the action budget and rejects with a `TimeoutError` naming the call, the
  action and its location, so the action fails before the spec timer. Raw
  driver calls (`t.app.logic.eval`, nav) carry their own driver timeouts instead.
  The race only stops waiting: a native call that blocks the JS thread cannot
  be preempted from JS, and an abandoned call may still complete in the app.
- Transient page errors (`isTransientPageError`: code `E_PAGE_NOT_ACTIVE` or
  `E_PAGE_NOT_READY`; for hosts that predate the codes, the messages of
  `is_transient_page_error` in `lingxia-automation/src/page.rs`: `page is not
  active:`, `page WebView is not ready`, `WebView not ready`, `no current page`,
  WebView2 `0x8007139F`) are retried by locator actions and `waitFor` until the
  action budget ends; other errors fail at once. Dispatch retries only the
  page-resolution subset (`isPreDispatchPageError`, never the WebView2 code,
  which can come from a script that already ran) and element refusals
  (`isElementRefusal`: `E_ELEMENT_NOT_FOUND`/`E_ELEMENT_NOT_INTERACTABLE`, or
  the `Element not found|not interactable:` messages). Keep the lists in step
  with the Rust helpers. An action that times out on such a rejection carries
  its `code`/`data` on the `AssertionError`, so `spec.fail({ expected: { code
  } })` can pin it.
- Failure diagnostics: `LiveFixture` remembers the last failed recorded action
  with its error; the case's `error.failedAction` is set only when the case
  failed with that same error object (or the spec timed out while it ran), so
  an action a spec expected to reject is never blamed. `error.page` is the
  current page from the forensics `nav.current()` (`{ name, instanceId }`),
  else the rejection's `data.current`. `report.json` keeps `cases` and adds a
  flat `failures[]` (`id, title, file, line, phase, code, message,
  failedAction, page, screenshot`); lxdev rebuilds it for reports it writes or
  completes itself (`failure_records`) and prints the same one-line summary as
  the HTML report (`failed_at` / `failedAt`).

## Execution and bundling

```text
Development machine: lxdev collects/bundles specs → sends them over dev websocket
Target App/Runner:   test JS worker → automation → Logic / WebViews / host / HTTP
Development machine: lxdev receives progress, results, and artifacts
```

- The CLI never executes spec bodies. On a phone, specs and their `fetch` run
  on the phone; with a desktop Runner, in that Runner on the PC.
- The host reuses one automation worker and creates a fresh JS context per
  run. Specs execute sequentially and await async hooks and bodies.
- Directory input collects `*.test.ts` recursively, follows static imports, and
  strips TypeScript types into one JS script with a source map, reading sources
  without writing a temporary entry into the project. A `.json` import becomes
  a module whose `default` export is the parsed value (validated with line and
  column on error), shared by every importer.
- A spec, `spec.configure()` or hook belongs to the file of the nearest spec
  frame on its registration stack, mapped through the bundle map. Engines may
  evaluate the bundle behind lines of their own (Rong's JavaScriptCore backend
  prepends `"use strict";\n`, so frames arrive one line low). The bundle's
  line 3 measures that shift (`LINE_PROBE_LINE`), prepends as many empty
  lines to the map it installs for `@lingxia/test`, and moves a thrown error's
  frames back to bundle lines before lxdev maps them. A registration that
  still maps to no file fails the run; it is never left file-less.
- Cleanup order: `spec.afterEach`, then LIFO `t.defer`; `timeoutCleanup`
  bounds both. `restoreProfile`'s rollback is the first defer (so it runs
  last) and drops its checkpoint whether or not the rollback succeeded.
- Before each spec that runs, the runtime lists the lxapps; if the app under
  test (`spec.app`, else the app `describeSubject` saw at run start) is
  missing or `closed` (a `closing` one gets 5 s), it opens it, relaunches its
  home page, emits a `diagnostic` with `phase: "recovery"` naming the previous
  case and its status, and records an `app.reopen` step on the new case. A
  failed reopen is recorded the same way and the spec then fails on its own.
- Watch pause: before it bundles, `lxdev test` takes a `session.watch.pause`
  lease (`{ lease, ttl_ms: 300000 }`) from the dev server, and after
  `session.test.start` renews it with the run's id. The server (not the
  runtime) holds leases: a relayed `session.test.*` response for the bound
  run renews it while `running` and removes it on any other state, a lease
  not renewed within its TTL lapses (a `kill -9`'d client), and `lxdev`
  sends `session.watch.resume` from the guard's `Drop` and from the second
  Ctrl-C before `exit(130)`. `watch_paused()` = any live lease; a session
  that cannot pause fails the run before it bundles. The watcher keeps the
  dirty set while paused and rebuilds once when it clears. `restart_lxapp`
  repeats the check under the command lock every relayed request holds
  until its response is observed, so a run whose lease was taken during the
  (seconds-long) rebuild defers the restart instead of having its app
  replaced mid-spec.
- Run end: after the last spec, unless a spec left async work pending
  (`contaminated`), the runtime runs the same `reopenAppUnderTest` it runs
  before each spec. A failed reopen (there or per spec) is a `diagnostic`
  with `phase: "recovery_failed"`; lxdev sets `Outcome.app_not_live` from it
  and prints the `Recover:` line, as it does for a `timed_out` or
  `internal_error` run that was not interrupted.
- Version check: `lingxia_control_protocol::dev_session::compat`. Components
  are the CLI (`LXDEV_BUILD_VERSION` / `LINGXIA_BUILD_VERSION`: version plus
  `(sha[-dirty] date)`), the session owner (`SessionInfo.build`), the runtime
  (`PeerBuild` from the runtime `hello`, surfaced by `echo` as
  `runtimeBuild`), and `node_modules/@lingxia/*/package.json` of the project
  (and, for a host, each local lxapp bundle). Skew is a different major.minor,
  or — only when both sides name one — a different commit: a package's
  `+<sha>` build metadata or `"lingxia": { "commit" }`.
- Session selection: `dev_session::select` (broker feature) is the one
  resolver; `hint_selector` returns the shortest selector that resolves back
  to the same session from the same directory (none, name, target,
  `target@dir-name`, `target@path`) for printed hints. `SessionInfo.name` /
  `.build` are optional fields. `SessionInfo.extra` (`serde(flatten)`) keeps
  fields a build does not know, so a broker passes newer fields through; a
  broker built before `extra` still drops them, which is why a session with a
  `--name` verifies its registration (below).
- Runner build identity: the version alone does not name a build, so every
  Runner install writes `runner-build.json` beside the app — `{version,
  commit}` from `install-local-runner.sh`/`.ps1`, `{version, release: true}`
  when `runner_cache::ensure_runner` unpacks a release asset.
  `ensure_matching_runner` compares it with `CliBuild::current()`
  (`LINGXIA_COMMIT_HASH`, and `LINGXIA_RELEASE_BUILD`, which
  `scripts/release/cli.sh` sets): a checkout build needs the same commit; a
  release build accepts its release asset or its commit, and otherwise
  re-downloads. A CLI without a commit checks the version only.
  `LINGXIA_ALLOW_SKEW=1` warns instead. `doctor --project` shows the same
  verdict for a standalone lxapp. In `compat::check` a host that reports a
  commit (the session's `lingxia`) must match the checking CLI's commit.
- Session lifecycle: `lingxia dev` starts and stops sessions; `lxdev` never
  starts one. `lingxia dev --background` runs `run_background_owner`: it
  spawns the owner (`lingxia dev …` minus `--background`/`--json`, its own
  process group, output appended to `.lingxia/background/dev-<ms>.log`)
  and polls for a session of this project registered after the spawn whose
  runtime answers `echo` with `runtimeConnected`. The owner exiting, the
  timeout (`BACKGROUND_START_TIMEOUT`, 1800 s) or Ctrl-C (a `ctrlc` flag)
  ends the wait: the owner's process tree is terminated
  (`terminate_process_tree`) unless it already exited, and the error carries
  the log path and its last 30 non-empty lines (`\r`-redrawn progress
  collapsed to its last frame). The readiness probe is a closure, so unit
  tests drive it with `sh` owners and a seconds-long timeout.
- `lingxia dev stop [SESSION]`: `plan_stop` maps `select`'s result —
  `NoSessions` and `NoMatch` are "nothing to stop" (exit 0), `Ambiguous` is an
  error with the candidate table; a broker that cannot be queried or a
  session that will not die is an error too.
- Locator actions wait for a unique match, enabled/editable state, stable
  geometry, and an unobscured hit point (after `scrollIntoView`), retrying
  while a navigation is still replacing the page.
- Run budget: without `--timeout-secs`, lxdev sends `budgetPerSpecMs`
  (30 s), `budgetMinMs` (300 s) and `budgetMaxMs` (3600 s) as controls and
  asks the host for the 3600 s ceiling, because only the runtime knows the
  selection. The runtime computes max(min, planned executions × per-spec),
  capped at max, and checks it between specs; with `--timeout-secs` it gets
  `budgetMs` and the host deadline is the same value. Once spent, the rest
  are skipped with a "not run" reason, a `diagnostic` (`phase: "budget"`) is
  emitted, `meta.budget.exhausted_after` records N of `planned`, and the
  report is partial. A spec cut off by the host deadline instead ends the run
  `timeout`; lxdev then derives N/M from the streamed cases
  (`budget_exhaustion`). An older runtime ignores the budget controls and
  runs to the host ceiling.
- `--shuffle[=SEED]` sends `control.shuffle` (lxdev draws a u32 when no seed
  is given and prints it); the runtime shuffles the planned executions with a
  seeded Fisher–Yates (mulberry32), after `--repeat-each` expansion.
  `--repeat-each N` (`control.repeatEach`) plans each selected spec N times,
  adjacent unless shuffled; each execution is its own case (`repeat`,
  `[repeat k/N]` in `full_name`, attachment path `…/repeat-k/attempt-n`), and
  retries key attempts by id and repeat. `run_started` lists every planned
  execution, and lxdev fills the next unstarted entry with a matching name.
- `FILE:LINE`: lxdev parses the file (Oxc) for spec calls — `spec(…)` and
  `spec.skip|only|fixme|fail(…)`, under any local name `spec` is imported as
  — and takes the one whose call spans the line, else every call inside the
  smallest node around the line that holds any (a loop, a helper). It sends
  `control.locations`: each bundled file's source name → those calls' line
  ranges, `null` for a file given whole. The runtime keeps a spec only when
  its registration frame (source-mapped) is in its file's ranges; a spec
  registered from a file that is not a key is dropped.
- `--list` bundles with `Purpose::List`: the bundle calls the framework's
  `list()` instead of `run()`, and throws when there is none, so an older
  `@lingxia/test` fails rather than running the suite. `list()` selects as
  `run()` does, then returns a zero-case report with `listed`. lxdev writes
  no run directory and leaves `latest`.
- Rerun lines: `meta.reruns` maps each failed id to its command. `FILE:LINE`
  when the id is generated (`generated_id`), the file was one of the run's,
  and no other id shares that line; `--id` on the run's paths otherwise.
- `PageInfo.ready` means `onReady` ran; `webviewAttached` means the page has a
  WebView. `fresh` relaunches wait for ready but accept a home page that hands
  off to another page.

## Automation JS boundary

- `@lingxia/types/automation` describes the raw host drivers. Keep Logic eval,
  WebView eval, browser tabs, and OS input as separate targets. Locators and
  retry policy belong to `@lingxia/test`, not the product automation runtime.
- One runtime object, two typed roots. `Automation` (the global `lx.automation()`
  of app Logic) returns `LogicLxAppDriver`; `HostRunAutomation` (the
  `automation-test-globals` root) returns `LxAppDriver`, which adds `network`,
  nav `waitUntil: 'ready'`, and `captureCalls`. Those members reject from Logic
  at runtime: `network` needs a run scope, `ready` is refused without
  `HostAutomationAuthority`. `captureCalls`/`LxAppEvalTrace` are `@internal`
  report plumbing; `TestApp.eval` omits that overload because the fixture
  always strips the envelope. The Logic declaration stays global, not opt-in,
  so existing Logic that calls `lx.automation()` keeps type-checking.
- Privileges for Logic callers are the sealed session grants (`Automation`,
  `AutomationHost`), bounded by the app's allowed privilege classes;
  devtools hosts grant only those. `desktop` and `terminal` are gated by the
  `host` grant (plus the `desktop` feature / native terminal), not by being a
  dev/test host. A host run bypasses grants through its authority marker.
- The host-tier getters on the root (`lxapps`, `browser`, `shell`, `device`,
  `desktop`, `terminal`) check `host` on property read and throw, so enumerating
  or logging the root throws in a session without it; the drivers' methods
  recheck on every call. Making the getters lazy (like `LxAppDriver.network`)
  is a Rust change in `lingxia-automation`. The fixture hides it:
  `t.automation.browser/desktop/terminal` are `lazyDriver` proxies
  (`fixture.ts`) that resolve the getter chain on each call, so the read
  never throws and the call rejects; a proxy is not a thenable (`then` reads
  `undefined`).
- Fixture shapes (`@lingxia/test`) differ from the raw drivers on purpose,
  so `TestApp` is not assignable to `LxAppDriver`: removers resolve
  `undefined`; `network.ts` maps `NetworkRouteRequest` (`routeCall`; a plain
  `continue` answers `real`, the body is parsed when JSON) and `ScenarioCall`
  (`scenarioCall`; `rule !== null` → `rule`, an `answeredBy` starting with
  `route` → `route`, a Function call → `companion`, else `real`) to one
  `NetworkCall`. `waitForCall` keeps a cursor per route handle and per
  scenario target (JSON of the filter) and hands out `calls[cursor]`, so a
  call made before the wait counts; it polls silenced inside one traced
  action and throws a `TimeoutError` listing the last 10 calls.
  `t.profile.checkpoint()` wraps the raw id as `{ id }`. The clock wrapper
  reports dropped timers as a `diagnostic` (`phase: "clock"`).
- `t.app.view` is a plain object (locators, `eval(fn)`, `screenshot`,
  `scroll`, guarded `pointer`/`key`); there is no raw page on the fixture.
  The fixture's evals take functions only; the string form is the raw
  driver's.
- `t.expect` dispatches on its argument: a branded locator
  (`Symbol.for("lingxia.test.locator")`) → locator matchers, a function →
  the retrying poll, anything else → the once-matchers of `expect`. The
  top-level `expect` throws on a branded locator. `TEST_ERROR_CODES`
  (`errors.ts`) repeats `AUTOMATION_ERROR_CODES` literally — `@lingxia/test`
  has no runtime import of `@lingxia/types` — and a compile-time check fails
  when `TestErrorCode` gains a code the list lacks. Codes that reach a spec
  are `E_*`; lxdev's snake_case run errors stay in the CLI.
- Trace events are best effort: `emitTrace` resends a failed
  `step_started`/`step_finished` once and drops it, so a transport hiccup
  never fails an action (the report comes from the fixture's records).
  Idempotent reads (`IDEMPOTENT_READS`, `app.info/pages/surfaceLayout`,
  `view.screenshot`, locator queries) retry `isTransientTransportError`
  (`E_TRANSPORT`, `os error 11/35/54/104`, reset, broken pipe, closed
  channel/WebSocket) twice with backoff; input dispatch never does.
- `spec.configure` options and a spec's own options are validated at
  registration but settled at run start (`settle` in `runtime.ts`), once the
  bundle map attributes each call to its file: own options win, `tags`,
  `covers` and `requires` merge. An unmet `requires` skips the case before a
  fixture exists, with the reason in `case.reason`.
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
- Function-form eval (`t.app.logic.eval(fn, ...args)`, `t.app.view.eval(fn,
  ...)`, and `logic.data`/`logic.call`, built on it) lives in `@lingxia/test`
  (`remote.ts`); the drivers still receive `{ script }`. Leading eval options
  (`logic.eval({ timeout }, fn)`, `view.eval({ page, timeout }, fn)`) are told
  apart from `fn` by type: `page` goes to the driver's `page.eval`, `timeout`
  (clamped to `budgetRoom()`) replaces the default share as `timeoutMs`; an
  object with a `script` key is not options, so a script string still fails
  as one. The script is one
  call expression, `((__lxFn, __lxArgs) => __lxFn(scope, ...__lxArgs))(<fn
  source>, <JSON args>)`, so the Logic expression-first eval and the WebView
  `await (expr)` path read it the same way and neither body heuristic
  applies. The Logic scope names `lx` unqualified, so the runtime's local
  recording `lx` still observes coverage. Methods and bound/native functions
  are rejected before sending (their source is not an expression). A remote
  `ReferenceError` is rethrown named `ReferenceError`, with its code/data and
  a note that `fn` cannot close over spec state; `t.waitFor` then fails fast.
- `t.waitFor` records one `waitFor` action and silences the reads inside it,
  like `t.expect(fn)`. Its timeout is clamped to the spec budget left since
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
  exclusive `NetworkRouteHandler` union: fulfill keys, `abort`, `continue`
  (optionally with `patchJson`), and `hang` never mix. `abort` accepts only kinds the wrapper reproduces faithfully
  (`AbortKind`, today `failed`); add a kind only with a matching emulation.
  Binary `body` is read from the handler object before its JSON view, which
  would turn a `Uint8Array` into an index map. `delay` (at most 30 s) is a
  `setTimeout` in the Logic wrapper that the request's `AbortSignal` cancels.
- `patchJson`: `decide` returns the patch as JSON text; the wrapper calls the
  real `fetch`, reads `text()`, and a host function applies RFC 7396
  (`registry::merge_patch`) and returns the new body, so only data crosses
  into the app context. Status and headers are kept minus `content-length`
  and `content-encoding`; `serde_json` without `preserve_order` re-serializes
  with sorted keys. Non-JSON rejects with a `TypeError`, empty passes through.
  The record's `action` stays `continue`.
- `hang`: `decide` registers a held token (`Registry::held`) tied to run,
  app and route id, and the wrapper returns a promise that polls
  `holds(token)` every 200 ms. `remove(run, id)` (the fixture's spec-end
  `unroute`, also after `times` expired the route), `remove_app` and
  `clear_run` release the token; the promise then rejects like a transport
  failure. Like a fulfillment, a hang never applies to a host the domain
  policy refuses.
- Request records are captured by the wrapper without consuming anything:
  headers via `Headers`, and the body only when it is a string,
  `URLSearchParams`, `ArrayBuffer`, or typed array (decoded as UTF-8). The
  wrapper pre-cuts to the limit and Rust cuts again on a UTF-8 boundary to
  64 KiB (`MAX_REQUEST_BODY_BYTES`); the process log also caps total body
  bytes. `NetworkDriver.requests()` reads the run-wide log; the fixture's
  `t.app.network.calls()` filters it to the spec's route ids.
- Answers and sequences: a `RouteSpec` holds `answers` (never empty) and a
  `served` counter; `decide_route` serves `answers[min(served, len-1)]` and
  counts it before `times` is spent, so `times` still bounds matches.
  Handler parsing goes through `parse_answers` (`sequence` or one answer);
  only a top-level binary `body` survives, sequence items are parsed from
  their JSON view. Templates (`scenario::render_action`) are rendered on the
  cloned answer at serve time, over text bodies, header values, status text,
  `patchJson` strings and SSE fields — never over binary bodies. Unknown
  `{{…}}` text is kept verbatim.
- Scenario files: the format lives in `lingxia_control_protocol::scenario`
  (`parse_file` → `ScenarioFile`, `resolve(variant)` → `Resolved` with 1-based
  rule `index`, file `path` and `Target`), so lxdev, the host and a
  companion read files the same way. `check_rule` checks every rule of every
  variant at parse time (targets, `match` keys per target, `times`, `note`,
  the shape of `function` answers); the host checks `http` answers with the
  route handler parser. `network/scenario.rs::parse_scenario(file, variant)`
  compiles `http` rules into `RouteSpec`s carrying `body_match`
  (`scenario::Matcher`, feature `scenario-match`; object keys are checked in
  sorted order so the first difference reported is stable) and returns
  `(rule index, spec)` pairs.
- Installed scenarios (`registry::InstalledScenario`: id, owner, `RuleSlot`s
  with hits and route id, a bounded `ScenarioCall` log, `companion`):
  `Registry::install_scenario` sets each spec's `origin` (scenario id, rule
  index) and installs through `install_all`, which installs in reverse so the
  first rule is the newest route; `route()` calls made later still win. The
  dev scenario lives in `Registry::dev`, run scenarios in `run_scenarios`,
  one per run and app (`remove_run_scenario` replaces; `clear_run` drops
  them).
- Matching (`decide_route`): walk routes newest first; a route whose target
  matches but whose `body_match` fails the request's JSON body is passed
  over and its reason kept. The request body is read once, lazily, only
  when a candidate has `match`. The answering route's origin gets a hit and
  a `ScenarioCall`; every other scenario whose rules were passed over gets a
  `ScenarioCall` with `no_match` and what answered instead
  (`Answered`: a rule, `route <pattern>`, or the network). The second return
  value (`NoMatch`) is set when no scenario rule answered: the interceptor
  logs it as a warning for the dev owner, and it lands on the call-log entry
  (`Call::no_match`). `Call::to_json` adds `answeredBy` (`rule k
  (name:variant)`, `route <pattern>`, `real`).
- The run API (`network/test_scenario.rs`): `lxapp().scenario(file, variant?)`
  parses, removes the run's previous scenario for the app, sends `function`
  rules upstream (below), then installs `http` rules; a failure after the
  companion accepted clears it again. `JSScenario` keeps the resolved rules
  and a snapshot; `rules` and `calls()` read the live entry by scenario id.
  `@lingxia/test`'s `ScenarioScope` tracks the spec's handle, takes the
  failure evidence (`report()`, before the defers) and unroutes it in a
  defer.
- SSE answers (`RouteAction::Sse`) carry frames and fields per step. The
  `fetch` wrapper builds a `ReadableStream` (JS `start` + `setTimeout`) that
  enqueues frames, honours `delayMs`, closes on `drop`, and otherwise polls
  `holds(hold)` like a hang; `decide_route` allocates the hold token only
  when the answer does not end in `drop`. Rong's `Response(ReadableStream)`
  materializes the whole body on `.body`/`.text()`, which matches the real
  path: rong_rt coalesces streamed fetch bodies into
  `DEFAULT_STREAM_COALESCE_TARGET` (512 KiB) chunks, so Logic `fetch` is not
  an incremental SSE consumer either way.
- `Rong.SSE` is replaced by the wrapper `SSE_INTERCEPTOR` returns.
  Extensions run after `js_runtime.rs` freezes the reserved `Rong`
  namespace, so `register_automation_runtime` registers it with
  `lx::register_rong_member_wrapper("SSE", …)`, applied just before the
  freeze. That registry can only replace a member a context already has,
  never add one, so `Rong` stays closed to extensions. It exists only with
  lingxia-lxapp's `automation` feature (doc-hidden); without it nothing is
  registered and `Rong` is sealed as created. With `active()` false the
  wrapper constructs the native class unchanged. Otherwise each connection
  attempt calls `observe`/`decide` (`GET`, `accept: text/event-stream`, the
  caller's headers, `last-event-id` on reconnects). No route, `continue` or
  `patchJson` hands the connection to the native client (later reconnects
  included, with the last id as a header). A routed attempt is played by a
  JS object on `OriginalSSE.prototype` (so `instanceof` holds) with own
  `next`/`close`/`return`/`url`/`Symbol.asyncIterator`, mirroring
  `rong_rt::sse::run_sse_worker`: non-200 or a non-event-stream content type
  fails without reconnect; a transport failure (`abort`, released `hang`)
  and a stream end (`drop`, a released hold, the end of a fulfilled body)
  reconnect with doubling backoff from `baseDelayMs` (reset by `retry`),
  capped at `maxDelayMs`, while `enabled` and `maxRetries` allow; `retries`
  resets on every successful open. A fulfilled `text/event-stream` body is
  parsed in JS. `requestTimeoutMs` is not emulated.
- Watching without routes (call log, recording, contract capture): see
  [Network observation](#network-observation).
- Dev-session scenarios (`network/dev.rs`, `session.network.*` in
  `lingxia-control-runtime`) install under the owner `@dev-session`. The
  host receives the whole file plus `variant` (`dryRun` only validates) and
  installs its `http` rules; `function` rules are listed in the status for
  numbering. Dev routes are skipped by `decide_route` while any run is
  active and answer again when it ends. Install, replace, clear, every
  answered request and every no-match write `LogBuilder` warnings; the dev
  bridge disconnecting clears the scenario (`bridge.rs` → `session_ended`,
  also on a transient reconnect: failing closed). An app relaunch keeps it:
  routes match by appid, not by context. `Registry::end_dev_scenario(reason)`
  is the one way a dev scenario ends; it keeps `dev_cleared` for
  `lastCleared`. While a dev scenario exists `observe` logs every call, so
  `session.network.status` can return `calls` with `answeredBy`. The methods
  exist only with the `test-runtime` feature; lxdev maps `unknown_method` to
  "built without the automation test runtime".
- `lxdev scenario` (`tools/lingxia-devtools-cli/src/scenario.rs`): `resolve`
  takes a file that exists, else `name[:variant]` (a file may carry
  `:variant` too) over `search_roots` (session content dir, project root,
  then the lxapp project of the current directory when it lies inside one of
  those), each joined with `tests/scenarios`. `install` resolves the
  variant, has the host dry-run the file, checks
  `session.companion.capabilities` when the file has `function` rules (and
  refuses the whole file without `scenario.function`), sends
  `scenario.use { owner: "dev" }` through the dev server, then installs the
  `http` rules; a host failure clears the companion again. A file without
  `function` rules clears the companion's `dev` owner. `status` merges the
  companion's per-rule hits onto `function` rules; `reached()` drives the
  app-cache hint (after `use`, a 4 s wait on a TTY; in `status`, whenever
  nothing reached it). `--watch` polls the file every 300 ms and reinstalls
  on a content change (`watch()`, testable with a fake `Session`). `list`
  needs no session. `lxdev test` no longer consults the dev scenario.
- The companion side of `function` rules (dev server relay, owner
  lifecycle, runtime upstream) and its wire contract:
  [scenario-companion-protocol.md](scenario-companion-protocol.md).
- Automation errors (`lingxia-automation/src/error.rs`): the lower half returns
  strings that older clients parse, so messages never change; `code_for` /
  `eval_code_for` map them to stable codes (`E_AUTOMATION_PRIVILEGE`,
  `E_PAGE_NOT_ACTIVE`, `E_PAGE_NOT_READY`, `E_ELEMENT_NOT_FOUND`,
  `E_ELEMENT_NOT_INTERACTABLE`, `E_AUTOMATION_TIMEOUT`, `E_EVAL_SCRIPT`,
  `E_EVAL_TIMEOUT`), else `E_AUTOMATION`. Page-driver failures carry `data:
  { page, instanceId?, current? }` read when they fail. The TS list is
  `AUTOMATION_ERROR_CODES` in `@lingxia/types/automation`; change both
  together. `PageInfo.instanceId` comes from the resolved `PageInstance`.
- `LxAppDriver.network` never throws on access: the driver authorizes per
  call. The same holds for every driver property (`lxapps`, `browser`,
  `desktop`, `page`, `nav`, `cookies`, desktop sub-drivers, …): each method
  re-checks the calling context (`require_host_context` /
  `upgrade_authorized`) and rejects with `E_AUTOMATION_PRIVILEGE`. Builds without the `runtime` feature compile `network/unavailable.rs`,
  a driver whose calls all reject; from app Logic, calls reject for lack of a
  run scope.
- Dev session: a runtime disconnect while `lingxia dev` relays an active run
  (seen from `session.test` responses within the lease) moves the run to
  `interrupted_test_run` and uses a 30 s gone-grace instead of 1 s. A poll for
  that run meanwhile gets error `runtime_reconnecting`; lxdev treats it as a
  pause, journals `runtime_disconnected`/`runtime_reconnected` events
  (`source: "lxdev"`), and a reconnect restores the run as active. After the
  grace a desktop session ends as before.
- Results root: `TestOptions::results_root` is `--output-root`, else
  `test_preset::results_root(cwd)` — `test.outputDir` or `test-results`
  joined to the directory of `lxdev.json` — else `./test-results`. A run
  writes `<root>/<run-id>` unless `--output-dir` names its directory;
  `update_latest(root, run_dir)` skips a run directory equal to the root.
  `resolve_report_path(path, root)` reads `latest` from that root, for
  `--last-failed` and `lxdev test report` (which takes `--output-root` too).
- `--last-failed`: `record_rerun` writes `meta.run_settings`
  (`preset`, `profile` — a PATH made absolute — and `profile_save`) into
  report.json. Before preset expansion, `test_preset::carry_last_failed`
  reads them from the report `--last-failed` names and splices `--preset`,
  `--profile`, `--profile-save=` right after `test`, each only when the
  command line (`scan`) lacks it (profile also not when it gives
  `--preset`, or when the carried preset sets the same profile), and a
  preset only if it would not add a second entry. It does
  nothing when the report cannot be read or has no failed case. After
  parsing, `test::nothing_to_rerun` (before a session is resolved) prints
  `No failed specs in <run>; nothing to rerun` — `kind: "result"`,
  `state: "nothing_to_rerun"` for `--format json|jsonl` — and exits 0.
- `--print-args` also prints `ArgSources` (environment and
  `--secrets-file`) as `# --flag KEY=VALUE  (from SOURCE)` lines, secret
  values `***` and credential-named `LXDEV_ARG_` values masked; JSON has
  them under `external`.
- Dev broker: `lingxia dev-broker` answers `info` with its `BrokerBuild`
  (CLI version, executable path, executable mtime at start) and live session
  count, and `shutdown` by exiting only when no session is registered (the
  check holds the session lock). `log_store::ensure_current_broker` probes
  it before registering (and, with `--name`, before the build in the
  foreground `lingxia dev`) and replaces any mismatched
  broker: an idle one is asked to `shutdown`; a busy one is terminated by the
  pid `info` reported (its sessions' `register_session` threads reconnect a
  second later and re-register); one older than `info` is found by a process
  scan (`<lingxia*> dev-broker`, same user). The current build is spawned
  right after, before those sessions come back, and must answer `info` as
  this build. A busy stale broker used to be only reported, and an older one
  dropped `name` from the record. If replacing fails, `--name` makes it an
  error; after registering, `verify_registered_name` lists the broker and
  fails the session if its record lacks the name.
- Dev WebSocket frame/message limits must fit both poll events (24 MiB) and the
  final result (8 MiB). A valid 16 MiB decoded attachment exceeds a 16 MiB frame
  after base64 encoding; both relay and CLI receiver need the shared limit.

## Network observation

One observation per Logic request feeds three consumers: the call log a
failed spec reports, a network recording (`--record-network`, `lxdev network
record`), and the contract capture `--openapi` reads
(`NetworkDriver.captureResponses()` / `responses()`). They share one wrapper
per context, one `observe`/`settle` pair and one read of a response body;
each consumer keeps its own bounds and redaction in
`lingxia-automation/src/network/capture.rs`, and all state lives in the
route table (`Registry`) under its one lock.

- Wrapping: `install_fetch_interceptor` replaces the global `fetch` and the
  `Rong.SSE` member wrapper replaces `Rong.SSE`, once per Logic context. Both
  wrappers carry `Symbol.for('lingxia.automation.network.wrapped')` and an
  installer that finds it returns the existing wrapper, and
  `lx::register_rong_member_wrapper` ignores a repeated registration, so a
  context never observes a call twice. Both capture the real timers when
  they are built, before any test clock can be installed.
- Fast path: `ACTIVE` counts routes, active runs (`begin_run` from
  `attach_run_scope`, ended by `clear_run`), recordings and captured apps.
  While it is 0, `fetch` calls the native one with the caller's `this` and
  arguments and returns its promise untouched, and `new Rong.SSE()` constructs
  the native client. A context no lxapp owns is never observed either.
- `observe(kind, method, url)` opens a `CallLog` entry when a run, recording
  or capture watches and answers `[id, record, contract]` (`capture::Watch`):
  `record` when the recording wants this `fetch` (SSE connections are never
  recorded), `contract` when a run captures the app's `fetch` responses.
  `decide_route(…, call)` marks the entry routed: a routed answer clears
  `record` (a recording keeps what the server said); a fulfillment is
  captured right there from the route's own answer and clears `contract`,
  as do `abort`, `hang` and `sse`; `continue` and `patchJson` keep it.
- One read: the wrapper's `consume` step is the only place a body is read
  for Rust. With `record` it reads `text()` for textual types or
  `arrayBuffer()` for binary ones within 64 KiB (a larger declared
  `content-length` and event streams are noted unread); with only `contract`
  it reads `text()` of a JSON type (`application/json`, `text/json`,
  `*+json`; never `x-ndjson`, `json-seq` or SSE, which an app may read
  incrementally; `JSON_TYPE` mirrors `capture::is_json_type`). It settles the
  call with the body and hands the app a rebuilt `Response` (status, status
  text, headers minus `content-length`/`content-encoding`, `url`; `redirected`
  and `type` are not reproduced). Anything else settles with status and
  content type and the app gets the original response. `patchJson` reads the
  real body anyway: it settles with the patched text (source `patch`), and
  the generic settle after it is a no-op (`CallLog::settle` ignores a second
  settle).
- `Registry::settle` hands a settled call to the capture (`Observed::settled`:
  source `patch` or `network`, JSON text only) and, unless it was the app's
  own `AbortError`, to the recording.
- Call log: 200 calls process-wide, URLs through `capture::redact_url`
  (userinfo dropped, secret-named query values `***`). The host object's
  `networkLog(sinceMs, limit)` returns the newest entries with the run's
  `--secret-arg` values masked; `@lingxia/test` stores up to 20 on the failed
  case's `error.network` (flattened into `failures[]`, rendered by
  `renderNetwork`), and lxdev copies `error.network` into the `failures[]` it
  rebuilds.
- Recordings (`capture::Recording`): `Registry::dev_recording` (the dev
  session) and `run_recording` (a run's `networkRecord`). `observe` picks the
  run's while any run is active, else the dev one, so a dev recording pauses
  during runs, and stores the chosen owner on the call (`Call::recorder`);
  `settle` pushes only into that recording. At most 500 exchanges and 16 MiB
  of bodies per recording; text over
  1 MiB and binary over 64 KiB are noted. `to_scenario` writes one `http`
  rule per method and URL, keeps one answer only when all are equal (else a `sequence` of up to
  100, so the n-th replayed call gets the n-th recorded answer), turns
  transport failures into `abort`, drops `AbortError`s, and redacts
  credential-named fields (`redact_json`, `SECRET_NAMES`, compared
  case-insensitively without `_`/`-`), JWTs and `Bearer` values in every text
  body and JSON string (`redact_text`), and query values (`url_pattern`).
  `dev::run_record` masks the run's `--secret-arg` values in the stopped
  scenario. For `--record-network` the framework starts a recording per spec
  and attaches the stopped scenario as `network.scenario.json` (masked like
  any attachment); lxdev copies each to `<dir>/<spec id>.json` after the
  run, suffixing `-2`, `-3`… when two spec ids sanitize to one name.
  `lxdev network record stop --out [--name]` creates a placeholder next to the output
  before `RECORD_STOP` and renames over it after writing; on a write failure
  it prints the scenario to stdout and fails.
- A pass-through `Rong.SSE` (no route, or `patchJson`) is the native client;
  its call-log entry settles on the first `next()` (200) or its rejection
  (`settleOnOpen`), so reports never show a null status for it.
- Contract capture (`capture::Captures`): per run and appid, enabled by
  `captureResponses({ maxBodyBytes })` (default 256 KiB, at most 1 MiB) and
  cleared by `clear_run` with the routes. Records keep method,
  `scheme://host/path` (`contract_url`: no userinfo, query or fragment),
  source (`route`/`patch`/`network`), route pattern, status, content type
  and the JSON body cut on a character boundary (`bodyTruncated`); never
  headers. Bodies are not redacted, because schemas validate them; they stay
  in the process and only issue paths and messages reach the report, which
  masks `--secret-arg` values like the rest. The process log holds at most
  500 records and 8 MiB of bodies; `responses({ since })` reads past a
  `seq`.

## Isolated data profiles

Isolation, snapshot and restore happen in the host, never in the app under
test: no test writes into its storage and app code has no test branch.

- `lxapp::data_profile` keeps a process-memory override per appid.
  `initialize_paths` reads it once per `LxApp` instance and resolves
  `storage.redb`, `userdata`, `usercache` and `temp` under the profile's
  `live/` root; the bundle, fingermark and every shared store (`lxapps.redb`,
  downloads, settings) are unchanged. Page WebViews are already ephemeral
  (`StrictDefault`); `BrowserRelaxed` tabs still share the Runner's store.
- Every switch is close → change → reopen (`with_app_closed`, serialized by
  one lock): retire the instance, wait for its `logic_contexts` to reach 0 so
  no redb handle is open, run the change, reopen with the captured open mode,
  panel and initial route, and wait for the app to settle. Copying a live
  redb is never safe; that is why a checkpoint costs two reopens.
- Settling (`wait_settled`, the `Settle` state machine): `App.onLaunch` has
  settled (`LxApp::launch_settled`, set when the handler's promise settles,
  reset whenever onLaunch is due again), the current page is ready, and its
  instance id has not changed for `SETTLE_QUIET` (300 ms). No ready page
  within `REOPEN_TIMEOUT` fails the switch; still unsettled `SETTLE_TIMEOUT`
  (5 s) after the first ready page is only logged. A page the app replaces at
  start-up is not an error, unlike `wait_page_runtime_ready`.
- `leave` clears the override before closing anything, so a failed close or
  reopen still leaves the next instance on its own data. A crash leaves no
  override; startup (`prepare_directory_structure`) sweeps
  `<data>/lingxia/test-profiles`, which is outside the cleanup roots.
- `checkpoint`/`restore` refuse unless the app currently runs on that exact
  profile (`RunProfile::is_active_for`), and the driver additionally requires
  the context's `ProfileRunScope`, attached only to runs started isolated.
  Rollback therefore cannot reach real data (`E_PROFILE_NOT_ISOLATED`).
- `session.test.start` with `profile` checks the slot, creates the profile,
  unpacks the seed (manifest `format`, `appid`, `fingermark`,
  `storage_format` must match; only regular files under `data/`, 256 MiB cap),
  switches, then starts; a refused start `abandon`s it. The run owns the
  `AutomationProfile`: `RunShared::finalize` (every terminal path) starts the
  teardown on its own thread. Until it finishes, `retire_completed` keeps the
  run in the slot and `poll` reports `running`, so neither the next start nor
  lxdev's export/rerun races a half-switched app, and the dev server keeps
  deferring file-watch reloads. Retained profiles expire after 300 s.
- Snapshots travel only over `session.profile.*` (≤ 4 MiB decoded chunks,
  offset-checked, sha256), never the artifact channel, so they stay out of
  `output_dir`, reports and secret scrubbing. lxdev refuses isolation unless
  `session.test.capabilities` reports `profile`: an older host ignores the
  start field and would run on real data. `meta.run` gets `profile` and
  `profile_seed` (`<name>@<sha256[:8]>`), never paths or contents.
- A profile switch reopens the app as a new instance; drivers hold a `Weak`
  to the old one. `LiveFixture` re-selects the lxapp after `checkpoint` /
  `restore`. `restoreProfile` checkpoints before the implied relaunch and
  registers restore + drop as the first defer; if it does not complete, the
  run is contaminated (partial), like a stuck cleanup.
- `restore(id, { keep })` (and `restoreProfile: { keep }`) merges inside the
  same closed-app window: `replace_live_keeping` reads the live
  `storage.redb` entries whose keys match the globs (`KeepKeys`: `*`, `?`, ≤ 64
  patterns), copies the checkpoint to the staged directory, makes the matching
  keys of the staged `storage.redb` exactly that set (write the current
  values, remove matching keys the live data no longer has), then swaps. A
  failed read or merge removes the staged copy and leaves live untouched, and
  the app is reopened only after the swap, so it never observes the
  checkpoint's values of a kept key. Values are copied as opaque bytes from
  the rong_storage table (`"storage"`, `&str → &[u8]`, `STORAGE_FORMAT`).
  Files are never kept. The driver resolves `{ kept }` (keys carried over);
  `LiveFixture` refuses `keep` on a host without `LxAppDriver.clock` (they
  shipped together) before calling restore, because an older host would
  silently ignore it.
- Profile switches and development rebuilds: a dev bundle is read straight
  from the build output, which a rebuild empties first. `lingxia dev` holds
  `DevServerState::build_lock` for each rebuild (file watch and `lxapp.build`)
  and a relayed `session.test.start` takes it before the command lock, so a
  start's switch never reopens a half-written bundle. For anything that still
  races (an external build, a lease that expired), `reopen_and_wait` retries
  a failed reopen of a dev bundle every 250 ms for `REOPEN_TIMEOUT`
  (`retry_while`), and `lxapp_dev_restart` opens a dev bundle that is not live
  instead of failing, so the next rebuild's reload brings a home app whose
  setup failed back. `prepare_lxapp_open` lets the home app itself open while
  it is not live.
- Not isolated in v1: downloads and `downloads.redb`, other lxapps opened in
  the run, and browser-profile WebViews. Mobile hosts share the Rust path but
  are not yet validated (transfer limits, `test-profiles` writability).

## Logic context lifetime

A profile switch, a dev reload and a restart each replace the app's Logic
context on a pooled worker VM, hundreds of times in a long run. Two things
kept every retired context alive on JavaScriptCore, and with it the app's
whole heap:

- A rong JS value held by native code is a GC root and retains its global
  context. `PageSvc` held itself (`this`) and its page's functions that way.
  The object now lives in `PageObject`, a cell shared by every clone;
  `PageSvc::release_js` empties it and clears the JS object's own copy
  (functions, emitter), pending callbacks and channels. It runs when a
  terminated page has no lifecycle event left (`TerminatePage`), when the
  lifecycle pump finishes a terminated page's owed `onUnload`, and for every
  live or retired page when the context deactivates. A released service
  answers requests with `BRIDGE_CANCELED`.
- Native objects that hold JS values (`AbortController`, `Response`, …) free
  their context only once finalized, which takes a full collection that also
  sweeps. JavaScriptCore schedules that on the VM thread's run loop, and Rong
  workers run none, so on Apple platforms a retired context waits for a
  collection that allocation alone brings late. The fix belongs in Rong (a
  worker idle hook that lets JSC's own timers run; public CoreFoundation
  only). LingXia no longer forces a collection: it used a private JSC
  function, and no Apple private API may ship in any build.
- Keep new native-held JS values releasable, or reachable only from JS.
  `a_retired_logic_context_is_released` checks that every Rust owner of a
  retired context lets go.

Host side:

- WebKit keeps a page's `WKUserContentController` beyond its `WKWebView`;
  teardown empties its script message handlers and user scripts so the
  handlers (and the native component bridge) do not outlive the view. The
  `+1` `WKUserScript`s and scheme-handler `NSHTTPURLResponse`s are released.
- The Runner's DevTools console (`DevToolsLogger`) keeps at most 2000 entries,
  and a new simulator window — one per app open — replays them in one edit.
  It used to replay the whole session's log line by line, scrolling after
  each, on the main thread at every reopen, which made each profile rollback
  slower than the last.

## Automation run lifetime

- The automation worker evaluates each run in a fresh context. What the
  context can reach — `console`, `attach`, `emit`, `logs` — holds the run's
  `RunShared` weakly, so the report, events and attachments go when the
  manager stops keeping the run (the active run plus
  `COMPLETED_RETAINED` finished ones), whenever the engine frees the
  context itself. A host function called after that rejects with
  `E_AUTOMATION_ENDED`. `a_finished_run_is_freed_once_it_leaves_retention`
  guards it.
- `destroy_webview_if_matches` also drops the WebView's event normalizer
  (only while the tag's normalizer is still that native view's). Tags carry
  the app session, so a normalizer left behind was never reused and one piled
  up per page per reopen.

### Soak check

Repeat a whole suite in one Runner and sample the Runner's footprint after
each run (`footprint -p <pid>`, and `ps -o rss=`). The showcase, with its own
HOME so no other session is touched:

```bash
export HOME=/tmp/lxsoak PATH=/tmp/lxsoak/bin:$PATH   # CLI + Runner built from this tree
cd examples/lingxia-showcase/lxapp
lingxia dev --background --framework react
for i in 1 2 3; do
  lxdev test --preset macos --arg framework=react --output-dir /tmp/lxsoak/run$i
  footprint -p "$(pgrep -f /tmp/lxsoak/.lingxia/runner)" | head -3
done
```

Growth per run should level off rather than add up; RSS alone understates
it once macOS compresses pages. `heap <pid> | grep
JSGlobalObjectInspectorController` counts live JS global objects (one per
context).

Showcase `--preset macos` (203 specs), macOS x86_64, Runner footprint after
each run (2026-09):

| build | run 1 | run 2 | run 3 | idle 3 min later |
|---|---|---|---|---|
| before (private forced collection of retired Logic contexts) | 1786 MB | 3445 MB | 3947 MB | — |
| now (no private API) | 1634 MB | 3137 MB | 3881 MB | 3904 MB |
| now + worker run-loop pump (prototype) | 1384 MB | 2115 MB | 1460 MB | 1110 MB |

Live JS global objects stay at 3–5 in every build, so retired contexts are
not what grows: it is garbage in the live heaps (almost all of it "WebKit
malloc", JSC's 16 KB blocks). JavaScriptCore's full collections and sweeping
run from the VM thread's run loop, which Rong workers do not run; allocation
alone mostly triggers young-generation collections. The prototype, which
runs the worker's run loop every 50 ms, lets them run; the fix belongs in
Rong's workers.

## Test clock

`LxAppDriver.clock` fakes time in one lxapp's Logic context only; the
automation context, page WebViews and native code keep real time.

- `AutomationExtension::init` (runtime builds) evaluates `clock/fake_clock.js`
  in every Logic context after the fetch interceptor. It captures the real
  `setTimeout`/`setInterval`/`clearInterval` and a host `turn` for its own
  use and defines a dormant, frozen controller at `globalThis[Symbol.for('lingxia.automation.clock')]`;
  nothing global changes until `install`.
- Driver calls go through `LxApp::eval_logic` as controller calls carrying an
  install token (`clock.tick(token, ms)`); answers are JSON, `{ error }`
  values map to `E_CLOCK_INSTALLED`, `E_CLOCK_NOT_INSTALLED` or
  `E_AUTOMATION`. Every call needs the context's `ClockRunScope` (attached to
  host runs only), so app Logic's own `lx.automation()` is refused.
- `install` swaps `Date` (a function sharing `Date.prototype`, statics
  copied), the four timer functions and an own `performance.now`. Fake ids
  start at `0xC0000000`, far above the real registry's counter, because the
  real `clearTimeout` applies ToUint32 and a stale fake id passed to it after
  uninstall must not cancel a real timer. `clear*` of a non-fake id is
  forwarded to the real function (timers created before install stay real).
  Fakes captured by app code keep working after uninstall by forwarding to
  the saved natives.
- Firing: earliest `(at, seq)`; an interval re-arms before its callback runs,
  and a throwing interval is cancelled and the error rethrown on a real turn
  (the real registry does the same). After each firing `settle` yields real
  turns until one passes without fake-timer activity (at least 3, at most
  100), so promise chains and immediately-completing native futures (routed
  `fetch`, `Response.json`, storage) progress before the next timer. A turn
  is `host_turn`, a round trip through `RongExecutor::global()`, not a real
  `setTimeout(0)`: `rong_timer` 0.6.0 can drop a one-shot timer whose tick is
  still queued when its spawn task finishes (likely at delay 0), which would
  stall the settle loop. `tick` caps at 10000 firings, `runAll` at
  `maxTimers`.
- Leases: `install` inserts `token → (run, appid)` before evaluating;
  `RunShared::finalize` calls `clock::clear_run`. The controller checks its
  lease on a real 1 s interval and uninstalls itself when it is gone, so a
  cancelled, timed-out or disconnected run cannot leave the app on fake time
  and finalization never has to reach into Logic. An install that finds a
  clock whose lease is gone replaces it.
- A reopened app has a fresh context (real time); its old lease is replaced
  by the next install or dropped at uninstall/run end. `uninstall` drops
  pending fake timers and reports the count; `LiveFixture` (`ClockScope`)
  uninstalls every app a spec installed on, re-selecting it by appid, and
  sets `forceRelaunchNext` when timers were dropped.
- Framework timers must not be faked: `Page.js` gets `setTimeout` /
  `clearTimeout` for the `setData` debounce through a hidden
  `__lxCaptureTimers` hook that `js_runtime.rs` calls right after
  `rong_modules::init` and then deletes (`page::capture_real_timers`), before
  any app code or test clock runs; and the `fetch` and
  `Rong.SSE` wrappers capture them when they are built, so route
  `delay`/`hang`, SSE `delayMs`, holds and reconnect backoff keep real time.
  Observation, recording and capture use no timers; scenario time templates
  read the host's wall clock.

## Presets

`lxdev test --preset NAME` (`tools/lingxia-devtools-cli/src/test_preset.rs`)
is expanded before clap sees the command line: `main` calls `expand` on
`argv`, which finds the `test` subcommand (skipping the global `--session`),
reads the `--preset` value before any `--`, and splices the preset's
strings right after `test`. clap then parses the result once; `TestOptions`
sets `args_override_self`, so a scalar the command line repeats replaces the
preset's, and `Vec` flags append. `--preset` stays in `argv` (clap records
it, and a second one is a clap error). `lxdev.json` is read from
`find_project_root(cwd)`; `parse` rejects unknown keys, bad names, non-string
items, the flags in `FORBIDDEN` and `--arg` keys `looks_secret_key` flags
(the Rust port of `@lingxia/test`'s `looksSecretKey`). `--list-presets` and
`--print-args` run before a session is resolved; `effective_args` drops
`--preset`/`--print-args` and masks `--secret-arg` values and credential-named
`--arg` values. Before splicing, `anchor_paths` makes the preset's relative
paths absolute against the directory of `lxdev.json`: the positional entry,
the values of `PATH_FLAGS`, and a `--profile` value that names a PATH
(`test_state::names_a_path`). `lxdev.json`'s `test.entry|outputDir|openapi|
tags` defaults (`outputDir` as `--output-root`) are spliced in first, each only when the preset and the
command line (`scan`) leave that flag — or the entry — unset, and not for
`report`, `--cancel-active` or `--list-presets`. Which tokens are values comes from clap's
own `TestOptions` definition (`takes_values`, optional values only when the
next token is not a flag), so a new flag cannot be misparsed; a new path flag
must be added to `PATH_FLAGS`. The command line's own tokens are untouched.
The `Rerun:` hint (`test::rerun_command`) is built from the parsed options, so
it carries the preset's flags already anchored; `StateOptions::rerun_flags`
repeats `--profile` and `--profile-save` (a saved profile is always the
snapshot it was read from, a rolling one). The base command (without
`--id`) is also written to `report.json` as `meta.rerun` for
`lxdev test report`. The JSON Schemas for scenario files and `lxdev.json` ship in
`@lingxia/test` (`schemas/`); `tests/schemas.test.mjs` checks them against
the same cases the Rust parsers reject.

## Tags, coverage manifest and OpenAPI contract

- Tags: `SpecOptions.tags` plus `spec.configure({ tags })`, which is
  file-scoped like the hooks (`resolveOwner` at run start; a call with no
  spec file on its stack emits a `collect` diagnostic). A case's `tags` are
  the file's, then its own, deduplicated. `--tag` values travel as
  `control.tags`, a JSON list of clauses: each clause is comma-separated
  terms (`tag` / `!tag`) of which one must hold, and every clause must hold
  (CNF). lxdev checks the syntax with the same grammar
  (`test_contract::parse_tag_expr`); selection itself happens in the runtime,
  ANDed with `only`, `ids`, `id`, `shard` and `grep`, and marks the report
  `filtered`. Tags ride on `run_started` and `case_started`, so reports lxdev
  writes for an interrupted run keep them.
- `tag_summary` (report.json), the HTML "By tag" table and the JUnit root
  `<properties>` (`tag:<name>`) count each tag's cases; untagged cases appear
  as `(untagged)` only when some case is tagged. lxdev recomputes
  `tag_summary` whenever it regrades cases (`refresh_report_file`).
- `--covers-manifest`: lxdev reads JSON or YAML (`serde_yaml_ng`, already in
  the workspace), normalizes it to `[{ id, title? }]` and sends it as
  `control.coversManifest` plus `coversManifestFile`. The runtime builds
  `coverage` from every registered spec, not only the selected ones (those
  are `not_run`), and emits a `coverage` diagnostic for `covers` ids missing
  from the manifest. lxdev refreshes spec statuses from regraded cases and
  keeps the runtime's `not_run` entries.
- Payload controls (`openapi`, `coversManifest`) never reach `meta.run`: the
  runtime (`PAYLOAD_CONTROL_KEYS`) and lxdev (`reported_control`) drop them
  and keep the file names.

### OpenAPI contract

- Where validation runs: in `@lingxia/test`, in the test worker. That is the
  only place `expect(value).toMatchSchema()` can answer synchronously, the
  runtime already owns grading, and it keeps the host free of a schema
  engine: `schema.ts` is a dependency-free validator for what OpenAPI schemas
  use (`$ref`, 3.0 `nullable`/boolean exclusive bounds, 3.1 type arrays and
  numeric exclusive bounds, `enum`/`const`, `required` with the
  `writeOnly`-in-responses exemption, `properties`/`patternProperties`/
  `additionalProperties`, `items`/`prefixItems`/`contains`, `allOf`/`anyOf`/
  `oneOf` with `discriminator`, `not`, `if`/`then`/`else`). `format` is not
  asserted; 3.0 ignores a `$ref`'s siblings, 3.1 applies them. A Rust crate
  in the host (the `jsonschema` family) would pull a regex/URL/HTTP stack into
  every automation build and still leave `toMatchSchema` to cross the driver
  boundary asynchronously.
- lxdev parses the documents (JSON/YAML), refuses Swagger 2, anything but
  3.0/3.1, and non-local `$ref`s, and sends them as `control.openapi`
  (`[{ name, doc }]`, at most 4 MiB as JSON) with `openapiFiles`.
  `openapi.ts` indexes operations by method, server base path (server
  variables at their defaults; path- and operation-level `servers` win) and
  path template; hosts are not compared, a concrete path beats a template.
  Response lookup is exact status, then `NXX`, then `default`; media type is
  the exact JSON type, then `application/json*`, any `+json`, `*/*`.
- Capture: with `--openapi` the runtime calls
  `lxapp().network.captureResponses()` for the spec's app before each body
  (idempotent); how responses are captured is in
  [Network observation](#network-observation).
- After each spec's cleanup the runtime reads `responses({ since })` past a
  run-wide watermark, so a response that arrives after a spec ended is
  checked with the next spec. Routed (`route`, `patch`) mismatches become a
  `ContractError` (`E_OPENAPI_CONTRACT`, phase `contract`) before `spec.fail`
  grading, so a known break can be declared; a spec that already failed gets
  the violations appended. Real-server mismatches are `contract.warnings` on
  the case, `openapi.warnings` (at most 100) and a `contract` diagnostic;
  unmatched requests, undocumented statuses and skipped responses (no
  schema, not JSON, empty, truncated) are only counted. A host that cannot
  capture responses fails the run: there is no contract run without them.
