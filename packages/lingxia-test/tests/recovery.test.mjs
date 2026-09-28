import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { spec, expect } from "../dist/index.js";
import { rawAutomation, reset, run } from "../dist/runner.js";
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

test("a spec abandoned with a fetch still in flight stops the run: the app is relaunched for inspection and the rest are not run", async () => {
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
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "skipped"]);
  assert.equal(nextRan, false);
  assert.equal(report.partial, true);
  const timedOut = report.cases[0];
  assert.match(timedOut.error.message, /spec timed out after 40ms/);
  assert.match(timedOut.error.message, /did not settle; still pending: .*interval setInterval 100ms/);
  assert.match(timedOut.error.message, /fetch GET https:\/\/backend\.example\.test\/slow/);
  assert.match(timedOut.error.message, /Cleanup skipped because the timed-out body is still running/);

  const stopped = events.filter((e) => e.type === "diagnostic" && e.phase === "run_stopped");
  assert.equal(stopped.length, 1, JSON.stringify(events.filter((e) => e.type === "diagnostic")));
  assert.match(stopped[0].message,
    /^"leaves work pending" left its timed-out body pending: fetch GET https:\/\/backend\.example\.test\/slow \(started \d+ms into the spec\), interval setInterval 100ms .*; cancelled its 1 pending timer\. The run stopped: abandoned code can resume through an untracked promise in this shared JS context; fetch GET https:\/\/backend\.example\.test\/slow \(started \d+ms into the spec\) is still running after 2000ms and would resume its code during a later spec\. Relaunched the app under test on its home page for inspection\. Make the spec settle \(await its work\), or give it a longer timeout\. The remaining specs are not run\.$/);
  assert.match(report.cases[1].reason, /^Not run: "leaves work pending" left its timed-out body pending: .*The run stopped: abandoned code can resume through an untracked promise in this shared JS context; fetch GET/);
  assert.ok(world.navCalls.some(([method, options]) => method === "relaunch" && options.page === "home"));

  // The abandoned spec's poll was cancelled, so it never fires into later specs.
  const after = ticks;
  await new Promise((resolve) => setTimeout(resolve, 250));
  assert.equal(ticks, after);
});

test("an abandoned body stops the shared context even when no tracked work remains", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  let ticks = 0;
  let nextRan = false;
  spec("awaits forever", { timeout: 20, forensics: false }, async (t) => {
    t.defer(() => {});
    setInterval(() => { ticks += 1; }, 30);
    await new Promise(() => {});
  });
  spec("runs after it", async (t) => {
    nextRan = true;
    await t.app.info();
  });

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "skipped"]);
  assert.equal(nextRan, false);
  assert.equal(report.partial, true);
  assert.equal(events.filter((e) => e.type === "diagnostic" && e.phase === "run_stopped").length, 1);
  const recovered = events.find((e) => e.type === "diagnostic" && e.phase === "run_stopped");
  assert.match(recovered.message, /abandoned code can resume through an untracked promise/);
  assert.ok(world.navCalls.some(([method, options]) => method === "relaunch" && options.page === "home"));
  const after = ticks;
  await new Promise((resolve) => setTimeout(resolve, 100));
  assert.equal(ticks, after, "the abandoned spec's poll never fires into later specs");
});

test("a body awaiting nothing the runtime tracks says so", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  spec("awaits forever", { timeout: 20, forensics: false }, () => new Promise(() => {}));
  spec("runs next", async () => {});

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "skipped"]);
  const recovered = events.find((e) => e.type === "diagnostic" && e.phase === "run_stopped");
  assert.match(recovered.message, /"awaits forever" left its timed-out body pending: an awaited promise that no timer, fetch or fixture call of the spec backs/);
});

test("a hung raw-driver eval cannot be taken back: it is named and the run stops", async () => {
  const world = createWorld();
  world.setEval("hangs forever", new Promise(() => {}));
  const { events } = installFakeHost(world);
  spec("hung eval", { timeout: 30, forensics: false }, async () => {
    await rawAutomation().lxapp().eval({ script: "hangs forever" });
  });
  spec("is not run", async () => {});

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "skipped"]);
  const stopped = events.find((e) => e.type === "diagnostic" && e.phase === "run_stopped");
  assert.match(stopped.message, /^"hung eval" left its timed-out body pending: eval rawAutomation\(\)\.lxapp\(\)\.eval\(\) \(started \d+ms into the spec\)\. The run stopped: .*eval rawAutomation\(\)\.lxapp\(\)\.eval\(\) .* is still running after 2000ms/);
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
      /^Not run: "stuck on fetch" left its timed-out body pending: fetch GET https:\/\/backend\.example\.test\/hang .*The run stopped: .*still running .*\. Recovering the app failed: home page never became ready\. .* Recover: lxdev lxapp restart$/);
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
    await expect(t.app.view.testId("late")).toBeVisible({ timeout: 50 }).catch(() => {});
    await new Promise(() => {});
  });
  spec("next", async () => {});

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "skipped"]);
  const recovered = events.find((e) => e.type === "diagnostic" && e.phase === "run_stopped");
  assert.match(recovered.message, /pending: an awaited promise that no timer/);
});

