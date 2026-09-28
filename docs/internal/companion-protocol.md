# The dev companion protocol

`lingxia dev` may start one **companion** per session, from
`.lingxia/dev-companion.json` (see
`tools/lingxia-cli/src/commands/dev/companion.rs`): a process that serves
the app's Worker Functions during development. LingXia knows nothing about
Workers — it never parses Function Definitions, a Worker project's files, or
its mock configuration. It reaches the companion only through the messages
below. This note is the contract a companion implements; it has two optional
parts, each behind a capability:

- `scenario.function`: scenario `function` rules (`scenario.*`).
- `mock`: the Function half of the mock selection (`mock.*`, and the
  `session.prepare` `mock` param).

## Capabilities

A companion lists what it implements in its `hello` `capabilities`, or in
its `session.prepare` result's `capabilities` (the two are merged):
`scenario.function` (`dev_session::capabilities::SCENARIO_FUNCTION`) and
`mock` (`dev_session::capabilities::MOCK`). Nothing of a part is sent
without its capability:

- Without `scenario.function`, every scenario with a `function` rule is
  refused as a whole — by `lxdev mock use` and by `t.scenario.use()` —
  before anything is installed, with a message naming the rules and the
  reason (no companion, or a companion without the capability).
- Without `mock`, the HTTP half of a mock selection applies and one line
  says the Function half was not switched
  (`functions: not switched — this dev session has no companion` /
  `… — the companion does not switch mocks (no 'mock' capability)`).
  Nothing is partially claimed: a command that named a Function target
  exits 1.

## Transport

After `session.prepare` the dev server writes `request` frames to the
companion's stdin and reads `response` frames (same `id`) from its stdout,
interleaved with its `event_batch` frames. Frames are the dev-session wire
(`DevSessionMessage`), one JSON object per line. The companion keeps a
request loop running after `session.prepare`; stdin EOF is the shutdown
signal. The server waits up to 15 s for an answer (a `mock.set` may take
seconds); a late answer is dropped.

Clients never talk to the companion directly. They send
`session.companion.<method>` to the dev server, which strips the prefix and
forwards the `scenario.*` and `mock.*` methods below (anything else is
`unknown_method`):

- `lxdev mock …` and `lxdev network status` send them over their client
  websocket.
- A host test run (`t.scenario.use()`) sends them up the runtime's dev bridge
  (`lingxia-control-runtime/src/bridge_upstream.rs`); the dev server answers
  on the same connection. A host without a dev bridge refuses `function`
  rules and Function targets.
- `session.companion.capabilities` is answered by the server itself:
  `{ companion: bool, capabilities: [..] }`.

A missing companion or capability answers `companion_unsupported`; a
companion error is passed through unchanged (code, message, data).

## `session.prepare`

The first request. Its params are sent only when set, so a companion that
predates them keeps seeing no params:

```json
{ "mock": "all" }
```

`mock` (`"all"` or `"none"`, `DevSessionPrepareParams`) is the session's
mock baseline (`lingxia dev --mock`): it replaces the companion's own
configured selection as a whole for the session, exactly as the companion's
own command-line switch would. Absent means the companion's own default.

## Scenario methods (`scenario.function`)

Types are in `lingxia_control_protocol::scenario::companion`.

### `scenario.use`

```json
{ "owner": "test:5b0c…", "scenario": { "name": "Checkout", "variant": "unknown", "source": "checkout" },
  "rules": [ { "function": "orders.submit", "fault": "unknown" },
             { "function": "orders.status", "sequence": [ { "result": { "state": "pending" } },
                                                          { "result": { "state": "paid" } } ] } ] }
```

Install `rules` for `owner`, replacing everything that owner had. `rules` are
the file's `function` rules verbatim, in precedence order (a variant's rules
first); their shape is already checked (`scenario::check_rule`). The
companion validates them against its Function Definitions — unknown Function,
`result` not matching the declared type, undeclared `error.code`, an invalid
`match` regex — and either installs all or none: on an error the owner's
previous rules keep answering.

Result: `{ "installed": <count> }`. Rejection: code `invalid_rules`, `data`
`{ "errors": [ { "rule": <0-based position in rules>, "message": "…" } ] }`.
LingXia turns positions into the file's rule numbers and paths
(`rule 3 (variants.unknown.rules[0]) function orders.submit: …`).

### `scenario.clear`

`{ "owner": "dev" }` → `{ "cleared": bool }`. Clearing an owner with nothing
installed succeeds with `false`.

### `scenario.status`

