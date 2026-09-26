import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { spec, rawAutomation, reset, run } from "../dist/index.js";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";

const realFetch = globalThis.fetch;
afterEach(() => {
  reset();
  delete globalThis.lx;
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  globalThis.fetch = realFetch;
});

/** A test-context `fetch` whose requests never answer. */
function hangingFetch() {
  globalThis.fetch = () => new Promise(() => {});
}

test("a spec that times out with work pending fails alone; the app is recovered and the next spec runs", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  hangingFetch();
  let ticks = 0;
  let nextRan = false;

  spec("leaves work pending", { timeout: 40, forensics: false }, async (t) => {
    t.defer(() => {});
    setInterval(() => { ticks += 1; }, 100);
    void fetch("https://backend.example.test/slow");
    await new Promise(() => {});
  });
  spec("runs after it", async (t) => {
    nextRan = true;
    await t.app.info();
  });

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "passed"]);
  assert.equal(nextRan, true);
  assert.equal(report.partial, false);
  const timedOut = report.cases[0];
  assert.match(timedOut.error.message, /spec timed out after 40ms/);
  assert.match(timedOut.error.message, /did not settle; still pending: .*interval setInterval 100ms/);
  assert.match(timedOut.error.message, /fetch GET https:\/\/backend\.example\.test\/slow/);
  assert.match(timedOut.error.message, /Cleanup skipped because the timed-out body is still running/);

  const recovery = events.filter((e) => e.type === "diagnostic" && e.phase === "recovery");
  assert.equal(recovery.length, 1, JSON.stringify(events.filter((e) => e.type === "diagnostic")));
  assert.match(recovery[0].message,
    /^"leaves work pending" left its timed-out body pending: fetch GET https:\/\/backend\.example\.test\/slow \(started \d+ms into the spec\), interval setInterval 100ms .*; cancelled its 1 pending timer\. Relaunched the app under test on its home page; the run continues\.$/);
  assert.ok(world.navCalls.some(([method, options]) => method === "relaunch" && options.page === "home"));

  // The abandoned spec's poll was cancelled, so it never fires into later specs.
  const after = ticks;
  await new Promise((resolve) => setTimeout(resolve, 250));
  assert.equal(ticks, after);
});

test("a body awaiting nothing the runtime tracks says so", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  spec("awaits forever", { timeout: 20, forensics: false }, () => new Promise(() => {}));
  spec("still runs", async () => {});

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "passed"]);
  const recovery = events.find((e) => e.type === "diagnostic" && e.phase === "recovery");
  assert.match(recovery.message, /"awaits forever" left its timed-out body pending: an awaited promise that no timer, fetch or fixture call of the spec backs/);
});

test("a hung raw-driver eval is named as the pending work", async () => {
  const world = createWorld();
  world.setEval("hangs forever", new Promise(() => {}));
  const { events } = installFakeHost(world);
  spec("hung eval", { timeout: 30, forensics: false }, async () => {
    await rawAutomation().lxapp().eval({ script: "hangs forever" });
  });
  spec("still runs", async () => {});

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "passed"]);
  const recovery = events.find((e) => e.type === "diagnostic" && e.phase === "recovery");
  assert.match(recovery.message, /^"hung eval" left its timed-out body pending: eval rawAutomation\(\)\.lxapp\(\)\.eval\(\) \(started \d+ms into the spec\)\./);
});

test("when recovery fails, the rest are not run and the reason names the stuck work and the fix", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  hangingFetch();
  let laterRan = false;

  spec("stuck on fetch", { timeout: 30, forensics: false }, async () => {
    world.failRelaunch(new Error("home page never became ready"));
    await fetch("https://backend.example.test/hang");
  });
  spec("would run on a wedged app", async () => { laterRan = true; });
  spec("so would this", async () => { laterRan = true; });

  const report = await run();
  assert.equal(laterRan, false);
  assert.equal(report.partial, true);
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "skipped", "skipped"]);
  for (const skipped of report.cases.slice(1)) {
    assert.match(skipped.reason,
      /^Not run: "stuck on fetch" left its timed-out body pending: fetch GET https:\/\/backend\.example\.test\/hang .*, and recovering the app failed: home page never became ready\. Recover: lxdev lxapp restart$/);
  }
  const failed = events.filter((e) => e.type === "diagnostic" && e.phase === "recovery_failed");
  assert.equal(failed.length, 1);
  assert.match(failed[0].message, /The remaining specs are not run\. Recover: lxdev lxapp restart/);
});

test("the runner's own timers are not reported as spec work", async () => {
  const world = createWorld();
  world.add({ testId: "late", visible: false });
  const { events } = installFakeHost(world);
  spec("polls, then hangs", { timeout: 300, forensics: false }, async (t) => {
    await t.expect(t.app.view.testId("late")).toBeVisible({ timeout: 50 }).catch(() => {});
    await new Promise(() => {});
  });
  spec("next", async () => {});

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "passed"]);
  const recovery = events.find((e) => e.type === "diagnostic" && e.phase === "recovery");
  assert.match(recovery.message, /pending: an awaited promise that no timer/);
});
