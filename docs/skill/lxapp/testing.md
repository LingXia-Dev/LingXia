# Testing

`lxdev test` runs `@lingxia/test` specs inside the running app or Runner, on a
test worker separate from Logic and WebViews. Specs drive the View through
`t.app.view` and read Logic through `t.app.logic`. Flags and exit codes:
`lxdev test --help`.

## First spec

```ts
// tests/pages/notes.test.ts
import { spec, expect } from '@lingxia/test';

interface NotesData { notes: { id: string; title: string }[] }

spec('saving a note lists it', async (t) => {
  await t.app.nav.relaunch({ page: 'notes' });
  await t.app.view.testId('note-title').fill('Groceries');
  await t.app.view.testId('note-save').click();
  await expect(t.app.view.testId('note-saved')).toBeVisible();

  const data = await t.app.logic.data<NotesData>();
  expect(data.notes.map((note) => note.title)).toContain('Groceries');
});
```

```bash
lingxia dev --background
lxdev test tests/pages/notes.test.ts
```

- `testId('x')` matches `[data-testid="x"]` on the current page and must match
  exactly one element (narrow with `.nth(i)`).
- `t.app.nav.relaunch({ page })` clears the stack and opens a fresh instance of
  a page, resolving after its `onReady`. `spec(title, { fresh: true }, body)`
  relaunches home. Neither resets app or backend data.
- One `expect`: `expect(locator)` and `expect.poll(() => read())` retry until
  the matcher passes; `expect(value)` checks once. Actions and retries wait
  5 s (`{ timeout }` per call); a spec has 30 s (`spec(title, { timeout },
  body)`). Await every action: a body that returns while one still runs fails.
- Text matchers compare whitespace-normalised text.
- Setup: `@lingxia/test` on the project's LingXia line and a test tsconfig
  with `lib: ["ES2020"]` (`lingxia new` writes `tsconfig.tests.json`). The test
  context has `console`, timers, and `fetch`; no app `lx.*`, DOM, filesystem,
  Node built-ins, or dynamic `import()`.

## Common tasks

