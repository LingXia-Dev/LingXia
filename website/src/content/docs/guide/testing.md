---
title: Testing
description: Write repeatable lxapp cases with @lingxia/test and run them against a live dev session with lxdev test.
sidebar:
  order: 9
---

LingXia tests run **against a running app**, not against a simulated one. `lingxia dev` owns the session; `lxdev test` loads your cases into it and drives the real Logic runtime and the real page webviews. A passing case therefore means the behavior works on that platform, not that a mock agreed with itself.

Cases are written with `@lingxia/test`, the authoring SDK.

## Install

```bash
npm install --save-dev @lingxia/test
```

Keep it on the same version as your CLI. `lxdev` warns when the two drift apart, because the SDK and the runner share a protocol.

## Write a case

A case is a `spec` with a title and an async body. Inside it you drive the app through a handle and assert with `expect`:

```ts
import { expect, spec } from '@lingxia/test';

spec('greets through real page input and the Logic bridge', async () => {
  const app = myApp();

  await app.nav.relaunch({ page: 'home' });
  await app.page.waitFor({ page: 'home', css: '[data-testid="home-page"]' });

  await app.page.fill({ page: 'home', css: '[data-testid="name"]', text: 'Ada' });
  await app.page.click({ page: 'home', css: '[data-testid="greet"]' });

  expect(await app.page.text({ page: 'home', css: '[data-testid="greeting"]' }))
    .toContain('Ada');
});
```

Two things are worth noticing. The selectors are ordinary CSS against your own markup — give elements a stable `data-testid` rather than matching on styling. And every interaction is awaited: the case is talking to another process, so nothing is synchronous.

## Wait, never sleep

The app is live, so state arrives when it arrives. Wait for the condition you actually care about:

```ts
await app.page.waitFor({ page: 'cart', css: '[data-testid="total"]', state: 'visible' });
```

A fixed delay is the most common source of a test that passes on your machine and fails in CI — a slower device simply needs longer. Waiting on the condition costs nothing when the app is fast and still succeeds when it is slow.

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
lxdev test tests/entries/all.test.ts
```

Pass values into a run with `--arg`, so one suite can cover several platforms:

```bash
lxdev test tests/entries/desktop.test.ts --arg platform=macos
```

Results print as they finish and are also written as a report, so CI can keep them as an artifact.

## Park work without deleting it

A case you know is not ready is more useful declared than missing — it keeps the gap visible in the report:

```ts
spec.skip('resumes an interrupted upload', {
  reason: 'needs the retry API',
});
```

## Behavior worth a permanent test

Not everything deserves one. A permanent case earns its keep when it protects a contract that must not silently change: what an API returns, what a page does with input, what a journey guarantees end to end. One-off visual polish is better served by looking at the running app — a screenshot comparison tends to break on every legitimate design change and teaches the team to ignore it.
