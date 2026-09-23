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
| Find an element | `t.app.page.testId(id)`, `.css(selector)`, `.nth(i)`, `{ page }` option |
| Act | `locator.click()` / `.fill(text)` / `.type(text)` / `.press(key)` |
| Assert UI | `t.expect(locator).toBeVisible()` / `.toBeAttached()` / `.toHaveText()` / `.toHaveCount()` / `.toHaveValue()` / `.toBeEnabled()`; `.not` |
| Wait for an element state | `locator.waitFor({ state: 'visible' \| 'attached' \| 'hidden' \| 'detached' })` |
| Read page Logic `data` | `t.app.pageData<T>({ page? })`; see [below](#reading-app-logic) |
| Call a page method | `t.app.callPage<R>(method, ...args)` |
| Run code in Logic / page DOM | `t.app.eval(fn, ...args)` / `t.app.page.eval(fn, ...args)` |
| Wait until a value is ready | `t.waitFor(read, accept?)` returns it; `t.expect.poll(read).toBe(x)` asserts it |
| Expect a rejection | `await t.reject(() => op(), { code?, message? })` |
| Fake Logic `fetch` responses | `t.app.network.route(pattern, handler)`; see [below](#routing-logic-fetch) |
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

## Routing Logic `fetch`

`t.app.network` fakes responses to the app's Logic `fetch`, so specs reach HTTP
error paths through the real data layer. Do not add test hooks to product
fetch wrappers.

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
  through.
- `route()` returns `{ id, pattern, unroute(), requests() }`; requests report
  `{ method, url, headers, body, bodyTruncated, action, status }` (`body` cut
  to 64 KiB, `null` for streams, `Blob`, `FormData`).
- Only Logic `fetch` is routed, not WebView requests. A host outside the app's
  [network grants](../native/permissions.md) is never faked.

## Gotchas

- **Visible means in the viewport.** `toBeVisible` and `waitFor()` (default
  `visible`) fail for content below the fold of an overflowing sheet or long
  page. Use `toBeAttached()` or `waitFor({ state: 'attached' })`; actions
  scroll their target into view themselves.
- **Fixture nav waits for `onReady`** (`timeoutMs`, default 15000) and rejects
  if the app replaces the page first, e.g. with its own `lx.reLaunch`. Pass
  `waitUntil: 'commit'` to resolve once the stack changed, then assert the
  landing page.
- **Specs share app state.** A new spec does not reset the app, storage, or
  backend (only a timed-out spec forces a home relaunch). Seed and clean up
  explicitly with `t.defer` or `spec.reset`.
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
- **Network routes last one spec** and work only in a `lxdev test` run;
  `requests()` lists only this spec's routes.
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
- Select with `--id ID`, `--last-failed report.json`, or `--shard 1/3`; shards
  need separate sessions and output directories. Give non-ASCII titles an `id`
  so `--id`/`--last-failed` survive reordering.
- `--retries N` requires `spec.reset`; reports keep every attempt and flag
  flaky passes.
- `--timeout-secs` bounds the whole run (default 300).
- `--verbose` shows steps; `--json` returns one result; `--jsonl` streams
  events. Interrupted runs keep partial reports and fail CI.
- Statuses: `timeout`, `xfail`, and `xpass` stay distinct; an unexpected pass
  fails the run. `spec.fail` without `expected` accepts any body failure.
- `lxdev test --cancel-active` cancels a run left active by a client that
  exited (`automation_run_in_progress`); it refuses a run a live client still polls.
- Keep files generated by test setup outside the watched project;
  `lingxia dev` holds reloads until the run ends. See the
  [development loop](../SKILL.md#the-development-loop).

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
