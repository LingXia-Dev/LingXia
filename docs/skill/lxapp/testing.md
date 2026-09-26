# Product testing with `lxdev test`

`lxdev test` runs `@lingxia/test` specs inside the target App/Runner, on a test
JS worker separate from lxapp Logic and WebViews. Specs drive the page through
`t.app.view` and read the app's Logic through `t.app.logic`.

## First spec

```ts
// tests/pages/notes.test.ts
import { spec } from '@lingxia/test';

interface NotesData { notes: { id: string; title: string }[] }

spec('saving a note lists it', async (t) => {
  await t.app.nav.relaunch({ page: 'notes' });
  await t.app.view.testId('note-title').fill('Groceries');
  await t.app.view.testId('note-save').click();
  await t.expect(t.app.view.testId('note-saved')).toBeVisible();

  const data = await t.app.logic.data<NotesData>();
  t.expect(data.notes.map((note) => note.title)).toContain('Groceries');
});
```

```bash
lingxia dev --background
lxdev test tests/pages/notes.test.ts
```

- `testId('x')` matches `[data-testid="x"]` in the current page's View. It must
  match exactly one element; narrow duplicates with `.nth(i)`.
- `t.app.nav.relaunch({ page })` clears the stack and opens a configured page
  (lxapp.json name) as a new instance, resolving after its `onReady`. Every
  page is unloaded, tab pages kept by `switchTab` included, so the page runs
  `onLoad` again. `spec(title, { fresh: true }, body)`
  relaunches the home page instead. Neither resets app or backend data.
- Actions and `t.expect(...)` retry for 5 s by default (`{ timeout }` per call);
  a spec has 30 s (`spec(title, { timeout }, body)`). Await every action and assertion.
- One waiting assertion: `t.expect(locator)` and `t.expect(() => read())`
  retry until the matcher passes; `t.expect(value)` (like the imported
  `expect`) checks once. `expect(locator)` throws: a locator needs `t.expect`.
- Setup: install `@lingxia/test` matching the project's LingXia line, and keep
  a separate test tsconfig with `lib: ["ES2020"]` (`lingxia new` writes
  `tsconfig.tests.json`). `import '@lingxia/test'` types the test context
  (timers, `fetch`, `console`). A spec program declares no `lx` of its own:
  specs that import product Logic modules type-check with the app's `lx`,
  in any include order.

## Common tasks

