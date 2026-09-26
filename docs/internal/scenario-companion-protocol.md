# Scenario `function` rules and the dev companion

A scenario file (`lingxia_control_protocol::scenario`) mixes `http` rules,
which the app host answers, and `function` rules, which mock Worker Function
calls. LingXia does not know Worker details — it never parses Function
Definitions or a Worker project's mock configuration — so `function` rules
are forwarded opaquely to the dev session's **companion**, the participant
`lingxia dev` starts from `.lingxia/dev-companion.json` (see
`tools/lingxia-cli/src/commands/dev/companion.rs`). This note is the contract
a companion implements.

## Capability

A companion that answers `function` rules lists `scenario.function`
(`dev_session::capabilities::SCENARIO_FUNCTION`) in its `hello`
`capabilities`, or in its `session.prepare` result's `capabilities` (the two
are merged). Without it nothing below is sent: every scenario that has a
`function` rule is refused as a whole — by `lxdev scenario use` and by
`t.app.scenario()` — before anything is installed, with a message that names
the rules and the reason (no companion, or a companion without the
capability).

## Transport

After `session.prepare` the dev server writes `request` frames to the
companion's stdin and reads `response` frames (same `id`) from its stdout,
interleaved with its `event_batch` frames. Frames are the dev-session wire
(`DevSessionMessage`), one JSON object per line. The server waits up to 15 s
for an answer; a late answer is dropped.

Clients never talk to the companion directly. They send
`session.companion.<method>` to the dev server, which strips the prefix and
forwards the four `scenario.*` methods below (anything else is
`unknown_method`):

- `lxdev scenario use|status|clear` and `lxdev network status` send them over
  their client websocket.
- A host test run (`t.app.scenario()`) sends them up the runtime's dev
  bridge (`lingxia-control-runtime/src/bridge_upstream.rs`); the dev server
  answers on the same connection. A host without a dev bridge refuses
  `function` rules.
- `session.companion.capabilities` is answered by the server itself:
  `{ companion: bool, capabilities: [..] }`.

A missing companion or capability answers `companion_unsupported`; a
companion error is passed through unchanged (code, message, data).

## Methods

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

`{ "since": <epoch ms>, "owner"?: "test:…" }` →

```json
{ "calls": [
  { "time": 1727260000000, "function": "orders.submit", "args": { "cart": "c1" },
    "owner": "test:5b0c…", "rule": 0, "outcome": "fault" },
  { "time": 1727260000200, "function": "orders.status", "outcome": "default",
    "noMatch": "rule 1 match.args.id: missing" } ] }
```

Every Function call since `since` (all owners when `owner` is absent), oldest
first, bounded by the companion. `owner`/`rule` name the rule that answered;
both are absent when the project's own handler did (`outcome: "default"`).
`outcome` is `result`, `error`, `fault` or `default`. `noMatch` explains a
call whose Function had rules but none matched, in the form LingXia uses for
HTTP (`rule <position> <path>: <reason>`).

## Rule semantics

Exactly as for `http` rules (see `docs/skill/lxapp/scenarios.md`):

- Rules are tried in order; the first whose `function` equals the call's
  name and whose `match.args` matches answers. None: the Worker project's
  configured handler (real or mock — the project's choice, not LingXia's).
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

## Owners

- `dev`: installed by `lxdev scenario use`. The dev server clears it when the
  runtime connection drops (a dev scenario fails closed, like its HTTP half),
  and `lxdev scenario use` of a file without `function` rules clears it.
- `test:<run id>`: installed by `t.app.scenario()` in that run; one per run,
  replaced by the next `t.app.scenario()` and emptied (`scenario.use` with no
  rules) at the spec's end. When a run starts while `dev` has rules, the dev
  server installs the run's owner empty right away. It clears the owner when
  it sees the run reach a terminal state.
- A test owner sits above `dev`, even when empty: while one exists, `dev`
  rules stand aside (only the test owner's rules answer; a call they do not
  match goes to the default handler), and `dev` answers again once the test
  owner is cleared. So a dev scenario never steers a test, and `lxdev test`
  never refuses to start because of one.
- The companion keeps overlays in memory and never edits the project's files.
- When the session ends the companion process ends with it.
