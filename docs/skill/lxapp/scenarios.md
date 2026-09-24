# Scenarios: product states in a dev session

A scenario puts the running app into a named product state — "gateway
offline", "stale data", "empty inbox" — without a test running, for UX review,
demos and manual checks. The same file serves specs through
`t.app.network.scenario()` (see [Product testing](./testing.md#scenario-files)).

## Writing one

Keep one file per state under `tests/scenarios/`; its name is the path
without `.json` (`tests/scenarios/status/offline.json` → `status/offline`).

```json
{
  "$schema": "../../node_modules/@lingxia/test/schemas/scenario.schema.json",
  "name": "Status card · offline",
  "description": "The status endpoint fails; the device list shows a stale lastSeen.",
  "http": {
    "routes": [
      { "url": "**/v1/status", "status": 503, "json": { "up": false } },
      { "url": "**/v1/devices/*", "json": { "id": "d1", "online": false, "lastSeen": "{{now-6h}}" } }
    ]
  }
}
```

- `http.routes` answer the app's Logic `fetch` and `Rong.SSE`; each route is
  the scenario route of [Product testing](./testing.md#scenario-files)
  (`url`, `method?`, `times?`, an answer or a `sequence`, `{{now-2h}}`
  templates). A top-level `routes` array means the same, so recordings and
  older files work unchanged.
- `name` and `description` show in `lxdev scenario list` and `status`.
- `$schema` points editors at the schema `@lingxia/test` ships
  (`schemas/scenario.schema.json`); unknown fields are errors.
- A `worker` section is reserved for Worker-backed state and is rejected as
  not supported yet.
- Record a starting point from real traffic:
  `lxdev network record start`, use the app, then
  `lxdev network record stop --out tests/scenarios/status/online.json --name "Status card · online"`.
  Review it before committing.

## Using one

```bash
lxdev scenario list                   # names, sections, name/description
lxdev scenario use status/offline     # a name, or a path to a file
lxdev scenario status                 # what is active, what it answered, the last one cleared
lxdev scenario clear
```

- Names resolve under `tests/scenarios/` of the session's content directory,
  then its project root, then the lxapp project you run `lxdev` from. A path
  to a file works anywhere.
- `use` targets the home lxapp (else the current one); `--appid` picks
  another. It replaces the active scenario as a whole; if a section fails,
  nothing of the new scenario stays installed.
- It lasts until `clear`, another `use`, or the end of the dev session. It
  survives an app relaunch; a Runner restart or a dropped dev connection
  clears it, and `status` then says why and when.
- It is loud on purpose: installing, clearing and every answered request
  write a warning to the session log (`lxdev logs`).
- It never changes a test's outcome: HTTP routes stand aside while an
  `lxdev test` run is active. A future section that cannot stand aside makes
  `lxdev test` refuse to start (`scenario_active`) until you
  `lxdev scenario clear`.
- Only development hosts and the Runner can do this; a release build has no
  routing and the command says so.
