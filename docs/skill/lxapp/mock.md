# Scenarios: product states from a file

A scenario puts the app into a named product state — "gateway offline",
"coupon expired", "payment result unknown" — whether the data comes from an
HTTP API or a Worker Function. The same file serves a dev session
(`lxdev scenario use`, for UX review, demos and manual checks) and specs
(`t.app.scenario()`, see [Product testing](./testing.md#scenarios-in-specs)).
Scenarios exist only in dev sessions and test runs; release builds cannot
load them.

## Writing one

One file per business scenario under `tests/scenarios/`; its name is the path
without `.json` (`tests/scenarios/checkout/payment.json` → `checkout/payment`).

```json
{
  "$schema": "../../node_modules/@lingxia/test/schemas/scenario.schema.json",
  "name": "Checkout",
  "description": "The cart and the payment result",
  "rules": [
    { "http": "GET **/cart", "json": { "items": [{ "sku": "A1", "qty": 2 }] } },
    { "http": "PATCH **/devices/*", "match": { "json": { "name": "Office" } },
      "status": 409, "json": { "error": "conflict" } },
    { "function": "coupons.apply", "match": { "args": { "code": "SPRING" } },
      "error": { "code": "COUPON_EXPIRED" } }
  ],
  "variants": {
    "paid":    { "rules": [{ "function": "orders.status", "result": { "state": "paid", "paidAt": "{{now-1m}}" } }] },
    "unknown": { "description": "The submit answer is lost",
                 "rules": [{ "function": "orders.submit", "fault": "unknown" },
                           { "function": "orders.status", "sequence": [
                               { "result": { "state": "pending" } },
                               { "result": { "state": "paid" } } ] }] },
    "offline": { "rules": [{ "http": "* **/cart", "abort": "failed" }] }
  }
}
```

**Rules** are tried in file order; the first one that matches a call answers
it. A call no rule matches goes to the real backend (for a Function: the
Worker project's configured handler). A variant's rules go before the shared
ones.

| | `http` rule | `function` rule |
|---|---|---|
| Target | `"http": "METHOD url"` — `GET **/wifi/main`, `* **/wifi/*` (any method); the URL is a glob over the whole URL (`**` any text, `*` no `/`) or `/regex/flags` | `"function": "orders.submit"` (one name, no globs) |
| Narrow | `match.json`: the request's JSON body | `match.args`: the call's arguments |
| Answer | `status`, `statusText`, `headers`, `json` \| `body` \| `bodyBase64`, `contentType`, `delay`; or `abort: "failed"`, `hang: true`, `sse: [...]`, `continue: true` (+ `patchJson`) — the [`route()` handler](./testing.md#faking-the-network) shape | `result`, `error: { code, … }` (a declared business error), or `fault: "notRun"` (failed before it ran) / `"unknown"` (outcome lost); `delay` |
| Both | `sequence: [answer, …]` (call order, the last repeats), `times: n` (then stand aside), `note`, `{{now}}` / `{{now-2h}}` / `{{now+30m}}` / `{{nowMs}}` in strings | |

**`match` is a deep subset.** An object matches when each listed key is present
and matches (other keys are ignored); an array matches element by element and
must have the same length; a string written `"/regex/flags"` (flags among
`imsu`) matches a scalar whose text it finds; anything else must be equal.
Query strings belong in the URL glob; headers cannot be matched.

- Unknown fields anywhere are errors, named by path (`variants.paid.rules[0]:
  …`); `$schema` gives editors completion.
- `function` rules need the dev session's companion (the Worker runtime
  `lingxia dev` starts) to support them. When it does not, any scenario with
  a `function` rule is refused as a whole — nothing is installed — and the
  message names the rules and why.
- Record a starting point from real HTTP traffic:
  `lxdev network record start`, use the app, then
  `lxdev network record stop --out tests/scenarios/status/online.json --name "Status · online"`.
  Review it before committing.

## Using one

```bash
lxdev scenario list                     # each usable name and name:variant
lxdev scenario use checkout:unknown     # a name[:variant], or a file[:variant]
lxdev scenario use checkout:paid --watch  # reinstall on every save
lxdev scenario status                   # what is active, what each rule answered
lxdev scenario clear
lxdev network status                    # recent calls and who answered them
```

- `use` validates the whole file first; an invalid file (or a refused
  `function` rule) leaves the active scenario answering. It replaces the
  active scenario as a whole.
- Switch states mid-flow with another `use`: `use checkout:paid` after
  `use checkout:unknown` makes the next call answer from `paid`.
- `--watch` keeps running: every save reinstalls; an invalid save is printed
  and the last valid version keeps answering. Ctrl-C stops watching, not the
  scenario.
- `status` prints `rule 1 GET **/cart answered 2×` per rule. A request that
  hits rule targets but no `match` is logged as `no rule matched PATCH …
  (1 rule for this target: rule 2 match.json.name: expected "Office", got
  "Den")`.
- **The app's own caches.** A page that serves cached data sends no request,
  so a new scenario seems to do nothing. `use` and `status` say so when no
  request reached the scenario: reload the page or `lxdev lxapp restart`.
- Names resolve under `tests/scenarios/` of the session's content directory,
  then its project root, then the lxapp project you run `lxdev` from.
  `--appid` picks the lxapp (default: the home lxapp, else the current one).
- A scenario lasts until `clear`, another `use`, or the end of the dev
  session; a Runner restart or a dropped dev connection clears it, and
  `status` says why. Installing, clearing and every answered request write a
  warning to the session log (`lxdev logs`).
- During `lxdev test` the dev scenario stands aside and a spec's own
  `t.app.scenario()` answers; the dev scenario answers again when the run
  ends.
