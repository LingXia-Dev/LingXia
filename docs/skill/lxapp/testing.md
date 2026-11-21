# Testing

`lxdev test` runs `@lingxia/test` tests inside the running app or Runner, on a
test worker separate from Logic and WebViews. Tests drive the UI through
`t.app.view` (the current page) and `t.app.page()` (one fixed page), and reach
into Logic with `t.app.logic.eval` only for what the UI cannot show. Flags and
exit codes: `lxdev test --help`.

## First test

```ts
// tests/pages/notes.test.ts
import { test, expect } from '@lingxia/test';

test('saving a note lists it', { start: { page: 'notes' } }, async (t) => {
  await t.app.view.testId('note-title').fill('Groceries');
  await t.app.view.testId('note-save').click();
  await expect(t.app.view.testId('note-list')).toContainText('Groceries');
});
```

```bash
lingxia dev --background
lxdev test tests/pages/notes.test.ts
```

- `testId('x')` matches `[data-testid="x"]` on the current page and must match
  exactly one element (narrow with `.nth(i)`).
- `test(title, { start: { page, params? } }, body)` relaunches the app on that
  page before the body, like `t.app.nav.reset({ page })` mid-test: the stack
  is cleared and a fresh instance opens, ready after its `onReady`. Neither
  resets app or backend data.
- One `expect`: `expect(locator)` and `expect.poll(() => read())` retry until
  the matcher passes; `expect(value)` checks once; `expect(fn)` takes only
  `toThrow`. Every `expect` needs a matcher: `await expect(x)` alone is a
  type error and fails the test. Actions and retries wait
  5 s (`{ timeout }` per call); a test has 30 s (`test(title, { timeout },
  body)`). Await every action: a body that returns while one still runs fails,
  and so does one whose `fetch` call does not settle soon after it returns.
- Text matchers compare whitespace-normalised text.
- Import `test` by that name and call it directly — `test(…)`, `test.skip(…)`.
  The bundle refuses an alias (`const s = cond ? test.skip : test`), a
  rename, or a re-export, because `FILE:LINE`, `lxdev test list` and `Rerun:`
  lines read test calls from the source.
- Setup: `@lingxia/test` on the project's LingXia line and a test tsconfig
  with `lib: ["ES2020"]` (`lingxia new` writes `tsconfig.tests.json`). The test
  context has `console`, timers, and `fetch`; no app `lx.*`, DOM, filesystem,
  Node built-ins, or dynamic `import()`.

## Running

```bash
lxdev test                        # everything (lxdev.json test.entry; else tests/ if it exists)
lxdev test tests/cart.test.ts:42  # the one test at (or enclosing) line 42
lxdev test --grep "empty cart"    # by title (or --id ID)
lxdev test --last-failed          # what failed last time
lxdev test --changed              # tests the uncommitted work can affect (--changed-since main: since branching)
lxdev test list                   # list tests; no test bodies or hooks run
lxdev test presets                # presets in lxdev.json
lxdev test args --preset ci       # the arguments a run would use
lxdev test cancel                 # release a run left by a dead client (--cancel-active: then run)
lxdev test report --failures      # reprint the last run, no session needed
```

Iterate with `--changed` or a file path, and fix with `--last-failed`. Run
everything once before pushing; CI runs it again.

- Reports go to `test-results/<run-id>/` (`report.html`, `report.json`,
  `junit.xml`); `test-results/latest` is the last run, set after its reports.
- In `junit.xml` a test's `covers` is a `covers` property, and a `test.fail`
  that failed as declared is `<skipped type="xfail">`, never a pass.
- A failure after the cases (`--profile-save`, `--record-network`, a failed
  return of the app to its own data) keeps each case's verdict but exits 1
  and is listed as a run error in every report.
- A failed test prints a `Rerun:` line.
- Each case is tagged with the page renderers its locators read (`[gpu]`, `[webview]`; `renderers` in `report.json`), and the run prints a `renderers:` count.
- `--changed` picks tests that import a changed file or name its page
  (`'home'`); it prints why. A page reached only by tapping is missed: name it.
- `--changed` ignores files outside the test project (host Rust,
  `lingxia.yaml`): after a host change, pick tests with `--grep` or `--tag`.