| Task | API |
|---|---|
| Start on a known page | `t.app.nav.relaunch({ page, query? })`; `{ fresh: true }` for home |
| Navigate | `t.app.nav.to` / `.redirect` / `.switchTab` / `.back` |
| Find an element | `t.app.view.testId(id)`, `.css(selector)`, `.nth(i)` / `.first()` / `.last()`, `.filter({ hasText })`; another page: `t.app.view.page(name)` or `{ page }` |
| Act | `locator.click()` / `.fill(text)` / `.type(text)` / `.press(key)` |
| Assert UI | `expect(locator).toBeVisible()` / `.toBeInViewport()` / `.toBeAttached()` / `.toHaveText()` / `.toContainText()` / `.toHaveAttribute(name, value?)` / `.toHaveCount()` / `.toHaveValue()` / `.toBeEnabled()`; `.not` |
| Wait for an element state | `locator.waitFor({ state: 'visible' \| 'inViewport' \| 'attached' \| 'hidden' \| 'detached' })` |
| Read Logic, call a page method, eval | [Reading Logic](#reading-logic) |
| Wait until a value is ready | `t.waitFor(read, { until })` returns it; `expect.poll(read).toBe(x)` asserts it |
| Expect a rejection | `await t.reject(() => op(), { code?, message? })`; codes are `TestErrorCode` |
| Fake Logic `fetch` / `Rong.SSE` | `t.app.network.route(pattern, handler)`; [Faking the network](#faking-the-network) |
| Load a scenario file | `t.app.mock.use(json, variant?)`; [Scenarios in specs](#scenarios-in-specs) |
| Check responses against OpenAPI | `--openapi`, `toMatchSchema`; [Contract checks](#contract-checks) |
| Tag specs, file defaults | `spec(title, { tags }, body)`, `spec.configure({ timeout, fresh, tags, requires, … })` |
| Skip without an input | `spec(title, { requires: { args: ['PASSWORD'], openapi: true } }, body)` |
| Fast-forward Logic time | `t.app.clock`; [Test clock](#test-clock) |
| Inputs | `t.arg('k')` from `--arg` / `--secret-arg`; [Secrets](#secrets) |
| Cleanup | `t.defer(fn)` (LIFO, always runs); `spec.afterEach` |
| Restore state before each attempt | `spec.reset(async (t) => { ... })` (required by `--retries`) |
| Skip or expect failure | `t.skip(reason)`, `spec.skip`, `spec.fixme`, `spec.fail(title, { expected: { code, message } }, body)` |
| Group trace, add evidence | `t.step(name, fn)`, `t.attach(name, data)` |
| Another running lxapp | `t.automation.lxapp(appId)` |
| App lifecycle, inbound links | `t.automation.lxapps` (`applink({ url })`) |
| External web pages, auth/payment callbacks | `t.automation.browser` |
| Runner device and appearance | `t.automation.device` ([example](./adaptive-ui.md#test-runtime-switching)) |
| Host shell, terminal, desktop | `t.automation.shell`, `.terminal`, `.desktop` where the host has them |
| External fixtures and service results | test-context `fetch` |

Trigger the behaviour under test through the UI; setup, eval, and backend
calls do not replace it. Automation types come from `@lingxia/types/automation`.
`rawAutomation()` from `@lingxia/test` bypasses tracing and fixture guards;
keep it for setup before any spec. Restore shell pins or device settings a
spec changes.

## Reading Logic

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
const label = await t.app.view.eval(({ document }) => document.querySelector('#total')?.textContent);
const kept = await t.app.view.eval({ page: 'cart' }, ({ document }) => document.title); // a page below the current one
```

- `logic.eval(fn, ...args)` runs in Logic with `{ lx, getApp,
  getCurrentPages }`; `view.eval` runs in the page with `{ document, window }`.
- `fn` is sent as source: it cannot use spec variables, imports, or helpers
  (`lxdev test` refuses one that does). Pass values as extra arguments;
  arguments and results must be JSON. An error thrown in `fn` fails the eval
  with `E_EVAL_SCRIPT`; catch it inside `fn` to assert an API's own `code`.
- Options go first: `{ timeout }`, `{ page }` for evals; `{ timeout }`,
  `{ wait: false }` for `call`. An eval gets a third of the spec's budget (at
  most 10 s) by default. `logic.call` resolves when the method's promise
  settles; its types are declared, not validated.
- `t.waitFor` fails at once on `TypeError`, `ReferenceError`, `SyntaxError`
  (override with `retryIf`) and retries other errors; on timeout it rejects with
  `E_TIMEOUT` naming the last value.
- Type shared helpers against `TestApp`.

## Faking the network

`t.app.network` answers the app's Logic `fetch` and `Rong.SSE`, so specs reach
error, loading, and streaming states through the real data layer; never add
test hooks to product code. WebView requests are not routed, and hosts outside
the app's [network grant](../native/permissions.md) are never faked.

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
  await expect(t.app.view.testId('rename-error')).toBeVisible();
  const sent = await patch.waitForCall();
  expect(sent.body).toEqual({ name: 'Office' });
});
```

- Pattern: a glob over the whole URL (`**` any, `*` no `/`, `{a,b}`), a
  `RegExp`, or `{ url, method?, times? }`. The newest matching route wins.
- Handler, one of:
  - fulfill: `{ status?, statusText?, headers?, contentType?, delay? }` with
    `body` or `json` (`delay` up to 30 s);
  - `{ abort: 'failed' }` — a transport failure;
  - `{ continue: true }`, optionally with `patchJson` (a JSON merge patch on
    the real response);
  - `{ hang: true }` — never answers, for spinners and client timeouts;
  - `{ sequence: [...] }` or `{ sse: [...] }` (below).
- `route()` returns `{ id, pattern, remove(), calls(), waitForCall() }`. Call
  records are `{ time, kind, method, url, status, body, headers, answeredBy,
  rule? }`. `waitForCall({ timeout? })` resolves with the next call not yet
  returned, including one made before the wait.
- Routes last one spec. A failed spec's report lists the app's last 20 Logic
  network calls.

```ts
await t.app.network.route('**/v1/devices', { continue: true, patchJson: { items: [], total: 0 } });
```

### Sequences and relative times

`{ sequence: [a, b, c] }` answers in call order; the last repeats:

```ts
await t.app.network.route('**/v1/status', {
  sequence: [{ status: 503 }, { status: 503 }, { json: { up: true } }],
});
```

Answer strings may hold `{{now}}`, `{{now-2h}}`, `{{now+30m}}` (ISO-8601 UTC;
units `ms s m h d`) and `{{nowMs}}`. They read real time, not the
[test clock](#test-clock).

### Server-sent events

`{ sse: [...] }` plays events `{ event?, data, id?, retry? }`, `{ comment }`,
`{ delayMs }`, and a final `{ drop: true }` that closes the stream (without it,
the stream stays open until the spec ends).

```ts
const live = await t.app.network.route('**/v1/events', {
  sequence: [
    { sse: [{ event: 'status', data: { online: 3 }, id: 'e1' }, { delayMs: 500 }, { drop: true }] },
    { sse: [{ event: 'status', data: { online: 4 }, id: 'e2' }] },
  ],
});
await expect(t.app.view.testId('online-count')).toHaveText('4');
const [first, reconnect] = await live.calls();
expect(reconnect.headers?.['last-event-id']).toBe('e1');
```

`Rong.SSE` reconnects after a `drop` with `Last-Event-ID`, served by the next
answer of the sequence ([consuming SSE](./lx-api.md#web-globals)).

### Scenarios in specs

A [scenario file](./mock.md#scenarios) serves specs too, on top of the
session's [mock selection](./mock.md):

```ts
import checkout from '../scenarios/checkout.json';

spec('an unknown payment result settles as paid', async (t) => {
  const scenario = await t.app.mock.use(checkout, 'unknown');
  await t.app.nav.relaunch({ page: 'checkout' });
  await t.app.view.testId('pay').click();
  await expect(t.app.view.testId('paid')).toBeVisible();
  const submit = await scenario.waitForCall({ function: 'orders.submit' });
  expect(submit.answeredBy).toBe('rule');
});
```

- One scenario per spec; a second call replaces it. Routes take precedence
  over its rules. Each spec starts with fresh mock handler state.
- It returns `{ name, variant, rules, calls(filter?), waitForCall(target),
  remove() }`; filter with `{ http: 'METHOD url' }`, `{ function: 'name' }`,
  or `{ rule: n }`.
- Import JSON with `resolveJsonModule` in the test tsconfig.

`lxdev test --record-network DIR` records each spec's real traffic as
`DIR/<spec id>.json`; credentials, tokens, and `--secret-arg` values are
redacted. Review recordings before committing.

## Contract checks

```bash
lxdev test tests/ --tag routed --openapi api/openapi.yaml
```

- A routed (or `patchJson`-patched) response that breaks its OpenAPI 3.x schema
  fails the spec with `E_OPENAPI_CONTRACT`; a real server's break is a warning.
- Only JSON responses are checked; `$ref`s must be local.

```ts
const data = await t.app.logic.data<{ devices: unknown[] }>();
expect(data.devices[0]).toMatchSchema('Device');                 // #/components/schemas/Device
expect(problem).toMatchSchema({ ref: '#/components/schemas/Problem', document: 'openapi.yaml' });
spec.configure({ requires: { openapi: true } });   // skip the file without --openapi
```

## Tags and coverage

| Layer | Talks to | Typical run |
|---|---|---|
| `unit` | nothing: pure Logic helpers through `t.app.logic.eval`, time through `t.app.clock` | every change |
| `routed` | the UI, with every backend call faked | every change, with `--openapi` |
| `live` | a real backend or device | nightly, before release |

```ts
spec.configure({ tags: ['routed'] });            // every spec in this file
spec('lists devices', { tags: ['smoke'] }, async (t) => { /* … */ });  // routed + smoke
```

```bash
lxdev test tests/ --tag '!live'                # everything except live
lxdev test tests/ --tag unit,routed --tag smoke   # (unit or routed) and smoke
```

- Tag every file with `spec.configure`: an untagged spec matches no `--tag`.
  A spec's own options win; `tags`, `covers`, `requires` add to the file's.
- `spec(title, { covers: ['DEV-RENAME'] }, body)` with
  `--covers-manifest tests/coverage.yaml` reports which requirement ids have a
  passing spec and which have none.

## Test clock

`t.app.clock` puts Logic on test time:

```ts
spec('status refreshes every 3 s', async (t) => {
  await t.app.network.route('**/v1/status', { json: { online: true } });
  await t.app.clock.install({ now: '2030-01-01T09:00:00Z' });
  await t.app.nav.relaunch({ page: 'status' });   // start the poll on test time
  const { fired } = await t.app.clock.tick(9_000);  // three polls, in order
  expect(fired).toBe(3);
  await expect(t.app.view.testId('status')).toHaveText('Online');
});
```

- Logic `Date`, `setTimeout` / `setInterval`, and `performance.now()` follow
  it. Timers fire only from `tick(ms)` or `runAll()`; `setSystemTime(t)` moves
  `Date` alone.
- Install before opening the page: timers started earlier stay real. The
  WebView, native work, real requests, route delays, and `setData` delivery
  keep real time. Wait for real I/O with `expect.poll`, then tick again.
- A spec's clock is removed when it ends. `install` twice rejects with
  `E_CLOCK_INSTALLED`; `tick` without one with `E_CLOCK_NOT_INSTALLED`.

## Isolated app data

```bash
lxdev test tests/ --profile empty                 # start empty
lxdev test tests/ --profile auth --profile-save   # reuse, refresh on pass
```

- `--profile empty|NAME|PATH` runs on a throwaway copy of the app's data; the
  developer's data returns when the run ends. Keep PATH snapshots
  (`*.lxstate`) out of git: they hold sign-in tokens.
- `--profile-save` writes the data back after a passing run (`=always` after
  any run).
- `spec(title, { restoreProfile: true }, body)` rolls data back after that
  spec; `t.app.profile.checkpoint()` / `restore(cp)` / `drop(cp)` do it by
  hand. Both need `--profile` and reopen the app.
- An app that rotates refresh tokens keeps its session through a rollback with
  `keep` (storage-key globs):

```ts
spec('edits a device', { restoreProfile: { keep: ['auth.*'] } }, async (t) => { /* ... */ });
```

## Gotchas

- **Visible is not in the viewport.** `toBeVisible` passes below the fold;
  use `toBeInViewport()`. Actions scroll their target into view.
- **`force` is a last resort.** `click({ force: true })` / `fill(text, {
  force: true })` dispatch DOM events directly; use only after
  `element is obscured`. Keep the host window uncovered.
- **`fill` updates framework state**; assert the state, not only the DOM.
- **Nav waits for `onReady`** and rejects if the app replaces the page first;
  pass `waitUntil: 'commit'`, then assert the landing page.
- **Specs share app state.** Nothing resets storage or backend between specs;
  seed and clean up with `t.defer` / `spec.reset`, or use `restoreProfile`.
- **Hooks are file-scoped**: `spec.reset`, `beforeEach`, `afterEach` apply to
  the file that registers them.
- **Specs run on the device.** `fetch('http://127.0.0.1:…')` reaches the
  device's loopback; pass reachable fixture URLs with `--arg`.
- **`t.arg('k')` throws** when missing; pass `{ default }` or
  `{ required: false }`.
- **Timeouts never outlive the spec**; a longer one is clamped.
- **Live `lxdev mock` changes** (scenarios and selections) stand aside
  during a run; specs see `mocks/config.json` and `lingxia dev --mock`.

## Running

```bash
lxdev test                        # everything (lxdev.json test.entry; else tests/ if it exists)
lxdev test tests/cart.test.ts:42  # the one spec at (or enclosing) line 42
lxdev test --grep "empty cart"    # by title (or --id ID)
lxdev test --last-failed          # what failed last time
lxdev test --list                 # list specs without running them
lxdev test report --failures      # reprint the last run, no session needed
```

- Reports go to `test-results/<run-id>/` (`report.html`, `report.json`,
  `junit.xml`); `test-results/latest` is the last run.
- A failed spec prints a `Rerun:` line.
- `--retries N` needs `spec.reset`; `--shard 1/3` needs separate sessions.
- `--cancel-active` cancels a run left behind by a client that exited.
- Saves during a run rebuild once it ends. Keep files that setup generates
  outside the watched project.
- The run fails fast on [version skew](../cli/lingxia.md#lingxia-dev).
- Rejections carry stable `TestErrorCode`s (`E_TIMEOUT`, `E_ELEMENT_NOT_FOUND`,
  `E_EVAL_SCRIPT`, `E_OPENAPI_CONTRACT`, …) for `t.reject` and `expected.code`.
- For unattended runs, start with `lingxia dev --background`, run, and always
  `lingxia dev stop`; name sessions (`--name`) when several share a checkout.

### Secrets

```bash
lxdev test tests/ --secrets-file .env.test     # dotenv, kept out of git
LXDEV_SECRET_API_TOKEN=… lxdev test tests/
LXDEV_ARG_REGION=eu lxdev test tests/          # a plain --arg REGION=eu
```

- Every `--secrets-file` entry and `LXDEV_SECRET_<KEY>` is a `--secret-arg`,
  shown as `***` in reports.
- Keys keep their case: `LXDEV_SECRET_PASSWORD` is `t.arg('PASSWORD')`.
- The command line wins over the file, the file over the environment.
  `--print-args` shows each value's source.

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
lxdev test --preset ci --grep checkout      # preset args, then the command line's
lxdev test --preset ci --print-args         # show the effective arguments
```

- `lxdev.json` sits in the project root. A preset's arguments come first;
  repeatable flags add up and other command-line flags win.
- Paths in `lxdev.json` are relative to it.
- Presets are committed, so they may not hold `--secret-arg` or credential
  args; name a gitignored `--secrets-file` instead.

## Layout

- `tests/pages/` — one page's behaviour and states.
- `tests/flows/` — cross-page, cross-app, and external journeys.
- `tests/api/` — deliberate `lx.*` contract checks.

Run focused specs while iterating and the suite at handoff.