`{}` → `{ "owners": [ { "owner": "dev", "active": true, "rules": [ { "hits": 2 } ] } ] }`.
`rules` follow the owner's `scenario.use` order; `hits` counts calls a rule
answered. `active` is false while a test owner sits above `dev`.

### `scenario.calls`

`{ "since": <epoch ms>, "owner"?: "test:…", "after"?: <seq> }` →

```json
{ "calls": [
  { "seq": 41, "time": 1727260000000, "function": "orders.submit", "args": { "cart": "c1" },
    "owner": "test:5b0c…", "rule": 0, "outcome": "fault" },
  { "seq": 42, "time": 1727260000200, "function": "orders.status", "outcome": "default",
    "handler": "mock", "noMatch": "rule 1 match.args.id: missing" } ],
  "droppedThrough": 0 }
```

Every Function call since `since` (all owners when `owner` is absent) whose
`seq` is above `after` (default 0), oldest first, bounded by the companion.
`seq` (required) numbers the companion's log from 1, increasing and never
reused, so calls in one millisecond differ. `droppedThrough` (required) is
the highest `seq` the bounded log dropped among calls that started at or
after `since`, whatever their owner or Function; 0 when it dropped none of
them. A test's `waitForCall` resumes by `after` and fails, naming the gap,
when `droppedThrough` passes its cursor. `owner`/`rule` name the rule that answered;
both are absent when the project's own handler did (`outcome: "default"`),
and then `handler` says which one answered, `"mock"` or `"real"` (omit it
without the `mock` capability). `outcome` is `result`, `error`, `fault` or
`default`. `noMatch` explains a call whose Function had rules but none
matched, in the form LingXia uses for HTTP (`rule <position> <path>: <reason>`).

### Rule semantics

Exactly as for `http` rules:

- Rules are tried in order; the first whose `function` equals the call's
  name and whose `match.args` matches answers. None: the mock selection
  below decides (mock or real handler).
- `match.args` is a deep subset: listed object keys must be present and
  match, arrays match element by element with equal length, a
  `"/regex/flags"` string (flags among `imsu`) matches a scalar's text, other
  values by equality. `scenario::Matcher` (feature `scenario-match`)
  implements exactly this and may be reused.
- Answers: `result` (the Function's return value), `error: { code, … }` (a
  declared business error, delivered as the Function would), `fault:
  "notRun"` (the call fails before the Function runs) or `"unknown"` (the
  call's outcome is lost to the client after it ran), each with an optional
  `delay` (ms, ≤ 30000).
- `sequence` answers in call order per rule, the last repeating; `times: n`
  retires a rule after n answers.
- Strings may hold `{{now}}`, `{{now±N<unit>}}` (ISO-8601 UTC) and
  `{{nowMs}}`, rendered when the answer is served (units `ms s m h d`).

### Scenario owners

- `dev`: installed by `lxdev mock use`. The dev server clears it when the
  runtime connection drops (a dev scenario fails closed, like its HTTP half),
  and `lxdev mock use` of a file without `function` rules clears it.
- `test:<run id>`: the host installs an empty owner and waits for its
  acknowledgement before evaluating test JavaScript. `t.scenario.use()`
  replaces its rules; the spec's end empties them. The dev server clears the
  owner when it sees the run reach a terminal state, including when startup
  acknowledgement was lost. Cleanup failure stops later test starts.
- A test owner sits above `dev`, even when empty: while one exists, `dev`
  rules stand aside (only the test owner's rules answer; a call they do not
  match goes to the mock selection), and `dev` answers again once the test
  owner is cleared. So a dev scenario never steers a test, and `lxdev test`
  never refuses to start because of one.

## Mock methods (`mock`)

Types are in `lingxia_control_protocol::mock::companion`. The companion
keeps its mock handlers and its configured selection in the Worker project,
in whatever form it likes, and keeps them complete (one mock per Function);
LingXia only switches between them.

### Selection semantics

The same as LingXia's HTTP half (`lingxia_control_protocol::mock::Selection`
implements it and may be reused):

- An *entry* is a whole (`all` or `none` for every Function) or targets
  (`all` or `none` for the named Functions).
- Owners, lowest first: the companion's own configured selection (replaced
  as a whole by the `session.prepare` baseline, when one is set), `dev`,
  `test:<run id>`.
- `mock.set` without `targets` replaces the owner's entries with one whole
  entry; with `targets` it appends a targets entry.
- For one call: take the highest owner that has entries (`dev` is skipped
  while a `test:` owner exists); walk its entries newest first; the first
  targets entry naming the Function, or the first whole entry, decides. When
  none of that owner's entries decides, fall to the next owner down; below
  every owner, the configured selection decides.