| Task | API |
|---|---|
| Start on a known page | `t.app.nav.relaunch({ page, query? })`; `{ fresh: true }` for home |
| Navigate | `t.app.nav.to` / `.redirect` / `.switchTab` / `.back` |
| Find an element | `t.app.view.testId(id)`, `.css(selector)`, `.nth(i)` / `.first()` / `.last()`, `.filter({ hasText })`, `{ page }` option |
| Act | `locator.click()` / `.fill(text)` / `.type(text)` / `.press(key)`; `{ force: true }` on click/fill, see [gotchas](#gotchas) |
| Assert UI | `t.expect(locator).toBeVisible()` / `.toBeInViewport()` / `.toBeAttached()` / `.toHaveText()` / `.toContainText()` / `.toHaveAttribute(name, value?)` / `.toHaveCount()` / `.toHaveValue()` / `.toBeEnabled()`; `.not` |
| Wait for an element state | `locator.waitFor({ state: 'visible' \| 'inViewport' \| 'attached' \| 'hidden' \| 'detached' })` |
| Read page Logic `data` | `t.app.logic.data<T>({ page? })`; see [below](#reading-app-logic) |
| Call a page method | `t.app.logic.call<Page, 'method'>(method, ...args)` |
| Run code in Logic / page DOM | `t.app.logic.eval(fn, ...args)` / `t.app.view.eval(fn, ...args)`; options first: `{ timeout }`, `{ page }` |
| Wait until a value is ready | `t.waitFor(read, { until })` returns it; `t.expect(read).toBe(x)` asserts it |
| Expect a rejection | `await t.reject(() => op(), { code?, message? })`; codes are `TestErrorCode` |
| Wait for a faked call | `await route.waitForCall()`, `await scenario.waitForCall({ http })` |
| Fake Logic `fetch` responses | `t.app.network.route(pattern, handler)`; see [below](#faking-the-network) |
| Put the app into a product state from a scenario file | `t.app.scenario(json, variant?)`; see [below](#scenarios-in-specs) |
| Put the app into a product state by hand | `lxdev scenario use <name>`; see [Scenarios](./scenarios.md) |
| Name a set of `lxdev test` flags | `lxdev.json` presets, `lxdev test --preset ci`; see [presets](#presets) |
| Check responses against an API contract | `lxdev test --openapi api.yaml`, `expect(value).toMatchSchema('Name')`; see [below](#api-contract-checks) |
| Tag specs, run a layer | `spec(title, { tags }, body)`, `spec.configure({ tags })`; `lxdev test --tag`; see [below](#tags-and-layers) |
| File defaults | `spec.configure({ timeout, fresh, tags, requires, … })`: any spec option but `id` |
| Skip without an input | `spec(title, { requires: { args: ['PASSWORD'], openapi: true } }, body)` |
| Fast-forward Logic timers and `Date` | `t.app.clock.install()` / `.tick(ms)`; see [below](#test-clock) |
| Inputs and secrets | `--arg k=v` / `--secret-arg k=v`, `--secrets-file .env.test`, `LXDEV_SECRET_K` / `LXDEV_ARG_K`; read with `t.arg('k')`; see [secrets](#secrets) |
| Cleanup | `t.defer(fn)` (LIFO, runs on success or failure); `spec.afterEach` |
| Restore state before each attempt | `spec.reset(async (t) => { ... })` (required by `--retries`) |
| Skip at run time | `t.skip(reason)`; at registration `spec.skip` / `spec.fixme` |
| Known failure | `spec.fail(title, { expected: { code, message } }, body)` |
| Group trace, add evidence | `t.step(name, fn)`, `t.attach(name, data)` |
| Another running lxapp | `t.apps.lxapp(appId)` |
| App lifecycle and inbound links | `t.automation.lxapps` |
| External web pages, auth/payment callback tabs | `t.automation.browser` |
| Runner presets/appearance | `t.automation.device`; [adaptive testing](adaptive-ui.md#test-runtime-switching) |
| Host shell, terminal, or local OS integration | `t.automation.shell`, `.terminal`, `.desktop` where supported (a host without one rejects the call) |
| External HTTP fixtures, callback collectors, service results | Test-context `fetch` |

The automation root is typed by `@lingxia/types/automation`; platform support
and selectors follow [lxdev](../cli/lxdev.md). In a test program the root
(`t.automation`, or `rawAutomation()` from `@lingxia/test`) is
`HostRunAutomation`; app Logic's `lx.automation()` is the narrower
`Automation`, which has no `network`, nav `waitUntil: 'ready'`, or eval call
tracing. Browser automation targets host browser tabs; desktop automation
requires a macOS/Windows host built with it, such as the Runner. `t.app.surfaceLayout()` reads the host render plan. Restore any existing
shell pins or device settings changed by a test.

## Context and assertions

The test context has `console`, timers, and `fetch`, but no app `lx.*`, DOM,
filesystem, Node built-ins, or dynamic `import()`. Importing product source
does not run it in the app: use the eval helpers below.

Trigger the behavior under test through UI actions; setup, eval, and backend
calls do not replace that product path. Use `t.app`/`t.automation`, not
`rawAutomation()`, which bypasses tracing and fixture guards; keep it for a
setup file that runs before any spec, or a deliberate raw-driver check:

```ts
import { rawAutomation } from '@lingxia/test';

const info = await rawAutomation().lxapp('com.example.app').info();
```

## Reading app Logic

```ts
import { type LogicPage } from '@lingxia/test';

interface DevicesData { devices: { id: string; name: string }[] }
interface DevicesPage extends LogicPage<DevicesData> {
  refresh(): Promise<void>;
  rename(id: string, name: string): Promise<boolean>;
}

const { devices } = await t.waitFor(
  () => t.app.logic.data<DevicesData>(),
  { until: (data) => data.devices.length > 0 },
);
await t.app.logic.call<DevicesPage>('refresh');
const renamed = await t.app.logic.call<DevicesPage, 'rename'>('rename', devices[0].id, 'Office'); // boolean
const path = await t.app.logic.eval(({ lx }) => lx.env.USER_DATA_PATH);
const route = await t.app.logic.eval(({ getCurrentPages }, index) => getCurrentPages()[index].route, 0);
const label = await t.app.view.eval(({ document }) => document.querySelector('#total')?.textContent);
const kept = await t.app.view.eval({ page: 'cart' }, ({ document }) => document.title); // a page below the current one
```

- `t.app.logic.eval(fn, ...args)` runs `fn` in the app's Logic with `{ lx,
  getApp, getCurrentPages }`; `t.app.view.eval(fn, ...args)` runs it in the
  page WebView with `{ document, window }`. Without the DOM lib they are
  `ViewDocument`/`ViewWindow`: `querySelector`, `textContent`, `value`,
  `getAttribute` read without casts.
- An eval may take a third of the spec's budget, at most 10 s. Work that
  legitimately takes longer passes a timeout first:
  `t.app.logic.eval({ timeout: 30_000 }, fn, ...args)`, `t.app.view.eval({
  page, timeout }, fn, ...args)`; it is clamped to the spec's remaining time.
- `logic.call<Page>(method)` accepts only `Page`'s methods; add the method as
  a second type argument to type the result. Types are declared, not
  validated.
- `t.waitFor` fails at once on `TypeError`, `ReferenceError`, and
  `SyntaxError` (override with `retryIf`); other thrown errors retry. On
  timeout it rejects with `E_TIMEOUT` naming the last value.
- The fixture takes functions only; a script string is for the raw driver
  (`rawAutomation().lxapp().eval({ script })`).

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
  await t.app.view.testId('rename-input').fill('Office');
  await t.app.view.testId('rename-save').click();
  await t.expect(t.app.view.testId('rename-error')).toBeVisible();
  const sent = await patch.waitForCall();
  t.expect(sent.body).toEqual({ name: 'Office' });
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
- `route()` returns `{ id, pattern, unroute(), calls(), waitForCall() }`.
  Every call list (`route.calls()`, `t.app.network.calls()`,
  `scenario.calls()`) holds one record: `{ time, kind, method, url, status,
  body, headers, answeredBy: 'rule' | 'route' | 'real' | 'companion', rule? }`
  (`body` parsed when JSON, cut to 64 KiB, `null` for streams, `Blob`,
  `FormData`).
- `waitForCall({ timeout? })` resolves with the next call the route handled
  that no earlier wait returned, including one made before the wait; on
  timeout it rejects with `E_TIMEOUT` listing the recent calls.
- `unroute()` and `unrouteAll()` resolve nothing. Routes last one spec and
  work only in a `lxdev test` run; `t.app.network.calls()` lists only this
  spec's routes.

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
await t.expect(t.app.view.testId('online-count')).toHaveText('4');
const [first, reconnect] = await live.calls();
t.expect(reconnect.headers?.['last-event-id']).toBe('e1');
```

- `Rong.SSE` in Logic receives the events as it would from a server: `retry`
  and `id` apply, and after a `drop` it reconnects with `Last-Event-ID` (the
  next answer of a `sequence` serves the reconnect). A call is recorded for
  every connection attempt. An `sse` answer to Logic `fetch`
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

### Scenarios in specs

A [scenario file](./scenarios.md) (`tests/scenarios/*.json`: `http` and
`function` rules, `match`, variants) serves specs too:

```ts
import checkout from '../scenarios/checkout.json';

spec('an unknown payment result settles as paid', async (t) => {
  const scenario = await t.app.scenario(checkout, 'unknown');
  await t.app.nav.relaunch({ page: 'checkout' });
  await t.app.view.testId('pay').click();
  await t.expect(t.app.view.testId('paid')).toBeVisible();
  const submit = await scenario.waitForCall({ function: 'orders.submit' });
  t.expect(submit.answeredBy).toBe('rule');
  const renames = await scenario.calls({ http: 'PATCH **/devices/*' });
  t.expect(renames[0].body).toEqual({ name: 'Office' });
});
```

- One scenario per spec: a second `t.app.scenario()` replaces the first (to
  switch variants mid-spec), and the spec's end removes it. Routes added with
  `route()` take precedence over its rules.
- It returns `{ name, variant, rules, calls(filter?), waitForCall(target),
  remove() }`. `rules` lists `{ index, target, hits }`; `calls()` lists every
  call that reached the scenario, oldest first, as the `NetworkCall` above:
  `answeredBy` (`rule` with its `rule` number, `route`, `real`, or the
  companion's default), the request `body` or Function arguments, and
  `noMatch` when rules targeted it but none matched. Filter (and wait) with
  `{ http: 'METHOD url' }`, `{ function: 'name' }` or `{ rule: n }`.
- A failed spec reports the scenario (`name:variant`), each rule's hits, and
  the last 20 calls with who answered each.
- Invalid files reject with the rule's path (`rules[2]: …`). `function`
  rules need the dev session's companion to support them; otherwise the call
  rejects and nothing is installed.
- Import JSON with `resolveJsonModule` in the test tsconfig.

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
  (`note`) instead of recorded; `Rong.SSE` connections and Function calls
  are not recorded.
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
const data = await t.app.logic.data<{ devices: unknown[] }>();
t.expect(data.devices[0]).toMatchSchema('Device');                 // #/components/schemas/Device
t.expect(problem).toMatchSchema({ ref: '#/components/schemas/Problem', document: 'openapi.yaml' });
```

- Without `--openapi`, `toMatchSchema` fails saying so. A spec that only means
  something against the contract declares it and is skipped, with the reason,
  when the run has none; `t.openapi` lists the loaded documents:

  ```ts
  spec.configure({ requires: { openapi: true } });   // every spec in the file
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
| `unit` | nothing: pure Logic helpers through `t.app.logic.eval`, time through `t.app.clock` | every change |
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
- Tags use letters, digits and `_ . : / -`. `spec.configure` applies to the
  file that calls it and adds to the spec's own `tags`.
- `spec.configure` takes every spec option but `id` as the file's defaults
  (`timeout`, `fresh`, `restoreProfile`, `app`, `forensics`, `covers`,
  `requires`, …); a spec's own option wins, and `tags`, `covers` and
  `requires` add to the file's.
- `requires: { args: ['PASSWORD'], openapi: true }` reports a spec
  `skipped` with the missing input (`requires --arg PASSWORD=<value> (or
  --secret-arg)`) instead of running it.
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
  t.expect(fired).toBe(3);
  await t.expect(t.app.view.testId('status')).toHaveText('Online');
});
```

- While installed, Logic's `Date` (`new Date()`, `Date()`, `Date.now()`),
  `setTimeout` / `setInterval` / `clear*` and `performance.now()` read test
  time. Timers fire only from `tick(ms)` (each at its own time, in order) or
  `runAll()` (until none is left; rejects after `maxTimers`, default 1000, so
  use `tick` with a `setInterval`). `setSystemTime(t)` moves `Date` without
  firing anything. `install`/`setSystemTime` resolve `{ now, pending }`,
  `tick`/`runAll` `{ now, pending, fired }`.
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
  `E_CLOCK_NOT_INSTALLED`. `uninstall()` resolves nothing; the timers it
  dropped are listed in the run's diagnostics.

## Isolated app data

`--profile` runs the suite on a throwaway copy of the app's data (storage,
`lx://userdata`, `lx://usercache`, `lx://temp`); the developer's own data is
untouched and back in place when the run ends, however it ends.

```bash
lxdev test tests/ --profile empty                 # start empty
lxdev test tests/ --profile auth --profile-save   # reuse, refresh on pass
```

- `--profile empty|NAME|PATH` starts from nothing or from a snapshot. A NAME
  lives under `~/.lingxia/test-state/`; keep PATH snapshots out of git
  (`*.lxstate`) — they can hold sign-in tokens.
- `--profile-save` writes the run's data back to that snapshot after a passing
  run (`--profile-save=always` after any finished run); a snapshot that does
  not exist yet starts empty and is created. It needs a NAME or PATH, not
  `empty`.
- The `Rerun:` line printed under a failed spec repeats `--profile` and
  `--profile-save`, so a rerun keeps a rolling snapshot current.
- Sign in once: a setup spec signs in through the UI only when the app shows it
  is signed out; later runs start from the saved state.
- `spec(title, { restoreProfile: true }, fn)` rolls the app's data back after
  that spec (implies `fresh`); `const cp = await t.profile.checkpoint()`
  (`{ id }`) / `restore(cp)` (`{ kept }`) / `drop(cp)` do it by hand. Both reopen the app, so re-read `t.app` afterwards,
  and both reject with `E_PROFILE_NOT_ISOLATED` without `--profile`.
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
  await t.profile.restore(cp, { keep: ['auth.*', 'session.token'] }); // resolves { kept }
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
  under the pointer (the lower part of an overflowing sheet), or another
  window covers a desktop app window, `click({ force: true })` / `fill(text,
  { force: true })` dispatch DOM events to it directly; it must still be one
  enabled match. It proves less than a normal click, so use it only where
  that fails with `element is obscured`.
- **`fill` works on framework-controlled inputs.** It sets the value through
  the element's native value setter and dispatches `input` and `change`, so
  React/Vue state follows; assert the state, not only the DOM value.
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
- **Secrets go through `--secret-arg`**, `--secrets-file` or `LXDEV_SECRET_*`
  (see [secrets](#secrets)). Their values are `***` in reports, events, and
  attachments. `--arg` keys named like credentials (`password`, `apiKey`) are
  masked only in the report's arg list.
- **`t.arg('k')` throws** naming the missing `--arg`; pass `{ default }` or
  `{ required: false }` to relax it.
- **Network routes and scenarios last one spec.** A dev scenario from
  `lxdev scenario use` stands aside during a run and answers again after it.
- **A timeout never outlives the spec.** A longer action timeout is clamped to
  the spec's remaining time, and the error says so.
- **The fixture takes functions, not script strings.** Pass values as JSON
  arguments, never by building source text. A probe that checks how an API
  rejects bad input casts the call inside `fn`, not the input:
  `t.app.logic.eval(({ lx }, bad) => (lx.fs.stat as (path: unknown) =>
  Promise<unknown>)(bad), 42)`. An error thrown inside `fn` reaches the spec
  as the eval's failure (`E_EVAL_SCRIPT`), so catch it in `fn` to assert the
  API's own `code`. A fixture `t.app` is not a raw `LxAppDriver`: type shared
  helpers against `TestApp`.

## Running and CI

```bash
lxdev test                        # everything (lxdev.json test.entry; else tests/ if it exists)
lxdev test tests/cart.test.ts     # one file; several paths allowed
lxdev test tests/cart.test.ts:42  # the one spec at (or enclosing) line 42
lxdev test --grep "empty cart"    # by title (or --id ID)
lxdev test --last-failed          # what failed last time
lxdev test --list                 # list specs (file:line, id, title, tags) without running them
```

- Each combines with `--preset`, `--tag` and the other flags. A directory
  runs its `*.test.ts` recursively; a line in a loop or helper around spec
  calls selects all of them. `--list` needs the running app.
- A failed spec's `Rerun:` line uses `--id` when the spec sets an `id`, else
  `FILE:LINE`.

- `lxdev test` runs against the app `lingxia dev` is running; for CI see
  [Running specs in CI](#running-specs-in-ci).
- Reports land in a run directory under the results root,
  `test-results/<run-id>/`: `report.html`, `report.json`, `junit.xml`, and
  `test-results/latest` points at the last run. `test-results/` is beside
  `lxdev.json` when the project has one (the same from any subdirectory),
  else in the current directory; `test.outputDir` or `--output-root DIR`
  moves the root. `--output-dir PATH` puts one run's files in PATH itself
  (a fixed path for CI to collect); `latest` in the root still points at it.
  Failures fail the command.
- `lxdev test report [DIR|latest] [--failures] [--format json|junit]` prints a
  finished run's summary, failures and `Rerun:` lines again from its
  `report.json`; it needs no session.
- Exit codes: `0` passed; `1` a spec failed or timed out, the run was
  incomplete, or it could not run; `2` invalid arguments; `130` interrupted.
- Empty selections fail; opt out with `--pass-with-no-tests`.
- `--last-failed` takes a report or run directory (default: the last run).
  `--shard 1/3` needs separate sessions and output directories. Give
  non-ASCII titles an `id` so `--id`/`--last-failed` survive reordering.
- `--last-failed` reruns on the previous run's terms: its `--preset` and its
  `--profile` (with `--profile-save`) carry over unless the command line gives
  them — a new `--preset` brings its own profile — and lxdev prints what it
  reused. When nothing failed it prints `No failed specs in <run>; nothing to
  rerun`, exits 0, starts no run and leaves `latest` alone.
- `--retries N` requires `spec.reset`; reports keep every attempt and flag
  flaky passes.
- `--timeout-secs` bounds the whole run. The default scales with the
  selection: max(300 s, 30 s per spec run), up to 3600 s. When it runs out,
  the rest are reported as not run and lxdev prints `budget exhausted after
  N/M`; raise it or `--shard`.
- `--shuffle` runs specs in a random order and prints the seed;
  `--shuffle=SEED` reproduces it. `--repeat-each N` runs every spec N times.
  Use both to find order dependence and flaky specs.
- `--verbose` shows steps; `--format json` returns one result (`--pretty`
  indents it); `--format jsonl` streams events. Interrupted runs keep partial
  reports and fail CI.
- A run pauses the session's source watcher: saving a file while `lxdev test`
  runs neither rebuilds nor reloads the app under a spec; the saves rebuild
  once when the run ends, however it ends.
- A run starts and ends with the app under test running: a spec that leaves
  it closed has it reopened before the next spec, and the run reopens it at
  the end. When a reopen fails, lxdev prints `Recover: … lxdev lxapp restart`.
- Before a run, lxdev checks that it, the session's host and the project's
  installed `@lingxia/*` packages share a version line, and stops with the fix
  when they do not (`LINGXIA_ALLOW_SKEW=1` makes that a warning).
- Statuses: `timeout`, `xfail`, and `xpass` stay distinct; an unexpected pass
  fails the run. `spec.fail` without `expected` accepts any body failure.
- Rejections carry stable codes, typed as `TestErrorCode` (`TEST_ERROR_CODES`):
  the driver's (`E_PAGE_NOT_ACTIVE`, `E_ELEMENT_NOT_FOUND`, `E_EVAL_SCRIPT`,
  …), `E_TIMEOUT` (a fixture wait or budget), `E_OPENAPI_CONTRACT` and
  `E_SKIPPED`, for `t.reject(op, { code })` and `expected.code`. `report.json` `failures[]`
  names each failure's action, page instance and code.
- A failed spec also lists the app's last Logic network calls; see
  [the network log](#the-network-log-of-a-failed-spec).
- Presets: name argument lists once in `lxdev.json` at the project root and
  run them with `--preset NAME`; see [below](#presets).
- `lxdev test --cancel-active` cancels a run left active by a client that
  exited (`automation_run_in_progress`); it refuses a run a live client still polls.
- Keep files generated by test setup outside the watched project; the watcher
  rebuilds them once the run ends. See the
  [development loop](../SKILL.md#the-development-loop).

### Secrets

```bash
# .env.test — keep it out of git (`lingxia new` adds it to .gitignore)
API_TOKEN=…
TEST_PASSWORD="p@ss word"
```

```bash
lxdev test tests/ --secrets-file .env.test
LXDEV_SECRET_API_TOKEN=… lxdev test tests/     # e.g. from a CI secret
LXDEV_ARG_REGION=eu lxdev test tests/          # a plain --arg REGION=eu
```

- Every entry of a `--secrets-file` (dotenv: `KEY=VALUE`, `#` comments,
  quotes) and every `LXDEV_SECRET_<KEY>` variable is a `--secret-arg`: its
  value is `***` wherever it would appear. `LXDEV_ARG_<KEY>` is a plain
  `--arg`.
- The key is the rest of the variable name, exactly — case included — and
  arg keys are case-sensitive: `LXDEV_SECRET_PASSWORD=…` is
  `t.arg('PASSWORD')`, not `t.arg('password')`. Name the variable
  `LXDEV_SECRET_password` or pass `--secret-arg password=…` for a lower-case
  key; a missing key's error names one given in another case.
- `--print-args` shows these too, one `#` line each with its source
  (`LXDEV_SECRET_PASSWORD`, `--secrets-file .env.test`) and secret values as
  `***`.
- The command line wins over the file, the file over the environment.
- A `Rerun:` line repeats `--secrets-file` and asks for command-line secrets by
  name (`--secret-arg 'token=<token>'`); environment values come back by
  themselves.

### Presets

```json
{
  "$schema": "./node_modules/@lingxia/test/schemas/lxdev.schema.json",
  "test": {
    "entry": "tests/",
    "outputDir": "test-results",
    "presets": {
      "ci": ["--tag", "unit,routed", "--openapi", "api/openapi.yaml", "--profile", "empty"],
      "nightly": ["--profile", "demo", "--profile-save=always", "--secrets-file", ".env.test"]
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
  given on the command line wins, and paths on the command line replace the
  preset's.
- `test.entry`, `test.outputDir`, `test.openapi` and `test.tags` are defaults
  for every run, with or without a preset. `test.outputDir` is the results
  root (`--output-root`): each run gets its own `<run-id>/` in it and
  `latest` points at the last one. A default gives way when the
  preset or the command line sets that flag (or an entry) itself.
- `lxdev.json` is found in the project root of the current directory (the
  directory with `package.json`, `lxapp.json` or `lingxia.yaml`).
- Relative paths in `lxdev.json` — the entry, `test.outputDir`, `--openapi`,
  `--covers-manifest`, `--record-network`, `--output-root`, `--output-dir`,
  `--last-failed`, `--secrets-file`, and
  a PATH given to `--profile` — are relative to the directory holding
  `lxdev.json`, so a preset runs the same from any subdirectory. Paths typed on
  the command line stay relative to the current directory. `--print-args`
  shows them resolved.
- A missing `--openapi` document fails the run before it starts: a contract
  check that silently switched itself off would make a green run mean less.
  When the document is not always present, keep it out of the shared preset
  and add `--openapi PATH` on the command line, or in a second preset.
- Presets are committed, so they may not hold `--secret-arg`, credential-named
  `--arg` keys, `--preset` or `--cancel-active`; pass those on the command
  line, or keep secrets in a gitignored `--secrets-file` the preset names.
  `--print-args` shows secret values as `***`.

## Running specs in CI

| Command | Does |
|---|---|
| `lingxia` | starts and stops the app |
| `lxdev` | works on the running app |

```bash
lingxia dev --background
lxdev test --preset ci
lingxia dev stop
```

Run `lingxia dev stop` even when the tests fail (e.g. an `if: always()` step, a
`trap`, or `finally`). One checkout per job; jobs sharing a checkout name their
session: `lingxia dev --background --name job-a`, `lxdev --session job-a`,
`lingxia dev stop job-a`.

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
  await t.app.view.testId('submit-order').click();
  await t.expect(t.app.view.testId('order-confirmation')).toBeVisible();
  await t.expect(async () => (await (await fetch(statusUrl)).json()).status)
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
