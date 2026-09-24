# Product testing with `lxdev test`

`lxdev test` runs `@lingxia/test` specs inside the target App/Runner, on a test
JS worker separate from lxapp Logic and WebViews. Specs drive the app through
`t.app` and read its Logic state through typed helpers.

## First spec

```ts
// tests/pages/notes.test.ts
import { spec, expect } from '@lingxia/test';

interface NotesData { notes: { id: string; title: string }[] }

spec('saving a note lists it', async (t) => {
  await t.app.nav.relaunch({ page: 'notes' });
  await t.app.page.testId('note-title').fill('Groceries');
  await t.app.page.testId('note-save').click();
  await t.expect(t.app.page.testId('note-saved')).toBeVisible();

  const data = await t.app.pageData<NotesData>();
  expect(data.notes.map((note) => note.title)).toContain('Groceries');
});
```

```bash
lingxia dev --background
lxdev test tests/pages/notes.test.ts
```

- `testId('x')` matches `[data-testid="x"]` in the current page's View. It must
  match exactly one element; narrow duplicates with `.nth(i)`.
- `t.app.nav.relaunch({ page })` clears the stack and opens a configured page
  (lxapp.json name), resolving after its `onReady`. `spec(title, { fresh: true }, body)`
  relaunches the home page instead. Neither resets app or backend data.
- Actions and `t.expect(...)` retry for 5 s by default (`{ timeout }` per call);
  a spec has 30 s (`spec(title, { timeout }, body)`). Await every action and assertion.
- `t.expect(locator)` / `t.expect.poll(read)` retry; imported `expect(value)` checks once.
- Setup: install `@lingxia/test` matching the project's LingXia line, and keep
  a separate test tsconfig with `lib: ["ES2020"]` and
  `types: ["@lingxia/types/automation-test-globals"]`.

## Common tasks

