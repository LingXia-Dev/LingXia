# Mocks: who answers the app's calls

The app calls `fetch` (or a Worker Function). In development, for each call:

```text
test route  >  scenario rule  >  selection → mock handler (mocks/) | real backend
```

The first layer that answers wins. Product code never branches on mocks and
never imports them; release builds contain none.

## Where to set it

| Where | Scope |
|---|---|
| `mocks/config.json` | the project default |
| `lingxia dev --mock all\|none` | this session, from its first request; also CI |
| `lxdev mock all\|none [TARGET…]` | live, temporary; `lxdev mock reset` drops it |

With none of them, everything is real. Specs see the first two (live changes
stand aside during a run) and add scenario states with
[`t.app.mock.use`](#in-specs).

## `mocks/index.ts`: the handlers

The complete set of mock handlers, one per call the app makes, kept for good
so any call can switch to a mock. Split it freely with relative imports inside
`mocks/`.

```ts
import type { Mocks } from '@lingxia/types/mocks';
import { DEVICES, ME } from './fixtures';

let signedIn = true; // module state

export default {
  'POST **/auth/sessions': async (req) => {
    const { email } = await req.json<{ email: string }>();
    if (!email.includes('@')) return { status: 401, json: { reason: 'Check your email.' }, delay: 600 };
    signedIn = true;
    return { json: ME };
  },
  'DELETE **/auth/sessions/current': () => { signedIn = false; return { status: 204 }; },
  'GET **/locations/*/clients': () => (signedIn ? { json: DEVICES } : { status: 401 }),
  'GET **/legal-documents': { json: [] },
  'GET **/ai/runs/*/events': { sse: [{ data: { state: 'done' } }, { drop: true }] },
  'GET **/qoe/summary': { continue: true }, // real even under `all`
} satisfies Mocks;
```

- **Keys** are HTTP targets, `'METHOD url-glob'` (`*` for any method; the
  glob covers the whole URL, `**` any text, `*` no `/`; or `/regex/flags`),
  written out in the default export: no spreads or computed keys. The first
  key that matches answers.
- **Values** are an answer or a handler `(req, ctx) => answer`. An answer is
  a [scenario](#scenarios) `http` answer without `sequence`, `times`,
  `match` or `bodyBase64`: code handles those. `req` is
  `{ method, url: URL, headers, text(), json() }`; `ctx.fetch` is the
  original `fetch`, so a handler can proxy without being intercepted again.
- **Reload.** A dev session follows `mocks/` live: saves anywhere under it,
  a `mocks/` created after the start, and deleting it (the app then has no
  mocks) — no restart.
- **State.** Module state lives until a save under `mocks/`,
  `lxdev mock reset`, an app restart, or the next spec.
- **Errors fail the call, loudly**: a handler that throws, returns
  `undefined`, or returns an invalid answer rejects the `fetch` with
  `TypeError: fetch failed` (`error.data.detail` says which handler and why).
  A call the selection sends to mocks with no matching key fails the same
  way: `no mock handler for GET https://…/x; add 'GET **/x' to mocks/index.ts,
  or run: lxdev mock none 'GET **/x'`. Nothing falls through to real.
- **Hot reload.** Saving under `mocks/` reloads the handlers without
  rebuilding or restarting the app; an invalid save keeps the last valid ones.
- **Never shipped.** Any build fails when product code imports `mocks/`:
  `pages/home/index.ts imports from mocks/ (mocks/fixtures.ts)`.
- Add `"mocks/**/*.ts"` to `tsconfig.logic.json` (`lingxia new` does).

## `mocks/config.json`: the project default

```json
{ "$schema": "../node_modules/@lingxia/test/schemas/mock-config.schema.json",
  "mock": "all", "overrides": ["GET **/qoe/*"] }
```

- `all`: mocks answer, `overrides` go real. `none` (the default): real
  answers, `overrides` go to mocks (each must be a key of `mocks/index.ts`).
- Only HTTP targets; a Worker project selects its Functions itself.
- **A whole value replaces the layers below entirely**: `--mock none` means
  everything real, whatever the config says, overrides included; a live
  `lxdev mock all` (no targets) likewise. A live change with targets changes
  only those targets, on top of the rest.

## Commands

```bash
lxdev mock                                # who answers now, and from which layer
lxdev mock all                            # everything from mocks
lxdev mock none 'GET **/qoe/*'            # only this target real
lxdev mock all orders.submit              # a Worker Function (the companion switches it)
lxdev mock use checkout:unknown [--watch] # a scenario state on top
lxdev mock clear                          # drop the scenario (the selection stays)
lxdev mock list                           # scenarios and their variants
lxdev mock reset                          # project defaults, no scenario, fresh state
```

Every change applies to later calls: pages already loaded keep their data,
and `lxdev lxapp restart` reloads them. `lingxia dev` and `lxdev mock` print
the effective selection and its source:

```text
mock: none — from lingxia dev --mock (mocks/config.json: all, 2 real overrides — not in effect)
mock: all — live (lxdev mock all) over lingxia dev --mock none; lxdev mock reset to return
functions: 1/1 mocked — live (lxdev mock all)
```

`lxdev network status` names who answered each call:

```text
GET https://api.example.com/cart → 200  answered by: rule 1 (checkout:unknown)
GET https://api.example.com/locations/l1/clients → 200  answered by: mock (GET **/locations/*/clients) · config
GET https://api.example.com/qoe/summary → 200  answered by: real · live target
```

## In specs

```ts
import checkout from '../scenarios/checkout.json';

const scenario = await t.app.mock.use(checkout, 'unknown');
// { name, variant, rules, calls(filter?), waitForCall(target), remove() }
```

- A spec answers from the session's selection (`mocks/config.json`, over it
  `lingxia dev --mock`). To run a whole suite against mocks:
  `lingxia dev --background --mock all`, then `lxdev test`.
- Each spec starts with fresh handler state and no scenario; a second `use`
  replaces the first; the spec's end removes it.
- With `lxdev test --openapi`, a handler's answer is checked like a routed
  one: off the contract, it fails the spec with `E_OPENAPI_CONTRACT`.
- More: [Scenarios in specs](./testing.md#scenarios-in-specs).

## Scenarios

A scenario puts the app into a named product state — "gateway offline",
"payment result unknown" — on top of the selection. One file per business
scenario under `tests/scenarios/`; its name is the path without `.json`.

```json
{
  "$schema": "../../node_modules/@lingxia/test/schemas/scenario.schema.json",
  "name": "Checkout",
  "rules": [
    { "http": "GET **/cart", "json": { "items": [{ "sku": "A1", "qty": 2 }] } },
    { "http": "PATCH **/devices/*", "match": { "json": { "name": "Office" } },
      "status": 409, "json": { "error": "conflict" } },
    { "function": "coupons.apply", "match": { "args": { "code": "SPRING" } },
      "error": { "code": "COUPON_EXPIRED" } }
  ],
  "variants": {
    "paid":    { "rules": [{ "function": "orders.status", "result": { "state": "paid", "paidAt": "{{now-1m}}" } }] },
    "unknown": { "rules": [{ "function": "orders.submit", "fault": "unknown" },
                           { "function": "orders.status", "sequence": [
                               { "result": { "state": "pending" } }, { "result": { "state": "paid" } } ] }] },
    "offline": { "rules": [{ "http": "* **/cart", "abort": "failed" }] }
  }
}
```

Rules are tried in file order (a variant's first); the first that matches
answers, and a call none matches goes to the selection.

| | `http` rule | `function` rule |
|---|---|---|
| Target | `"http": "METHOD url-glob"` | `"function": "orders.submit"` |
| Narrow | `match.json`: the request's JSON body | `match.args`: the call's arguments |
| Answer | `status`, `statusText`, `headers`, `json` \| `body` \| `bodyBase64`, `contentType`, `delay`; or `abort: "failed"`, `hang: true`, `sse: [...]`, `continue: true` (the next layer answers; with `patchJson`, patched) | `result`, `error: { code, … }`, or `fault: "notRun"` / `"unknown"`; `delay` |
| Both | `sequence: [answer, …]` (call order, the last repeats), `times: n`, `note`, `{{now}}` / `{{now-2h}}` / `{{nowMs}}` in strings | |

- `match` is a deep subset: listed keys must match, arrays match element by
  element, a `"/regex/flags"` string matches a scalar's text.
- Unknown fields are errors, named by path (`variants.paid.rules[0]: …`).
  `use` validates the whole file first; an invalid file leaves the active
  scenario answering.
- `function` rules and Function targets need the dev session's companion to
  support them; otherwise nothing of the scenario is installed and the
  message says why.
- Record a starting point from real traffic: `lxdev network record start`,
  use the app, `lxdev network record stop --out tests/scenarios/x.json`.