- `lxdev test list` with a session loads the test modules in the app (their
  top-level code runs) and holds the session like a run. Without one, or with
  `--offline`, it reads the tests from the source; `~` marks a computed title,
  id or tag, or a test registered in a loop, which a run may list differently.
- `--retries N` needs `test.reset`; `--shard 1/3` needs separate sessions.
- A run pauses [auto-reload](../cli/dev-reload.md). Keep files that setup
  generates outside the watched project.
- The run fails fast when the project or the session is outside this tool's [compatibility window](../cli/lingxia.md#lingxia-dev).
- Rejections carry stable `TestErrorCode`s (`E_TIMEOUT`, `E_ELEMENT_NOT_FOUND`,
  `E_EVAL_SCRIPT`, `E_OPENAPI_CONTRACT`, …) for `t.reject` and `expected.code`.
  Declare the app's own codes once to expect them too:
  `declare module '@lingxia/test' { interface AppErrorCodes { E_QUOTA: true } }`.
- For unattended runs, start with `lingxia dev --background`, run, and always
  `lingxia dev stop`; name sessions (`--name`) when several share a checkout.

## Common tasks

| Task | API |
|---|---|
| Relaunch mid-test | `t.app.nav.reset({ page, params? })` |
| Navigate | `t.app.nav.go({ page, params? } \| { tab }, { replace? })` / `.back(n?)`; `.press('back' \| 'home')` presses the navigation bar button (leave guard included) |
| Find an element | `t.app.view.testId(id)`, `.css(selector)`, `.getByRole(role, { name, selected })`, `.nth(i)` / `.first()` / `.last()`, `.filter({ hasText })`; `locator.getByRole(...)` searches inside every match of an un-narrowed locator |
| Another page (below the current one, on a surface) | `(await t.app.page({ name })).view`; [Pages](#pages) |
| Act | `locator.click()` / `.fill(text)` / `.type(text)` / `.press(key)`; `type` inserts at a focused field's selection (replacing it, caret after) and at the end of an unfocused one; `press` takes navigation keys (Tab, arrows, Home/End, Enter/Space, Escape, PageUp/PageDown) and a select's type-ahead letter; in a field a character key is `E_UNSUPPORTED` on every platform, so type it |
| Hover | `locator.hover()` moves a mouse onto it on macOS/Windows (into the app window: the system cursor and window order stay put; `E_UNSUPPORTED` elsewhere); `locator.pin({ hover: true })` holds `hover:` styles and `interestFor` hints with no pointer, on every platform, until `{ hover: false }` or the test's end; it sends no `pointerenter`/`pointerleave` |
| Read an element once | `locator.textContent()` / `.inputValue()` / `.getAttribute(name)` / `.boundingBox()` / `.touchTarget()` (the box a finger reaches) / `.selection()` (a focused GPU editor's `[start, end]`, else `null`) / `.count()` / `.isVisible()`; no retry, so assert with `expect(locator)` |
| Bring an element wholly on screen | `locator.scrollIntoView()`; actions do it themselves, so use it before a `swipe` or a box check |
| Native window input | `t.app.window.pointer` / `.key`; independent of pages |
| Swipe, drag, fling, pull to refresh, edge back | `t.app.view.swipe` / `.drag` / `.scrub` / `.fling` / `.pullToRefresh` / `.touch`, `t.app.gesture.*`; [Gestures](#gestures) |
| Assert UI | `expect(locator).toBeVisible()` / `.toBeInViewport()` / `.toBeAttached()` / `.toHaveText()` / `.toContainText()` / `.toHaveAttribute(name, value?)` / `.toHaveCount()` / `.toHaveValue()` / `.toBeEnabled()` / `.toBeChecked()` / `.toBeSelected()` / `.toBeExpanded()` / `.toBePressed()` (fail either way on an element without that state) / `.toBeBusy()` (`aria-busy`; absent is not busy); `.not` |
| Wait for an element state | `locator.waitFor({ state: 'visible' \| 'inViewport' \| 'attached' \| 'hidden' \| 'detached' })` |
| Page data, page actions, Logic eval | [Pages](#pages), [Logic eval](#logic-eval) |
| Wait until a value is ready | `t.waitFor(read, { until })` returns it; `expect.poll(read).toBe(x)` asserts it |
| Expect a rejection | `await t.reject(() => op(), { code?, message? })`; `code` is a `TestErrorCode` or one the app declared in `AppErrorCodes` |
| Fake Logic `fetch` / `Rong.SSE` | `t.app.network.route(pattern, handler)`; [Faking the network](#faking-the-network) |
| Load a scenario file | `t.scenario.use`; [Scenarios in tests](#scenarios-in-tests) |
| Check responses against OpenAPI | `--openapi`, `toMatchSchema`; [Contract checks](#contract-checks) |
| Tag tests, file defaults | `test(title, { tags }, body)`, `test.configure({ timeout, start, tags, requires, skip, … })` |
| Fast-forward Logic time | `t.app.clock`; [Test clock](#test-clock) |
| Toasts, confirm dialogs, action sheets | `t.app.dialogs`; [Dialogs](#dialogs) |
| GPU page frame pacing | `t.app.metrics({ windowMs? })`: frame time p50/p95/p99 ms, `dropped`, `jank`, `inputToPresent`, `firstPresentMs`, `revealMs`, `firstPresentAt`, `reveal` counters; `t.app.hud(true)` draws them over the page |
| Compare screens against baselines | `t.visual.shot(name, { mask })`; [Visual baselines](#visual-baselines) |
| GPU page look for screenshots | `t.app.view.environment({ theme: 'dark', still: true })` pins theme and stills motion for this test only (device settings untouched; `null` clears); `textScale: 1.3` (0.5 up to the lxapp's `maxTextScale`, default 2; above it fails) and `contrast: 'high'` pin the system text size and contrast the same way; `t.app.view.settled()` waits until no frame presents for 500 ms |
| Inputs | `t.arg('k')` from `--arg` / `--secret-arg`; [Secrets](#secrets) |
| Cleanup | `t.defer(fn)` (LIFO, always runs); `test.afterEach` |
| Restore state before each attempt | `test.reset(async (t) => { ... })` (required by `--retries`) |
| Skip | `test.skip`, `test.fixme`; on a condition `{ skip: cond ? 'why' : undefined }` (whole file: `test.configure({ skip })`); found mid-test `t.skip(reason)`; without an input `{ requires: { args: ['PASSWORD'], openapi: true } }` |
| Run another lxapp's tests | import its test files from your entry; skip the ones this app cannot run with `skipTestFiles({ '../other/lxapp/tests/x.test.ts': 'why' })` from `@lingxia/test/runner` (names as reports print them); for tests from a package, `skipTestFiles({ 'pages/x.test.ts': 'why' }, { root: ['pkg-dir/specs', '@scope/pkg/specs'] })` matches under that directory wherever it resolves (linked or installed) |
| Expect a failure | `test.fail(title, { expected: { code, message } }, body)`; found mid-test `t.fail(reason)` (xfail) — e.g. a frame budget missed when `(await t.app.metrics()).profile === 'debug'` |
| Group trace, add evidence | `t.step(name, fn)` (await one at a time; nest through the callback's `step`), `t.attach(name, data)` |
| Another running lxapp | `t.automation.lxapp(appId)` |
| App lifecycle, inbound links | `t.automation.lxapps` (`applink({ url })`) |
| External web pages, auth/payment callbacks | `t.automation.browser`: `snapshot()` then act on its refs (`click({ ref })`, `fill`, `selectOption`, …) or `{ css }`; same auto-wait and failure codes (`covered`, `stale_ref`, …) as `lxdev browser` |
| Runner device and appearance | `t.automation.device` ([example](./adaptive-ui.md#test-runtime-switching)) |
| Host shell, terminal, desktop | `t.automation.shell`, `.terminal`, `.desktop` where the host has them |
| External fixtures and service results | test-context `fetch` |

Trigger the behaviour under test through the UI; setup, eval, and backend
calls do not replace it. Automation types come from `@lingxia/types/automation`.
Read runtime info and element snapshots with camelCase fields (`appId`,
`currentPage`, `navigationBar`, `rect.centerX`); CLI JSON keeps its wire names.
Element boxes are where the element is drawn, mid-transition included: after a
layout change, read a box with `expect.poll` until it settles.
Restore shell pins or device settings a test changes. Routes, `t.scenario.use`
scenarios, test clocks and dialog answers a test installs are removed when it
ends; its leftover timers are cancelled.

## Gestures

Gestures are real pointer input: a timed finger (Android), mouse or
trackpad (macOS) sequence injected into the host window, so page chrome —
pull to refresh, scroll edges, flings — handles it as it would a user's.
Points are `[x, y]` in page CSS pixels or a locator (its center).

```ts
test('pulling the list refreshes it once', { start: { page: 'feed' } }, async (t) => {
  const pull = await t.app.view.pullToRefresh();            // distance 160, hold 300 ms, release
  expect(pull.refreshed).toBe(true);
  await expect(t.app.view.testId('feed-status')).toHaveText('Refreshing…');
  await t.app.view.fling('up', 4000);                       // finger direction, px/s
  await t.app.view.swipe({ from: [200, 600], to: [200, 200], durationMs: 400 });
  await t.app.view.drag(t.app.view.testId('card'), { dx: 0, dy: 120, durationMs: 500 });
  await t.app.view.scrub(t.app.view.testId('plot'), { from: [10, 40], to: [200, 40] }); // points in the element's box
});
```

- `pullToRefresh({ distance?, hold?, release?, at? })` returns `{ refreshed,
  maxOffset, phases }`, sampled from the page's refresh indicator as the
  finger moves; it rejects with `E_UNSUPPORTED` on a page without one.
- `t.app.view.touch(at, { press? })` holds a pointer for checks mid-gesture:
  `.moveTo(at, { durationMs })`, `.hold(ms)`, `.up()`, `.cancel()`. On a
  desktop it defaults to trackpad fingers, which press nothing at rest; pass
  `press: true` to hold a slider or scrub (the mouse there, a finger on touch).
- `t.app.view.refreshIndicator()` reads `{ phase, offset }` (`idle`,
  `pulling`, `armed`, `refreshing`, `ending`).
- `t.app.gesture.swipe(...)` / `.edgeBack({ edge? })` act on the host window
  in window points. Edge back is a system gesture; hosts that cannot inject
  into it reject with `E_UNSUPPORTED`.
- Moves are spaced in real time (default one per 16 ms). Each gesture returns
  `{ input, delivered, events, durationMs }`; `delivered: 'view'` means the
  host fell back past its input dispatcher, and `report.json` lists every
  gesture's `delivered`.
- `requires: { input: ['touch', 'trackpad'] }` skips the test, with the
  reason, on a host that has none of those devices.
- iOS takes page gestures on GPU pages only (`delivered: 'view'`); a WebView
  app there reports no input, so `requires.input` skips. HarmonyOS behaves the same.
- On HarmonyOS, `app.chrome` and window taps reach only the host's own toast, modal and action sheet (`delivered: 'view'`).
- `app.chrome.elements()` bounds are in window points; measure them against `app.chrome.viewport()`, not
  `app.view.viewport()`, which is in the page's own points (a desktop device frame scales those).
- `requires: { chrome: true }` (likewise `refreshIndicator`, `hostInput`) skips where the host cannot
  read its bars, read pull to refresh, or reach either with input; an undeclared `E_UNSUPPORTED` fails.
- `requires: { pointer: 'coarse' }` (or `'fine'`) runs only where GPU pages get that pointer: touch sheets
  vs mouse popovers. A host or WebView page that reports none skips.
- A locator `click()` or scroll on an Android WebView page goes to the WebView
  itself, even for a target below the fold, so page chrome never sees it; drive
  chrome (pull to refresh, edge back) with the gestures above.

## Pages

`t.app.view` is whichever page is current at each call. `t.app.page()` binds
one live page instance — the current one, or the one `{ name }` / `{ instanceId }`
names, waiting up to `{ timeout }` (5 s) for it to open — and never follows
navigation: once that instance is replaced or closed, every call on the handle
rejects. Bind again after navigating.

Type the handle with the page itself: `t.app.page<typeof page>()` takes `data`
and `actions` from `import type page from '../../pages/devices/index'`, the same
type the View uses. A `PageContract<Data, Actions>` from `@lingxia/types/page`
also works where the test cannot see the Logic file.

```ts
import type devicesPage from '../../pages/devices/index';

const devices = await t.app.page<typeof devicesPage>({ name: 'devices' });
const { devices: list } = await t.waitFor(() => devices.data(), { until: (d) => d.devices.length > 0 });
expect(await devices.actions.rename({ id: list[0].id, name: 'Office' })).toBe(true);
await expect(devices.view.testId('device-name').first()).toHaveText('Office');
```

- `data()` is the instance's Logic `data` as `page.update` delivers it to the View
  (undefined members omitted). Page data must be JSON; a Date, Map or
  function in it does not compile.
- `actions.x(payload?)` calls a public page action through the page's own
  bridge, as the View does, and resolves when Logic settles it. Actions take at
  most one JSON payload; streamed (generator) actions are for the View.
- `page.invoke('rename', { payload: { id, name }, timeout: 2_000 })` gives one
  action its own timeout (default: the test's remaining budget). A timed-out
  action may still run, so the test fails even if it catches the timeout, and
  the next test starts from a relaunched app.
- A name open more than once rejects; select it by `{ instanceId }`
  (`t.app.nav.stack()` lists them).

## Logic eval

```ts
const path = await t.app.logic.eval(({ lx }) => lx.fs.userData);
const label = await t.app.view.eval(({ document }) => document.querySelector('#total')?.textContent);
const slow = await t.app.logic.eval({ timeout: 30_000 }, async ({ lx }, key: string) => lx.storage().get(key), 'cache');
```

- `logic.eval(fn, ...args)` runs in Logic with `{ lx, appInstance, pageStack,
  getPage, currentPage }` (`appInstance()` is the object `App({...})`
  registered; neither exists for app code); `view.eval` runs in the page with
  `{ document, window }`. Reach for them when the UI cannot show what a test checks.
- `fn` is sent as source: it cannot use test variables, imports, or helpers
  (`lxdev test` refuses one that does). Pass values as extra arguments;
  arguments and results must be JSON (interfaces work without an index signature).
  Top-level void is allowed; nested undefined, Date, methods and DOM handles reject
  instead of being silently transformed. An error thrown in `fn` fails the eval
  with `E_EVAL_SCRIPT`; an `lx.*` error keeps its own `code` for `t.reject` and
  `expected.code`, and its `data` (e.g. `bizCode`) is readable inside `fn`.
- Options go first: `{ timeout }`. An eval may take 10 s by default, clamped
  to what the test has left. A timed-out eval fails the test even if caught
  (its script may still run); the next test starts from a relaunched app.
- `t.waitFor` and `expect.poll` retry failed reads, including a page eval's
  `TypeError` (the DOM not rendered yet) and nested locator timeouts. A Logic
  `TypeError`, other programming errors and invalid JSON fail immediately
  (override with `retryIf`); timeout reports `E_TIMEOUT` and the last observation.
- Type shared helpers against `TestApp`, `TestPage<C>` or `TestView`.

## Faking the network

`t.app.network` answers the app's Logic `fetch` and `Rong.SSE`, so tests reach
error, loading, and streaming states through the real data layer; never add
test hooks to product code. WebView requests are not routed, and hosts outside
the app's [network grant](../native/permissions.md) are never faked.

### Routes

```ts
test('rename shows the not-implemented error', async (t) => {
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
  records are `{ seq, time, kind, method, url, status, body, headers,
  answeredBy, rule? }`. `waitForCall({ timeout? })` resolves with the next
  call not yet returned, including one made before the wait.
- Routes last one test. A failed test's report lists the app's last 20 Logic
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
the stream stays open until the test ends).

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

### Scenarios in tests

A [scenario file](./mock.md#scenarios) serves tests too, on top of the
session's [mock selection](./mock.md):

```ts
import checkout from '../scenarios/checkout.json';

test('an unknown payment result settles as paid', async (t) => {
  const scenario = await t.scenario.use(checkout, { variant: 'unknown' });
  await t.app.nav.reset({ page: 'checkout' });
  await t.app.view.testId('pay').click();
  await expect(t.app.view.testId('paid')).toBeVisible();
  const submit = await scenario.waitForCall({ function: 'orders.submit' });
  expect(submit.answeredBy).toBe('rule');
});
```

- One scenario per test; a second call replaces it. Routes take precedence
  over its rules. Each test starts with fresh mock handler state.
- `t.scenario.use(checkout, { app: 'other-app' })` installs the scenario's
  `http` rules for that app; a scenario with `function` rules rejects.
- It returns `{ name, variant, rules, calls(filter?), waitForCall(target),
  remove() }`; filter with `{ http: 'METHOD url' }`, `{ function: 'name' }`,
  or `{ rule: n }`.
- Import JSON with `resolveJsonModule` in the test tsconfig.

`lxdev test --record-network DIR` records each test's real traffic as
`DIR/<test id>.json`; credentials, tokens, and `--secret-arg` values are
redacted. Review recordings before committing.

## Contract checks

```bash
lxdev test tests/ --tag routed --openapi api/openapi.yaml
```

- A routed (or `patchJson`-patched) response that breaks its OpenAPI 3.x schema
  fails the test with `E_OPENAPI_CONTRACT`; a real server's break is a warning.
- Only JSON responses are checked; `$ref`s must be local.
- A response that cannot be attributed to the test (still in flight, or from
  an earlier one) fails that test.

```ts
const { devices } = await (await t.app.page<DevicesPage>()).data();
expect(devices[0]).toMatchSchema('Device');                      // #/components/schemas/Device
expect(problem).toMatchSchema({ ref: '#/components/schemas/Problem', document: 'openapi.yaml' });
test.configure({ requires: { openapi: true } });   // skip the file without --openapi
```

## Tags and coverage

| Layer | Talks to | Typical run |
|---|---|---|
| `routed` | the UI, with every backend call faked | every change, with `--openapi` |
| `live` | a real backend or device | nightly, before release |

Pure Logic helpers are plain TypeScript: import them into a Node test runner
(`node --test`, vitest) instead. `lxdev test` is for behaviour that needs the
app running.

A GPU View component is unit-tested in Node with `@lingxia/view/testing`: `render(page, view)`
compiles a page and runs it on the real View runtime, with a phone `env` you can override. Query by
test id, text or the props you wrote; every element event has a screen method (`press`, `type`,
`toggle`, `select`, `slide`, `escape`, …). Roles, names and layout need a device: use `lxdev test`.
Theme steps (`p-4`, `text-sm`) reach the wire as `[slot, times, plus]`; `points(props)` resolves them
under the built-in pack to compare with numbers, and `render(page, view, { points: true })` does so for every node.

```js
import { render, textOf } from '@lingxia/view/testing';
const screen = await render('tests/counter.page.tsx', { data: { label: 'Taps' } });
screen.press(screen.getByTestId('inc'));   // fires onClick, re-renders
assert.equal(textOf(screen.getByTestId('value')), 'Taps: 1');
```

```ts
test.configure({ tags: ['routed'] });            // every test in this file
test('lists devices', { tags: ['smoke'] }, async (t) => { /* … */ });  // routed + smoke
```

```bash
lxdev test tests/ --tag '!live'                # everything except live
lxdev test tests/ --tag routed --tag smoke   # routed and smoke
```

- Tag every file with `test.configure`: an untagged test matches no `--tag`.
  A test's own options win; `tags`, `covers`, `requires` add to the file's.
- `test(title, { covers: ['DEV-RENAME'] }, body)` with
  `--covers-manifest tests/coverage.yaml` reports which requirement ids have a
  passing test and which have none.
- `covers` lists the requirement ids a test proves (`AC-OPEN-01`,
  `lx.storage`), not `tags`. `id` is the test's own unique stable id: never
  put a requirement or acceptance id in `id`.

## Test clock

`t.app.clock` puts Logic on test time:

```ts
test('status refreshes every 3 s', async (t) => {
  await t.app.network.route('**/v1/status', { json: { online: true } });
  await t.app.clock.install({ now: '2030-01-01T09:00:00Z' });
  await t.app.nav.reset({ page: 'status' });   // start the poll on test time
  const { fired } = await t.app.clock.tick(9_000);  // three polls, in order
  expect(fired).toBe(3);
  await expect(t.app.view.testId('status')).toHaveText('Online');
});
```

- Logic `Date`, `setTimeout` / `setInterval`, and `performance.now()` follow
  it. Timers fire only from `tick(ms)` or `runAll()`; `setSystemTime(t)` moves
  `Date` alone.
- Install before opening the page: timers started earlier stay real. The
  WebView, native work, real requests, route delays, and state delivery
  keep real time. Wait for real I/O with `expect.poll`, then tick again.
- A test's clock is removed when it ends. `install` twice rejects with
  `E_CLOCK_INSTALLED`; `tick` without one with `E_CLOCK_NOT_INSTALLED`.

## Dialogs

During a test, `t.app.dialogs` sees the dialogs the app's Logic opens:

```ts
test('deleting asks first', async (t) => {
  await t.app.dialogs.answerNextModal({ confirm: true });  // before the tap
  await t.app.view.testId('delete').click();
  await expect.poll(() => t.app.dialogs.toasts())
    .toContainEqual(expect.objectContaining({ message: 'Deleted' }));
  expect(await t.app.dialogs.modals()).toContainEqual(
    expect.objectContaining({ title: 'Delete device?', answer: { confirm: true } }));
});
```

- Toasts are recorded (`{ message, tone, loading, durationMs, at }`) and still drawn;
  never stub `lx.ui.toast` in Logic.
- Dialogs (`lx.ui.alert` / `confirm`) and `lx.ui.actionSheet` are
  drawn unless the test queues an answer for the next one, so a test can tap them
  (`.lx-modal-btn-confirm`); `modals()` / `actionSheets()` record them with
  `drawn: true` and the user's choice as `answer`.
- `answerNextModal({ confirm })` and `answerNextActionSheet({ index } |
  { cancel: true })` each answer the next dialog only; later ones are drawn.
  An unused answer fails the test when it ends.
- `setAnswerMode({ modals: 'strict', actionSheets: 'strict' })` makes a dialog
  with no queued answer fail the test at once; `withAnswerMode(mode, body)`
  applies it only while `body` runs.
- Another app's dialogs (`t.automation.lxapp(appId).dialogs`) are recorded
  from the first call on it.
- Each test starts with nothing recorded or queued. Outside a test run
  dialogs draw as usual.

## Visual baselines

`await t.app.view.environment({ theme: 'dark', still: true })`, then
`await t.visual.shot('home', { mask: [t.app.view.testId('clock')] })` attaches
the settled page; mask rects are in the screenshot's own pixels.
A shot of a page with a `<webview>` waits up to 4 s for it to draw and fails
`E_WEBVIEW_NOT_READY` naming it; `t.visual.shot` always waits, while
`view.screenshot({ allowNotReady: true })` captures it as its box's background.

- `lxdev test --visual compare` fails a shot that changed against this device's
  baseline at its screen size; a shot with a baseline only at another size (a
  fold, a resized layout) fails as a size mismatch, one with none is `new`.
- `lxdev test --visual accept` makes each pinned shot the baseline (the old one
  is kept as `.prev`); an unpinned shot is refused.
- `--only SHOT` limits either to a shot name or `<test id>/<name>`;
  `--baseline-dir DIR` replaces `~/.lingxia/visual` (`<app>/<device>/<WxH>/` below it).
- `visual.html` and the end of `report.html` show before | after | diff, changed first.

## Isolated app data

```bash
lxdev test tests/ --profile empty                 # start empty
lxdev test tests/ --profile auth --profile-save   # reuse, refresh on pass
```

- `--profile empty|NAME|PATH` runs on a throwaway copy of the app's data; the
  developer's data returns when the run ends. Keep PATH snapshots
  (`*.lxstate`) out of git: they hold sign-in tokens.
- `--profile-save` writes the data back after a passing run (`=always` after
  any run). With it, a missing snapshot starts empty; so does one taken by
  another install of the host, which is moved to `<file>.stale`.
- `test(title, { restoreProfile: true }, body)` rolls data back after that
  test and starts it on `start` (or home); `t.app.profile.checkpoint()` / `restore(cp)` / `drop(cp)` do it by
  hand. Both need `--profile` and reopen the app.
- An app that rotates refresh tokens keeps its session through a rollback with
  `keep` (storage-key globs):

```ts
test('edits a device', { restoreProfile: { keep: ['auth.*'] } }, async (t) => { /* ... */ });
```

## Secrets

```bash
lxdev test tests/ --secrets-file .env.test     # dotenv, kept out of git
LXDEV_SECRET_API_TOKEN=… lxdev test tests/
LXDEV_ARG_REGION=eu lxdev test tests/          # a plain --arg REGION=eu
```

- Every `--secrets-file` entry and `LXDEV_SECRET_<KEY>` is a `--secret-arg`,
  shown as `***` in reports.
- Keys keep their case: `LXDEV_SECRET_PASSWORD` is `t.arg('PASSWORD')`.
- The command line wins over the file, the file over the environment.
  `lxdev test args` shows each value's source.

## Presets

```json
{
  "$schema": "./node_modules/@lingxia/test/schemas/lxdev.schema.json",
  "test": {
    "entry": "tests/",
    "outputDir": "test-results",
    "presets": {
      "ci": ["--tag", "routed", "--openapi", "api/openapi.yaml", "--profile", "empty"],
      "nightly": ["--profile", "demo", "--profile-save=always", "--secrets-file", ".env.test"]
    }
  }
}
```

```bash
lxdev test --preset ci --grep checkout      # preset args, then the command line's
lxdev test args --preset ci                 # show the effective arguments
```

- `lxdev.json` sits in the project root. A preset's arguments come first;
  repeatable flags add up and other command-line flags win.
- Paths in `lxdev.json` are relative to it.
- `test.setup` (or `--setup`) names modules evaluated before any test file, so a test run
  alone with `--id` sees the globals its imports read.
- Presets are committed, so they may not hold `--secret-arg` or credential
  args; name a gitignored `--secrets-file` instead.

## Gotchas

- **Visible is not in the viewport.** `toBeVisible` passes below the fold;
  use `toBeInViewport()`. Actions scroll their target into view.
- **`force` is a last resort.** `click({ force: true })` / `fill(text, {
  force: true })` dispatch DOM events directly; use only after
  `element is obscured`.
- **Runs leave the window where it is.** A GPU lxapp runs covered or behind other apps: frames
  complete and count in `t.app.metrics`, finite animations finish, looping ones park. With a
  WebView lxapp open, a run orders a minimized or fully covered window in front at its start,
  without taking focus, and logs why; keep it uncovered after that (`lxdev host focus`).
- **A locked screen stops the run** on macOS and Windows (where the clipboard is refused): pages are hidden, so the rest
  are reported as not run. Unlock it and run again.
- **`fill` updates framework state**; assert the state, not only the DOM.
- **`fill` sets text without focusing**: no keyboard rises, so no keyboard avoidance runs. To exercise the keyboard, `click()` the field, then `type()`.
- **`fill` on a GPU date or time input sets it as its picker would**: ISO text only, and a value past `min`/`max` fails naming the bound (`E_EDIT_FAILED: 'back' takes a date from 2026-12-18`).
- **`type` into a GPU number field** fails at once with `E_EDIT_REFUSED` when the field's keyboard has no such key (`.` in `inputMode="numeric"`, a second `.` in a decimal field).
- **To see the slow-load recovery panel**, set `"devHoldFirstState": true` in a page's `index.json`: under `lingxia dev` that page never gets its first state. Ignored outside a dev session.
- **A GPU page whose first render threw, or that stopped rendering after repeated failures,** fails a locator or an action at once with code `E_PAGE_FAILED` and the error, not a timeout. A failure that still retries under its last frame keeps waiting.
- **GPU `<List>` rows off screen** answer `testId()` locators as attached; actions scroll them in, and
  text assertions wait until the row draws.
- **Nav waits for `onReady`** and rejects if the app replaces the page first;
  pass `waitUntil: 'commit'`, then assert the landing page.
- **Tests share app state.** Nothing resets storage or backend between tests;
  seed and clean up with `t.defer` / `test.reset`, or use `restoreProfile`.
- **Hooks are file-scoped**: `test.reset`, `beforeEach`, `afterEach` apply to
  the file that registers them.
- **Tests run on the device.** `fetch('http://127.0.0.1:…')` reaches the
  device's loopback; pass reachable fixture URLs with `--arg`.
- **`t.arg('k')` throws** when missing; pass `{ default }` or
  `{ required: false }`.
- **An abandoned test stops the run**: the rest are skipped, since its
  leftover promises could resume inside the next test. Await all work, or
  raise `timeout`.
- **Live `lxdev mock` changes** (scenarios and selections) stand aside
  during a run; tests see `mocks/config.json` and `lingxia dev --mock`.

## Layout

- `tests/pages/` — one page's behaviour and states.
- `tests/flows/` — cross-page, cross-app, and external journeys.
- `tests/api/` — deliberate `lx.*` contract checks.