- A scenario rule answers before any of this; a rule's `continue`-style
  fallthrough (no rule matched) reaches the selection.

### `mock.set`

```json
{ "owner": "dev", "mode": "none", "targets": ["orders.submit"] }
```

`mode` is `all`, `none`, or `default`; `targets` are Function names (absent
for a whole entry; not allowed with `default`). `default` drops the owner's
entries, so the owners below decide — down to the companion's configured
selection. Unknown names reject the whole call, applying nothing: code
`invalid_targets`, `data` `{ "unknown": ["orders.submitt"] }`. The call may
take seconds (the companion may rebuild to switch handlers).

Result: `{ "mocked": 7, "total": 12 }` — Functions whose mock handler
answers now, and Functions the project defines.

### `mock.status`

`{}` →

```json
{ "mocked": 7, "total": 12,
  "owners": [ { "owner": "dev", "active": true, "entries": [ { "mode": "none", "targets": ["orders.submit"] } ] } ],
  "handlers": [ { "function": "orders.submit", "mock": false, "hits": 3 } ],
  "reset": "fresh" }
```

`owners` lists the owners that have entries (`active` is false for `dev`
while a test owner exists); `handlers` (optional) says per Function which
handler answers now and how many calls it answered; `reset` says whether
`mock.reset` can start handler memory over (`fresh`) or not (`shared`).
`lxdev mock` prints `functions: 7/12 mocked (orders.submit 3×) — live (lxdev mock all)`:
the layer is the highest active owner with entries (`test:…`, `dev`,
`baseline`, `config`), else the companion's configured selection.

### `mock.reset`

`{ "owner"?: "test:5b0c…" }` → `{ "reset": true }` or
`{ "reset": false, "reason": "…" }`.

Start mock handler memory over (a fresh module instance), for the owner's
calls when one is named. Sent before a test run's code starts and at each of its spec starts
(`owner: "test:<run id>"`) and by `lxdev mock reset` (`owner: "dev"`). A
companion that cannot do it cheaply answers `false` with a reason; LingXia
prints it once per run and goes on.

A `mock.reset` or `mock.set` for a `test:<run id>` owner creates that
owner: from then until it is dropped, `dev` stands aside.

### Mock owners

- `dev`: set by `lxdev mock all|none`. The dev server drops it
  (`mock.set { owner: "dev", mode: "default" }`) on `lxdev mock reset` and
  when the runtime connection drops, like a dev scenario.
- `test:<run id>`: the host creates it (`mock.reset`) before any test code
  runs and drops it (`mock.set { owner, mode: "default" }`) at run end; the
  run's own selection sets it in between. The dev server repeats the drop
  when it sees the run reach a terminal state.
- Handler memory is not preserved across a run: after a run the dev
  handlers may start fresh.

## Lifetime

- The companion keeps overlays and selections in memory and never edits the
  project's files.
- It hot-reloads its own mock handlers when their files change.
- When the session ends the companion process ends with it.

## Coordinated scenario changes

The CLI validates locally, pauses new Logic network calls with a host-issued
scenario generation, updates the companion, then commits HTTP rules with that
generation. Clear invalidates the generation. `invalid_rules`,
`companion_unsupported` and `companion_not_sent`
(`scenario::companion::left_previous_rules`) leave the companion's previous
rules answering: the change fails, the previous scenario stays whole and
admission reopens (`session.network.scenario.resume` with the generation; in
test runs without revoking the context). Any other failure — an in-flight
disconnect (`connection_lost`), a timeout, a generic `unavailable` — may follow
an applied request and keeps admission closed. `lxdev mock clear` asks the
companion first, then always clears the host, reopening admission; a companion
that did not confirm makes the clear partial (`partial_clear`, nonzero exit,
per-phase results). CLI mutations are serialized per session across local CLI
processes. Host-run scenario changes, and attempt reclamation, serialize per
run on its scenario lock and revoke the context on an unknown outcome;
reclamation reopens only the admission it closed itself. Clearing the rules of
a scenario that is being removed still needs an acknowledgement:
`companion_not_sent` there is a failure. An absent companion is
`companion_unsupported`, for which clearing is a no-op.
There is one product scenario per run, not one Function owner
per app; replacing its HTTP target replaces the previous app's rules too.
This covers new Logic fetch/SSE calls; it cannot undo requests already sent or
coordinate unrelated clients talking directly to the companion.

`lxdev mock reset` requires every participating phase to succeed. A refused
handler reset returns a nonzero exit code and per-phase results; absence of the
mock capability is reported as not applicable, not a successful reset.
