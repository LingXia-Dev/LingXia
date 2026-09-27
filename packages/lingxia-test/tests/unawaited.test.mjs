import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { spec, expect } from "../dist/index.js";
import { reset, run } from "../dist/runner.js";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";

afterEach(() => {
  reset();
  delete globalThis.lx;
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
});

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

test("a body that returns with an un-awaited assertion fails, naming it and where it started", async () => {
  const world = createWorld();
  world.add({ testId: "home-greeting", visible: false });
  installFakeHost(world);

  spec("greets by name", { forensics: false }, async (t) => {
    expect(t.app.view.testId("home-greeting")).toBeVisible({ timeout: 2_000 });
  });

  const started = Date.now();
  const report = await run();
  const [result] = report.cases;
  assert.equal(result.status, "failed");
  assert.equal(result.error.phase, "body");
  assert.match(result.error.message,
    /^"greets by name" returned while 1 fixture call was still running \(was it awaited\?\):\n  expect\(\[data-testid="home-greeting"\]\)\.toBeVisible  at .*unawaited\.test\.mjs:\d+:\d+ \(started \d+ms ago\)$/);
  assert.ok(Date.now() - started < 1_500, "the orphaned retry was stopped, not waited out");
});

test("an un-awaited action is stopped: it never lands after the spec returned", async () => {
  const world = createWorld();
  const save = world.add({ testId: "save", visible: false });
  setTimeout(() => { save.visible = true; }, 150);
  installFakeHost(world);
  let later = true;

  spec("saves", { forensics: false }, async (t) => {
    t.app.view.testId("save").click({ timeout: 2_000 });
    void t.app.nav.current();
  });
  spec("next", { forensics: false }, async () => {
    await pause(300);
    later = save.clicked === undefined;
  });

  const report = await run();
  assert.equal(report.cases[0].status, "failed");
  assert.match(report.cases[0].error.message, /2 fixture calls were still running/);
  assert.match(report.cases[0].error.message, /page\.click \[data-testid="save"\]/);
  assert.equal(report.cases[1].status, "passed", JSON.stringify(report.cases[1].error));
  assert.ok(later, "the click never landed");
});

test("awaited calls pass; a thrown error wins over an un-awaited call; cleanup still works", async () => {
  const world = createWorld();
  world.add({ testId: "ready" });
  world.add({ testId: "late", visible: false });
  installFakeHost(world);
  let cleaned = false;

  spec("awaits", { forensics: false }, async (t) => {
    await expect(t.app.view.testId("ready")).toBeVisible();
    await Promise.all([t.app.nav.current(), expect.poll(() => 1).toBe(1)]);
  });
  spec("throws", { forensics: false }, async (t) => {
    expect(t.app.view.testId("late")).toBeVisible({ timeout: 1_000 });
    throw new Error("boom");
  });
  spec("cleans up", { forensics: false }, async (t) => {
    t.defer(async () => {
      await expect(t.app.view.testId("ready")).toBeVisible();
      cleaned = true;
    });
    expect(t.app.view.testId("late")).toBeVisible({ timeout: 1_000 });
  });

  const report = await run();
  assert.deepEqual(report.cases.map((item) => item.status), ["passed", "failed", "failed"]);
  assert.equal(report.cases[1].error.message, "boom");
  assert.match(report.cases[2].error.message, /returned while 1 fixture call was still running/);
  assert.ok(cleaned, "cleanup ran with a usable fixture");
});