| Task | API |
|---|---|
| Start on a known page | `t.app.nav.relaunch({ page, query? })`; `{ fresh: true }` for home |
| Navigate | `t.app.nav.to` / `.redirect` / `.switchTab` / `.back` |
| Find an element | `t.app.page.testId(id)`, `.css(selector)`, `.nth(i)` / `.first()` / `.last()`, `.filter({ hasText })`, `{ page }` option |
| Act | `locator.click()` / `.fill(text)` / `.type(text)` / `.press(key)`; `{ force: true }` on click/fill, see [gotchas](#gotchas) |
| Assert UI | `t.expect(locator).toBeVisible()` / `.toBeInViewport()` / `.toBeAttached()` / `.toHaveText()` / `.toContainText()` / `.toHaveAttribute(name, value?)` / `.toHaveCount()` / `.toHaveValue()` / `.toBeEnabled()`; `.not` |
| Wait for an element state | `locator.waitFor({ state: 'visible' \| 'inViewport' \| 'attached' \| 'hidden' \| 'detached' })` |
| Read page Logic `data` | `t.app.pageData<T>({ page? })`; see [below](#reading-app-logic) |
| Call a page method | `t.app.callPage<R>(method, ...args)` |
| Run code in Logic / page DOM | `t.app.eval(fn, ...args)` / `t.app.page.eval(fn, ...args)` |
| Wait until a value is ready | `t.waitFor(read, accept?)` returns it; `t.expect.poll(read).toBe(x)` asserts it |
| Expect a rejection | `await t.reject(() => op(), { code?, message? })` |
| Fake Logic `fetch` responses | `t.app.network.route(pattern, handler)`; see [below](#faking-the-network) |
| Replay a set of fake responses from a file | `t.app.network.scenario(json)`; see [scenario files](#scenario-files) |
| Put the app into a product state by hand | `lxdev scenario use <name>`; see [Scenarios](./scenarios.md) |
| Name a set of `lxdev test` flags | `lxdev.json` presets, `lxdev test --preset ci`; see [presets](#presets) |
| Check responses against an API contract | `lxdev test --openapi api.yaml`, `expect(value).toMatchSchema('Name')`; see [below](#api-contract-checks) |
| Tag specs, run a layer | `spec(title, { tags }, body)`, `spec.configure({ tags })`; `lxdev test --tag`; see [below](#tags-and-layers) |
| Fast-forward Logic timers and `Date` | `t.app.clock.install()` / `.tick(ms)`; see [below](#test-clock) |
| Inputs and secrets | `--arg k=v` / `--secret-arg k=v`, read with `t.arg('k')` |
| Cleanup | `t.defer(fn)` (LIFO, runs on success or failure); `spec.afterEach` |
| Restore state before each attempt | `spec.reset(async (t) => { ... })` (required by `--retries`) |
| Skip at run time | `t.skip(reason)`; at registration `spec.skip` / `spec.fixme` |
| Known failure | `spec.fail(title, { expected: { code, message } }, body)` |
| Group trace, add evidence | `t.step(name, fn)`, `t.attach(name, data)` |
| Another running lxapp | `t.apps.lxapp(appId)` |
| App lifecycle and inbound links | `t.automation.lxapps` |
| External web pages, auth/payment callback tabs | `t.automation.browser` |
| Runner presets/appearance | `t.automation.device`; [adaptive testing](adaptive-ui.md#test-runtime-switching) |
| Host shell, terminal, or local OS integration | `t.automation.shell`, `.terminal`, `.desktop` where supported |
| External HTTP fixtures, callback collectors, service results | Test-context `fetch` |

The automation root is typed by `@lingxia/types/automation`; platform support
and selectors follow [lxdev](../cli/lxdev.md). In a test program
`lx.automation()` is `HostRunAutomation`; app Logic gets the narrower
`Automation`, which has no `network`, nav `waitUntil: 'ready'`, or eval call
tracing. Browser automation targets host browser tabs; desktop automation
requires a macOS/Windows host built with it, such as the Runner. `t.app.surfaceLayout()` reads the host render plan. Restore any existing
shell pins or device settings changed by a test.

## Context and assertions

The test context has `lx.automation()`, `console`, timers, and `fetch`, but no
app `lx.*`, DOM, filesystem, Node built-ins, or dynamic `import()`. Importing
product source does not run it in the app: use the eval helpers below.

Trigger the behavior under test through UI actions; setup, eval, and backend
calls do not replace that product path. Use `t.app`/`t.automation`, not raw
`lx.automation()`, which bypasses tracing and fixture guards.

## Reading app Logic

```ts
interface DevicesData { devices: { id: string; name: string }[] }

const { devices } = await t.waitFor(
  () => t.app.pageData<DevicesData>(),
  (data) => data.devices.length > 0,
);
await t.app.callPage('refresh');
const path = await t.app.eval(({ lx }) => lx.env.USER_DATA_PATH);
const route = await t.app.eval(({ getCurrentPages }, index) => getCurrentPages()[index].route, 0);
const title = await t.app.page.eval(({ document }) => (document as { title: string }).title);
```

- `t.app.eval(fn, ...args)` runs `fn` in the app's Logic with `{ lx, getApp,
  getCurrentPages }`; `t.app.page.eval(fn, ...args)` runs it in the page
  WebView with `{ document, window }` (`unknown` without the DOM lib: cast).
- `T`/`R` in `pageData<T>` / `callPage<R>` are declared, not validated.
- `t.waitFor` fails at once on `TypeError`, `ReferenceError`, and
  `SyntaxError` (override with `retryIf`); other thrown errors retry.
- Prefer these over the string form `t.app.eval({ script })`.

## Faking the network

`t.app.network` answers the app's Logic `fetch` and `Rong.SSE` requests, so
specs reach error, loading and streaming states through the real data layer.
Do not add test hooks to product fetch wrappers. Only Logic requests are
routed, not WebView ones, and a host outside the app's
[network grants](../native/permissions.md) is never faked.

### Routes

```ts
spec('rename shows the not-implemented error', async (t) => {
  const patch = await t.app.network.route(
    { url: '**/v1/devices/*', method: 'PATCH', times: 1 },
    { status: 501, json: { error: 'not_implemented' }, delay: 300 },
  );
  await t.app.network.route('**/v1/clients', { abort: 'failed' });
  await t.app.page.testId('rename-input').fill('Office');
  await t.app.page.testId('rename-save').click();
  await t.expect(t.app.page.testId('rename-error')).toBeVisible();
  const [sent] = await patch.requests();
  expect(JSON.parse(sent.body ?? '{}')).toEqual({ name: 'Office' });
});
```

- Pattern: a glob over the whole URL (`**` any, `*` no `/`, `{a,b}`; `?` is
  literal), a `RegExp` without lookaround or backreferences, or
  `{ url, method?, times? }`. The newest matching route wins.
- Handler, exactly one of: fulfill `{ status?, statusText?, headers?,
  contentType?, delay? }` plus `body` (sent verbatim) or `json`; `delay` (max
  30000 ms) shows loading states. `{ abort: 'failed' }` rejects like a
  transport failure (`TypeError: fetch failed`). `{ continue: true }` passes
  through; add `patchJson` to edit the real response instead of faking it
  (RFC 7396 merge patch: keys merge, `null` deletes, arrays replace).
  `{ hang: true }` never answers until the spec ends, for spinners and
  client-side timeouts; an `AbortSignal` still cancels it.

  ```ts
  await t.app.network.route('**/v1/devices', { continue: true, patchJson: { items: [], total: 0 } });
  await t.app.network.route('**/v1/status', { hang: true });
  ```
- `route()` returns `{ id, pattern, unroute(), requests() }`; requests report
  `{ method, url, headers, body, bodyTruncated, action, status }` (`body` cut
  to 64 KiB, `null` for streams, `Blob`, `FormData`).
- Routes last one spec and work only in a `lxdev test` run;
  `t.app.network.requests()` lists only this spec's routes.

### Sequences and relative times

`{ sequence: [a, b, c] }` answers matched requests in call order; the last
answer repeats. Use it for "fails twice, then recovers":

```ts
await t.app.network.route('**/v1/status', {
  sequence: [{ status: 503 }, { status: 503 }, { json: { up: true } }],
});
```

String values of an answer may carry relative times, rendered each time it is
served: `{{now}}`, `{{now-2h}}`, `{{now+30m}}` (ISO-8601 UTC, like
`toISOString()`; units `ms`, `s`, `m`, `h`, `d`) and `{{nowMs}}` (epoch
milliseconds, as a string). Other `{{…}}` text is left alone. They read real
time, not a [test clock](#test-clock): with a clock installed, compute dates
in the spec instead.

### Server-sent events

`{ sse: [...] }` answers with a `text/event-stream` (status 200). Items play in
order: events `{ event?, data, id?, retry? }` (`data` that is not a string is
sent as JSON), `{ comment }`, `{ delayMs }` (up to 30000), and `{ drop: true }`,
which closes the stream like a server dropping the connection and must be last.
Without `drop` the stream stays open after its last item until the spec ends.

```ts
const live = await t.app.network.route('**/v1/events', {
  sequence: [
    { sse: [{ event: 'status', data: { online: 3 }, id: 'e1' }, { delayMs: 500 }, { drop: true }] },
    { sse: [{ event: 'status', data: { online: 4 }, id: 'e2' }] },
  ],
});
await t.expect(t.app.page.testId('online-count')).toHaveText('4');
const [first, reconnect] = await live.requests();
expect(reconnect.headers['last-event-id']).toBe('e1');
```

- `Rong.SSE` in Logic receives the events as it would from a server: `retry`
  and `id` apply, and after a `drop` it reconnects with `Last-Event-ID` (the
  next answer of a `sequence` serves the reconnect). A request log entry is
  recorded for every connection attempt. An `sse` answer to Logic `fetch`
  delivers the same bytes as the response body.
- Consume event streams in Logic with `Rong.SSE`, not `fetch`: Logic `fetch`
  hands a streamed body over in large chunks, so `response.body.getReader()`
  does not see events as they arrive.

  ```ts
  const events = new (Rong as any).SSE(url, {
    headers: { Authorization: `Bearer ${token}` },
    reconnect: { baseDelayMs: 1000, maxDelayMs: 30000 },
  });
  for await (const { type, data, id } of events) { /* … */ }
  events.close();
  ```

### Scenario files

A scenario is a JSON file of routes, installed together. Keep one per product
state (`tests/scenarios/outage.json`) and share it between specs and manual
checks in a dev session.

```json
{
  "name": "status-outage",
  "description": "Status fails twice, then recovers",
  "routes": [
    { "url": "**/v1/status", "method": "GET",
      "sequence": [{ "status": 503 }, { "status": 503 }, { "json": { "up": true, "checkedAt": "{{now}}" } }] },
    { "url": "**/v1/devices/special", "status": 404, "note": "listed first, so it wins" },
    { "url": "/\\/v1\\/devices\\/\\w+$/", "json": { "id": "d1", "lastSeen": "{{now-2h}}" } },
    { "url": "**/v1/events", "sse": [{ "data": "hello" }] },
    { "url": "**/v1/offline", "abort": "failed" }
  ]
}
```

```ts
import outage from '../scenarios/outage.json';

spec('the status banner recovers', async (t) => {
  const scenario = await t.app.network.scenario(outage);
  await t.app.nav.relaunch({ page: 'home' });
  await t.expect(t.app.page.testId('status-ok')).toBeVisible();
  expect((await scenario.routes[0].requests()).length).toBe(3);
});
```

- Routes sit at the top level or under `http.routes` (the sectioned form
  `lxdev scenario` files use); `$schema`, `name` and `description` are
  optional. A `worker` section is rejected as not supported yet.
- Each route is `{ url, method?, times?, note? }` plus one answer in the
  `route()` handler shape, or a `sequence`. `url` is a glob or a regex written
  `"/source/flags"`. A file may also give a binary body as `bodyBase64`.
- The first matching route of a scenario answers; routes added later with
  `route()` still take precedence. Unknown fields and bad patterns reject with
  the route's index (`routes[2]: …`).
- `scenario()` returns `{ name, routes, unroute(), requests() }`; its routes
  are removed when the spec ends, like `route()`.
- Import JSON with `resolveJsonModule` in the test tsconfig.

### The same file in a dev session

`lxdev scenario use <name>` installs a scenario file into the running app
with no test running, for manual checks; it stands aside while `lxdev test`
runs. See [Scenarios](./scenarios.md).

### Recording real traffic

```bash
lxdev network record start --match '**/v1/**'
# … use the app …
lxdev network record stop --out tests/scenarios/recorded.json --name "Recorded"
lxdev test tests/ --record-network recorded/   # one scenario per spec: recorded/<spec id>.json
```

- A recording keeps each request's method, URL, status, content type and JSON
  or text body; repeated URLs become a `sequence` when the answers differ, and
  a transport failure becomes `abort`. Routed answers are not recorded.
- Credentials are never written: request headers are not recorded (so
  `Authorization` and `Cookie` never are), response headers other than the
  content type are dropped (so `Set-Cookie` is), JSON fields named like a
  credential (`token`, `access_token`, `refresh_token`, `id_token`,
  `api_key`/`apiKey`, `secret`, `client_secret`, `password`,
  `authorization`, `session`/`session_id`, `cookie`, in any case or
  separator style) become `***`, JSON Web Tokens and `Bearer <token>` values
  in any recorded text become `***`, token-like query values become `*`, and
  `--secret-arg` values (or `record stop --redact <value>`) become `***`.
- While a test run is active a dev-session recording pauses, so it never
  captures test traffic; `lxdev test --record-network` records on its own
  either way. `record stop --out` checks that it can write the file before
  it stops the recording; if the write still fails, the scenario is printed
  to stdout and the command fails.
- Under `--record-network`, specs whose ids map to the same file name get a
  numeric suffix (`<spec id>-2.json`) instead of overwriting each other.
- Event streams and binary bodies over 64 KiB are noted on the route
  (`note`) instead of recorded; `Rong.SSE` connections are not recorded.
  Review a recording before committing it.

### The network log of a failed spec

A failed spec lists the app's last 20 Logic network calls since it started, in
`failures[].network` of `report.json` and under the failure in `report.html`:
method, URL, status or error, duration, and whether a route or the real
network answered (with the route's pattern). No bodies or headers are kept;
token-like query values and `--secret-arg` values are masked.

## API contract checks

`--openapi` checks the app's Logic `fetch` responses against an OpenAPI 3.0 or
3.1 document (JSON or YAML, repeatable):

```bash
lxdev test tests/ --tag routed --openapi api/openapi.yaml
```

- A response a route fulfilled (or patched with `patchJson`) that breaks the
  documented schema for its operation and status fails the spec with
  `E_OPENAPI_CONTRACT`, naming the JSON path and what was expected. A stale
  route fixture cannot pass silently.
- A real server's response that breaks it is a warning: printed and listed in
  the report, never a failure.
- Operations match by method, server base path and path template; hosts are
  not compared. Requests no operation describes and statuses the operation
  does not document are counted in the report, not failed.
- Only JSON responses are validated (`application/json`, `text/json`,
  `*+json`; not `x-ndjson` or event streams). Bodies over 256 KiB are cut and
  skipped. `format` is not asserted. `$ref`s must be local: bundle a
  multi-file document first.
- Checking keeps only method, `scheme://host/path` (no query), status,
  content type and the JSON body of each response; never headers. The app
  still receives an equivalent response.

Assert a value directly with `toMatchSchema`:

```ts
const data = await t.app.pageData<{ devices: unknown[] }>();
expect(data.devices[0]).toMatchSchema('Device');                   // #/components/schemas/Device
expect(problem).toMatchSchema({ ref: '#/components/schemas/Problem', document: 'openapi.yaml' });
```

- Without `--openapi`, `toMatchSchema` fails saying so. A spec that only means
  something against the contract skips itself instead; `t.openapi` lists the
  loaded documents, or is `undefined`:

  ```ts
  spec.beforeEach((t) => {
    if (!t.openapi) t.skip('needs its OpenAPI document: run with --openapi api/openapi.yaml');
  });
  ```
- Declare a known contract break with
  `spec.fail(title, { expected: { code: 'E_OPENAPI_CONTRACT' } }, body)`.

## Selecting and covering specs

### Tags and layers

Tag specs by how much of the world they touch, and run each layer on its own:

```ts
// tests/pages/devices.test.ts
spec.configure({ tags: ['routed'] });            // every spec in this file

spec('lists devices', { tags: ['smoke'] }, async (t) => { /* … */ });  // routed + smoke
```

| Layer | Talks to | Typical run |
|---|---|---|
| `unit` | nothing: pure Logic helpers through `t.app.eval`, time through `t.app.clock` | every change |
| `routed` | the UI, with every backend call faked by routes or scenarios | every change and CI, with `--openapi` so fixtures cannot drift from the contract |
| `live` | a real backend or device | nightly, before release; `--openapi` turns server drift into warnings |

```bash
lxdev test tests/ --tag routed --openapi api/openapi.yaml   # CI
lxdev test tests/ --tag '!live'                # everything except live
lxdev test tests/ --tag unit,routed --tag smoke   # (unit or routed) and smoke
```

- One `--tag` is any of its comma-separated terms; `!tag` means "without
  tag"; several `--tag` flags must all hold.
- An untagged spec has no tags: `--tag routed` leaves it out, `--tag '!live'`
  keeps it. Tag every file (`spec.configure`) so no spec falls between layers.
- `--tag` combines with `--grep`, `--id`, `--last-failed` and `--shard`.
- Tags use letters, digits and `_ . : / -`. `spec.configure` applies to the
  file that calls it and adds to the spec's own `tags`.
- Reports show each spec's tags and a per-tag table (`tag_summary` in
  `report.json`, `tag:<name>` properties in `junit.xml`), so a failing `live`
  group does not hide a clean `routed` one; untagged specs appear as
  `(untagged)`.
- Record a `live` run with `--record-network` to seed `routed` scenarios.

### Coverage manifest

List the requirements a suite must cover and let the report say which have a
spec:

```yaml
# tests/coverage.yaml — a list of ids, or { id, title }
- { id: DEV-LIST, title: Device list }
- { id: DEV-RENAME, title: Rename a device }
```

```ts
spec('renames a device', { covers: ['DEV-RENAME'] }, async (t) => { /* … */ });
```

```bash
lxdev test tests/ --covers-manifest tests/coverage.yaml
```

`report.json` `coverage` and the HTML report list every id with the specs
that cover it and their outcome, the ids no spec covers, and `covers` ids
missing from the manifest (also printed as a warning). A spec outside the
selection still counts, as `not_run`, so a filtered run shows no false holes.
The manifest may also be JSON, a `covers:` list, or an `id: title` map.

## Test clock

`t.app.clock` puts the app's Logic on test time, so a 30-second poll, a
session expiry or a date-dependent screen is tested without waiting.

```ts
spec('status refreshes every 3 s', async (t) => {
  await t.app.network.route('**/v1/status', { json: { online: true } });
  await t.app.clock.install({ now: '2030-01-01T09:00:00Z' });
  await t.app.nav.relaunch({ page: 'status' });   // start the poll on test time
  const { fired } = await t.app.clock.tick(9_000);  // three polls, in order
  expect(fired).toBe(3);
  await t.expect(t.app.page.testId('status')).toHaveText('Online');
});
```

- While installed, Logic's `Date` (`new Date()`, `Date()`, `Date.now()`),
  `setTimeout` / `setInterval` / `clear*` and `performance.now()` read test
  time. Timers fire only from `tick(ms)` (each at its own time, in order) or
  `runAll()` (until none is left; rejects after `maxTimers`, default 1000, so
  use `tick` with a `setInterval`). `setSystemTime(t)` moves `Date` without
  firing anything. `tick`/`runAll` resolve `{ now, fired, pending }`.
- After each timer fires, promise chains it started settle before the next
  one: an async poll that awaits a routed `fetch` and `res.json()` re-arms
  within the same `tick`. Work waiting on real I/O does not settle inside a
  tick; wait for it with `t.waitFor` / `t.expect`, then tick again.
- Real time still runs for the page WebView, native work and timeouts, real
  network requests, route `delay` / `hang`, SSE `delayMs` and reconnect
  backoff, scenario `{{now}}` templates, `setData` delivery, and timers
  started before `install` — install before opening the page under test.
  Routes, recording, the network log and contract checks work the same with a
  clock installed.
- Only the selected app's Logic (`t.apps.lxapp(id).clock` for another), and
  only in a `lxdev test` run. A spec's clock is removed when it ends (pending
  test timers are dropped, and the next spec then starts from a relaunched
  home page); the run's end or the app reopening (a profile rollback) also
  returns it to real time. The runner's own timers and spec timeout are never
  faked.
- `install` twice rejects with `E_CLOCK_INSTALLED`; `tick` without a clock with
  `E_CLOCK_NOT_INSTALLED`. `uninstall()` resolves `{ uninstalled, dropped }`.

## Isolated app data

`--isolate` runs the suite on a throwaway copy of the app's data (storage,
`lx://userdata`, `lx://usercache`, `lx://temp`); the developer's own data is
untouched and back in place when the run ends, however it ends.

```bash
lxdev test tests/ --isolate                        # start empty
lxdev test tests/ --state auth --save-state auth   # reuse, refresh on pass
```

- `--state NAME|PATH` seeds from a snapshot; `--save-state NAME|PATH` saves
  after a passing run (`--save-state-on always` for any finished run). Both
  imply `--isolate`. A NAME lives under `~/.lingxia/test-state/`; keep PATH
  snapshots out of git (`*.lxstate`) — they can hold sign-in tokens.
- Sign in once: a setup spec signs in through the UI only when the app shows it
  is signed out; later runs start from the saved state.
- `spec(title, { restoreProfile: true }, fn)` rolls the app's data back after
  that spec (implies `fresh`); `t.profile.checkpoint()` / `restore(id)` /
  `drop(id)` do it by hand. Both reopen the app, so re-read `t.app` afterwards,
  and both reject with `E_PROFILE_NOT_ISOLATED` without `--isolate`.
- A reopen resolves once the app has settled: `App.onLaunch` has finished
  (its promise included), a page is ready, and the current page has not
  changed for 300 ms. A redirect the app makes at start-up therefore lands
  inside the rollback, never in the next spec. Start-up work still running
  5 s after the page is ready is left running.
- A rollback restores everything the app stored, sign-in tokens included. If
  the app rotates single-use refresh tokens, a rollback across a rotation hands
  it a spent token and the server may end the session. Keep the session with
  `keep`: the `lx.getStorage()` keys it matches keep their current state (a
  new key stays, a deleted one stays deleted) while everything else rolls back.

  ```ts
  spec('edits a device', { restoreProfile: { keep: ['auth.*'] } }, async (t) => { /* ... */ });
  await t.profile.restore(id, { keep: ['auth.*', 'session.token'] }); // resolves { kept }
  ```

  Globs match whole keys: `*` any run of characters (dots included), `?` one.
  Only storage keys are kept; `lx://userdata` files always roll back. The app
  is closed during the merge, so it never sees the rolled-back tokens.
- A snapshot belongs to one app, channel and device; another one is refused.
  Downloads (`destination: "downloads"`) and other lxapps stay shared.

## Gotchas

- **Visible means rendered, not in the viewport.** `toBeVisible` and
  `waitFor()` (default `visible`) pass for content below the fold, and
  `toBeHidden` fails for it; assert placement with `toBeInViewport()`. Actions
  scroll their target into view themselves.
- **`force` is for unreachable content.** When no scroll brings an element
  under the pointer (the lower part of an overflowing sheet), `click({ force:
  true })` / `fill(text, { force: true })` dispatch to it directly; it must
  still be one enabled match. It proves less than a normal click, so use it
  only where that fails with `element is obscured`.
- **Fixture nav waits for `onReady`** (`timeoutMs`, default 15000) and rejects
  if the app replaces the page first, e.g. with its own `lx.reLaunch`. Pass
  `waitUntil: 'commit'` to resolve once the stack changed, then assert the
  landing page.
- **Specs share app state.** A new spec does not reset the app, storage, or
  backend (only a timed-out spec forces a home relaunch). If a spec leaves the
  app under test closed (it crashed, or its Logic was torn down), the next spec
  reopens it on its home page first and the run reports a `recovery`
  diagnostic naming the spec that preceded it. Seed and clean up
  explicitly with `t.defer` or `spec.reset`, or roll back with
  `restoreProfile` in an isolated run.
- **Hooks are file-scoped.** `spec.reset`, `beforeEach`, and `afterEach` apply
  to specs in the file that registers them. A shared helper such as
  `installHooks()` registers into the spec file that calls it at top level.
- **Eval functions are self-contained.** `fn` is sent as source text: it cannot
  use spec variables, imports, or helpers. Pass values as extra arguments;
  arguments and the result must be JSON.
- **Specs run on the target device.** Test `fetch('http://127.0.0.1:...')`
  reaches the device's loopback, not the development machine. Start fixture
  servers from shell/CI and pass reachable URLs with `--arg`.
- **Secrets go through `--secret-arg`.** Its value is `***` in reports, events,
  and attachments. `--arg` keys named like credentials (`password`, `apiKey`)
  are masked only in the report's arg list.
- **`t.arg('k')` throws** naming the missing `--arg`; pass `{ default }` or
  `{ required: false }` to relax it.
- **Network routes last one spec.** A scenario installed with
  `lxdev scenario use` stands aside during a run.
- **A timeout never outlives the spec.** A longer action timeout is clamped to
  the spec's remaining time, and the error says so.

## Running and CI

```bash
lxdev test tests/                     # every *.test.ts, recursively
lxdev test tests/ --grep checkout
```

- Reports land in `test-results/<run-id>/` (or `--output-dir`): `report.html`,
  `report.json`, `junit.xml`. Failures fail the command.
- Empty selections fail; opt out with `--pass-with-no-tests`.
- Select with `--id ID`, `--last-failed report.json`, `--tag EXPR`, or
  `--shard 1/3`; shards need separate sessions and output directories. Give
  non-ASCII titles an `id` so `--id`/`--last-failed` survive reordering.
- `--retries N` requires `spec.reset`; reports keep every attempt and flag
  flaky passes.
- `--timeout-secs` bounds the whole run. The default scales with the
  selection: max(300 s, 30 s per spec run), up to 3600 s. When it runs out,
  the rest are reported as not run and lxdev prints `budget exhausted after
  N/M`; raise it or `--shard`.
- `--shuffle` runs specs in a random order and prints the seed;
  `--shuffle=SEED` reproduces it. `--repeat-each N` runs every spec N times.
  Use both to find order dependence and flaky specs.
- `--verbose` shows steps; `--json` returns one result; `--jsonl` streams
  events. Interrupted runs keep partial reports and fail CI.
- Saving a source file while `lxdev test` runs rebuilds it, but `lingxia dev`
  reloads the app only after the run ends, never under a running spec.
- Statuses: `timeout`, `xfail`, and `xpass` stay distinct; an unexpected pass
  fails the run. `spec.fail` without `expected` accepts any body failure.
- Driver rejections carry stable codes (`E_PAGE_NOT_ACTIVE`,
  `E_ELEMENT_NOT_FOUND`, `E_EVAL_SCRIPT`, …; `AUTOMATION_ERROR_CODES`) for
  `t.reject(op, { code })` and `expected.code`. `report.json` `failures[]`
  names each failure's action, page instance and code.
- A failed spec also lists the app's last Logic network calls; see
  [the network log](#the-network-log-of-a-failed-spec).
- Presets: name argument lists once in `lxdev.json` at the project root and
  run them with `--preset NAME`; see [below](#presets).
- `lxdev test --cancel-active` cancels a run left active by a client that
  exited (`automation_run_in_progress`); it refuses a run a live client still polls.
- Keep files generated by test setup outside the watched project;
  `lingxia dev` holds reloads until the run ends. See the
  [development loop](../SKILL.md#the-development-loop).

### Presets

```json
{
  "$schema": "./node_modules/@lingxia/test/schemas/lxdev.schema.json",
  "test": {
    "presets": {
      "ci": ["tests/", "--tag", "unit,routed", "--openapi", "api/openapi.yaml", "--isolate"],
      "nightly": ["tests/", "--state", "demo", "--save-state-on", "always"]
    }
  }
}
```

```bash
lxdev test --preset ci                      # the preset's arguments
lxdev test --preset ci --grep checkout      # then the command line's
lxdev test --preset ci --print-args         # show the effective arguments and exit
lxdev test --list-presets
```

- A preset is the arguments it lists, placed before the command line's:
  repeatable flags (`--tag`, `--openapi`, `--arg`) add up, any other flag
  given on the command line wins.
- `lxdev.json` is found in the project root of the current directory (the
  directory with `package.json`, `lxapp.json` or `lingxia.yaml`).
- Presets are committed, so they may not hold `--secret-arg`, credential-named
  `--arg` keys, `--preset` or `--cancel-active`; pass those on the command
  line. `--print-args` shows secret values as `***`.

## External integration journeys

A spec can prepare external fixtures, operate the app, complete a browser
callback, then verify persisted business results. Correlate external reads with
this run's entity ID and register cleanup. `statusUrl` and `cleanupUrl` below
are product-owned fixture endpoints passed with `--arg`:

```ts
spec('submission reaches the external service', async (t) => {
  const statusUrl = t.arg('statusUrl');
  const cleanupUrl = t.arg('cleanupUrl');
  t.defer(async () => {
    const response = await fetch(cleanupUrl, { method: 'DELETE' });
    if (!response.ok) throw new Error(`Cleanup failed: ${response.status}`);
  });
  await t.app.page.testId('submit-order').click();
  await t.expect(t.app.page.testId('order-confirmation')).toBeVisible();
  await t.expect.poll(async () => (await (await fetch(statusUrl)).json()).status)
    .toBe('submitted');
});
```

For redirects, drive `t.automation.browser` and assert the resulting app state.
For inbound links, call `t.automation.lxapps.applink({ url })`, then wait for
the page outcome; acceptance does not mean navigation completed. Fixture
services and backend mock selection belong to the product/backend.

## Layout and evidence

- `tests/pages/`: a page's behavior and states.
- `tests/flows/`: cross-page, cross-app, and external business journeys.
- `tests/api/`: deliberate `lx.*` runtime contract checks, when needed.

Run focused specs while iterating and the applicable suite at handoff. Report
exercised scenarios with logs/artifacts; review screenshots for UX.