/** A Rong host object: callable, with its methods on its prototype only. */
function hostObject(name, methods, properties = {}) {
  const proto = Object.create(Function.prototype);
  for (const method of methods) proto[method] = function () { return name; };
  Object.defineProperty(proto, Symbol.toStringTag, { value: name });
  const object = Object.setPrototypeOf(function () {}, proto);
  for (const [key, value] of Object.entries(properties)) {
    Object.defineProperty(proto, key, { get() { return value; } });
  }
  return object;
}

test("a live spec reads every host tier through rawAutomation() with its full surface; an abandoned spec's root refuses", async () => {
  const surfaces = {
    shell: ["pins", "setPin", "reorderPins"],
    terminal: ["input", "newTab", "setMaximized", "snapshot", "split"],
    lxapps: ["applink", "close", "current", "list", "open", "restart", "screenshot", "uninstall", "windows"],
    device: ["get", "list", "set"],
    browser: ["activate", "back", "click", "close", "current", "eval", "open", "tabs"],
    desktop: ["displays", "doctor", "screenshot", "snapshot", "windows"],
  };
  const desktopPointer = hostObject("DesktopPointer", ["click", "move"]);
  const hostTiers = Object.fromEntries(Object.entries(surfaces).map(([name, members]) => [
    name,
    hostObject(name, members, name === "desktop" ? { pointer: desktopPointer } : name === "browser" ? { cookies: hostObject("BrowserCookies", ["get", "set"]) } : {}),
  ]));
  installFakeHost(createWorld(), { hostTiers });
  // The surface test's own reads: property reads and `typeof` per member.
  const readMember = (record, name) => { try { return { unbuilt: false, value: record[name] }; } catch { return { unbuilt: true, value: undefined }; } };
  const inspect = (target, members) => members
    .map((name) => ({ name, ...readMember(target, name) }))
    .filter((member) => member.unbuilt || typeof member.value !== "function")
    .map((member) => member.name);
  const seen = {};
  let kept;
  let lateRead;
  let release;
  const late = new Promise((resolve) => { release = resolve; });

  spec("reads the surface", async () => {
    for (const [name, members] of Object.entries(surfaces)) {
      const tier = rawAutomation()[name];
      seen[name] = { same: tier === hostTiers[name], broken: inspect(tier, members) };
    }
    seen.pointer = inspect(rawAutomation().desktop.pointer, ["click", "move"]);
    seen.cookies = inspect(rawAutomation().browser.cookies, ["get", "set"]);
    seen.lxapp = typeof rawAutomation().lxapp().eval;
  });
  spec("abandoned", { timeout: 20, forensics: false }, async () => {
    kept = rawAutomation();
    await late;
    try { void kept.shell; lateRead = "read"; } catch (error) { lateRead = error.code; }
  });
  spec("after it", async () => { release(); });

  const report = await run();
  for (const [name] of Object.entries(surfaces)) {
    assert.deepEqual(seen[name], { same: true, broken: [] }, name);
  }
  assert.deepEqual(seen.pointer, []);
  assert.deepEqual(seen.cookies, []);
  assert.equal(seen.lxapp, "function");
  assert.deepEqual(report.cases.map((c) => c.status), ["passed", "timeout", "skipped"]);
  release();
  for (let i = 0; i < 5 && lateRead === undefined; i += 1) await new Promise((resolve) => setTimeout(resolve, 5));
  assert.equal(lateRead, "E_AUTOMATION_PRIVILEGE", "the abandoned spec's handle refuses");
});

test("rawAutomation hands the host tiers out as the native objects", () => {
  const lxapps = { async list() { return []; } };
  installFakeHost(createWorld(), { lxapps });
  const root = rawAutomation();
  assert.equal(root.lxapps, lxapps, "a host tier is not wrapped");
  assert.equal(typeof root.lxapp, "function");
  assert.equal(typeof root.lxapp().eval, "function");
});
