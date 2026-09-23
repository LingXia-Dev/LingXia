---
title: Testing
description: Write repeatable lxapp cases with @lingxia/test and run them against a live dev session with lxdev test.
sidebar:
  order: 10
---

LingXia tests run **against a running app**, not against a simulated one. `lingxia dev` owns the session; `lxdev test` loads your cases into it and drives the real Logic runtime and the real page webviews. A passing case therefore means the behavior works on that platform, not that a mock agreed with itself.

Cases are written with `@lingxia/test`, the authoring SDK. Spec bodies run in a JavaScript worker **on the target** (phone or desktop Runner), not on the development machine. `fetch('http://127.0.0.1:...')` therefore hits the device loopback.

## Install

```bash
npm install --save-dev @lingxia/test
```

Keep it on the same version as your CLI. `lxdev` warns when the two drift apart, because the SDK and the runner share a protocol.

## Write a case

A case is a `spec` whose async body receives a test handle `t`. Drive the app through locators and retrying assertions:

```ts
import { spec } from '@lingxia/test';

spec('greets through real page input and the Logic bridge', async (t) => {
  await t.app.nav.relaunch({ page: 'home' });
  await t.expect(t.app.page.testId('home-page')).toBeVisible();

  await t.app.page.testId('name').fill('Ada');
  await t.app.page.testId('greet').click();

  await t.expect(t.app.page.testId('greeting')).toContain('Ada');
});
```

Give elements a stable `data-testid` rather than matching on styling. `t.app.page.testId(id)` and `t.app.page.css(selector)` return locators; `t.expect(locator)` retries until the condition holds or the spec budget expires. Imported `expect(value)` from `@lingxia/test` checks once and does not retry.

Every interaction is awaited: the case is talking to another process.

## Wait, never sleep

The app is live, so state arrives when it arrives. Wait for the condition you actually care about:

```ts
await t.expect(t.app.page.testId('total')).toBeVisible();
await t.expect.poll(async () => {
  const response = await fetch(statusUrl);
  return (await response.json()).status;
}).toBe('submitted');
```

A fixed delay is the most common source of a test that passes on your machine and fails in CI. Waiting on the condition costs nothing when the app is fast and still succeeds when it is slow. Register cleanup with `t.defer` so it runs on success or failure.

## Organize by what breaks

Separate cases by the layer they protect, so a failure names the layer:

| Directory | Holds |
| --- | --- |
| `tests/api/` | Logic contracts — what `lx.*` returns and rejects |
| `tests/pages/` | Page behavior — rendering, input, navigation within a page |
| `tests/flows/` | User journeys that cross pages |

An entry file imports the cases you want in one run, which lets one project keep several suites — a fast one for every change, a full one for a release.

## Run

```bash
lxdev test tests/pages/home.test.ts
lxdev test tests/ --grep checkout
```

Pass values into a run with `--arg` (`t.args`), so one suite can cover several platforms or fixture URLs:

```bash
lxdev test tests/flows/checkout.test.ts --arg platform=macos --arg statusUrl=https://…
```

Reports record the args of a run. Keys that look like credentials (`password`, `secret`, `token`, `apiKey`, `credential`) are written as `***`, and `--secret-arg key=value` masks any other key; the spec still reads the real value from `t.args`.

Results print as they finish and are written under `test-results/<run-id>/` (`report.html`, `report.json`, `junit.xml`) so CI can keep them as an artifact.

## Park work without deleting it

A case you know is not ready is more useful declared than missing — it keeps the gap visible in the report:

```ts
spec.skip('resumes an interrupted upload', {
  reason: 'needs the retry API',
});
```

When only the run itself can tell whether a case applies, skip from inside the body. `t.skip(reason)` stops the spec and reports it as skipped with that reason — neither passed nor failed:

```ts
spec('reconnects an offline client', async (t) => {
  const offline = await findOfflineClient(t);
  if (!offline) t.skip('this account has no offline client');
  // …
});
```

`spec.fail` declares a known-broken case: any failure of its body — a failed assertion or a thrown product error — is reported as the expected failure (`xfail`), and a body that completes is an unexpected pass (`xpass`) that fails the run.

## Behavior worth a permanent test

Not everything deserves one. A permanent case earns its keep when it protects a contract that must not silently change: what an API returns, what a page does with input, what a journey guarantees end to end. One-off visual polish is better served by looking at the running app — a screenshot comparison tends to break on every legitimate design change and teaches the team to ignore it.
