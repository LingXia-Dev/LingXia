# Design note: a Worker section for `lxdev scenario`

Status: not implemented. `lxdev scenario use` rejects a `worker` section
today ("not supported yet"), and so do `parse_scenario` and
`t.app.network.scenario()`. This note fixes the contract a provider must meet
so the section can land without changing the file format or the CLI.

## Why a second section

An `http` section fakes what the app's Logic sees on the wire. Many product
states are cheaper and more faithful to set up on the backend the app talks
to: a local Worker dev server whose mock handlers hold state ("the gateway is
offline", "the subscription expired") and can inject faults (latency, 5xx,
dropped streams). A scenario that names that state puts app and backend in
the same state with one `use`.

## File shape

```jsonc
{
  "name": "Status card · offline",
  "http":   { "routes": [ … ] },
  "worker": {
    "target": "api",                     // which dev Worker, when there are several
    "state":  { "gateway": "offline" },  // opaque to lxdev, validated by the Worker
    "faults": [ { "match": "GET /v1/status", "status": 503, "times": 2 } ]
  }
}
```

lxdev does not interpret `state` or `faults`; the Worker validates them and
answers with errors that name the offending path, the way `parse_scenario`
names `http.routes[i]`.

## Provider contract

A `WorkerProvider` implements `SectionProvider` in
`tools/lingxia-devtools-cli/src/scenario.rs`:

- `install(section)` sends the section to the Worker dev runtime's control
  endpoint and returns its status. It must be all-or-nothing on the Worker
  side: on an error no part of the section stays applied.
- `clear()` restores the Worker's default mock state; it succeeds when
  nothing is installed. `lxdev scenario use` of a file without a `worker`
  section calls it, so a previous Worker state never outlives its scenario.
- `status()` returns `{ active, name?, source?, installedAt?, lastCleared? }`
  like the HTTP section, so `lxdev scenario status` can print both.
- `suspendable()` is `false`: backend state is shared with anything else
  talking to the Worker and cannot stand aside for one client. While it is
  active, `lxdev test` refuses to start with `scenario_active`
  (`refuse_test_while_blocking`) and tells the user to
  `lxdev scenario clear`. Specs that need Worker state set it themselves
  through a test API, not through a dev scenario.

`SECTIONS` already orders `worker` before `http`: the Worker is the slower,
less reversible side, so it goes first, and an `http` failure rolls it back
with `clear()`.

## Discovery and lifetime

- How lxdev finds the Worker's control endpoint (a field in the dev session
  registration, or a project setting) is decided with the Worker runtime; it
  must not require a running `lingxia dev` to be restarted.
- Lifetime follows the dev session: when the session ends, the Worker state
  should be cleared as well. The Worker cannot see the bridge disconnect, so
  either lxdev registers the state with a lease the Worker expires, or
  `lingxia dev` clears it on exit. Until one of those exists, `status` must
  say that a Worker state may outlive the session.

## Test API

`t.app.network.scenario()` stays HTTP-only. A spec-scoped
`t.app.scenario(file)` that installs every section (and rolls back at spec
end) can follow once the Worker runtime offers per-run isolation; without
it, one run's backend state would leak into another's.
