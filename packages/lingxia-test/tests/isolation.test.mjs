import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { spec, rawAutomation, expect } from "../dist/index.js";
import { reset, run } from "../dist/runner.js";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";

const realFetch = globalThis.fetch;
afterEach(() => {
  reset();
  delete globalThis.lx;
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  globalThis.fetch = realFetch;
});

const tick = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms));

/** A host route table: `failUnroute` makes removal reject. */
function fakeNetwork() {
  const routes = new Map();
  let nextId = 0;
  const network = {
    routes,
    failUnroute: undefined,
    async route(pattern) {
      const id = ++nextId;
      routes.set(id, { id, pattern });
      return {
        id,
        pattern: String(pattern),
        async unroute() {
          if (network.failUnroute) throw network.failUnroute;
          return routes.delete(id);
        },
        async requests() { return []; },
        async requestsAfter() { return { requests: [], droppedThrough: 0 }; },
      };
    },
    async unrouteAll() { const count = routes.size; routes.clear(); return count; },
    async requests() { return []; },
  };
  return network;
}

/** Resolve `release` when the run reports it stopped: code resumes during what would be the next spec. */
function releaseWhenStopped(host, release) {
  const emit = globalThis.__LINGXIA_AUTOMATION_HOST__.emit;
  globalThis.__LINGXIA_AUTOMATION_HOST__.emit = (event) => {
    emit(event);
    if (event.type === "diagnostic" && event.phase === "run_stopped") release();
  };
}

test("a body that resumes late cannot act: its raw calls are refused, its assertion belongs to no case, and no later body runs", async () => {
  const world = createWorld();
  const host = installFakeHost(world, { attempts: true });
  let release;
  const late = new Promise((resolve) => { release = resolve; });
  releaseWhenStopped(host, release);
  const outcomes = {};
  let shared = false;
  let followingRan = false;

  spec("old body", { timeout: 20, forensics: false }, async () => {
    // A handle taken before the timeout, and fresh ones taken after it.
    const nav = rawAutomation().lxapp().nav;
    await late;
    shared = true;
    for (const [name, act] of [
      ["captured", () => nav.relaunch({ page: "zombie-captured" })],
      ["fresh", () => rawAutomation().lxapp().nav.relaunch({ page: "zombie-fresh" })],
      ["global", () => globalThis.lx.automation().lxapp().nav.relaunch({ page: "zombie-global" })],
    ]) {
      try { await act(); outcomes[name] = "acted"; } catch (error) { outcomes[name] = error.code ?? error.message; }
    }
    expect("late assertion from old body").toBe("expected");
  });
  spec("following body", { forensics: false }, async () => {
    followingRan = true;
    if (shared) throw new Error("saw the old body's write");
  });

  const report = await run();
  // The old body finishes after the run: let it.
  for (let i = 0; i < 5; i += 1) await tick(5);

  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "skipped"]);
  assert.equal(report.partial, true);
  assert.equal(followingRan, false);
  assert.equal(shared, true, "the old body did resume");
  assert.deepEqual(outcomes, { captured: "E_AUTOMATION_PRIVILEGE", fresh: "E_AUTOMATION_PRIVILEGE", global: "E_AUTOMATION_PRIVILEGE" });
  assert.ok(!world.navCalls.some(([, options]) => String(options?.page).startsWith("zombie")), JSON.stringify(world.navCalls));
  for (const record of report.cases) {
    assert.ok(!JSON.stringify(record.assertions).includes("late assertion"), `${record.title} holds the late assertion`);
  }
  const note = host.events.find((e) => e.type === "diagnostic" && e.phase === "late_assertion");
  assert.ok(note, "the late assertion is noted");
  assert.match(note.message, /no case records it: expect\(received\)\.toBe failed/);
  assert.match(host.attempts.revoked, /"old body" left its timed-out body pending/);
});

test("a driver an abandoned body kept is refused by the host after the revoke", async () => {
  const world = createWorld();
  const lxapps = { async list() { return [{ appid: "demo-app", status: "opened" }]; }, async open() { return {}; } };
  const host = installFakeHost(world, { attempts: true, lxapps });
  let release;
  const late = new Promise((resolve) => { release = resolve; });
  releaseWhenStopped(host, release);
  let outcome;

  spec("keeps a host tier", { timeout: 20, forensics: false }, async () => {
    const manager = rawAutomation().lxapps;
    await late;
    try { await manager.open({ appid: "other-app" }); outcome = "acted"; } catch (error) { outcome = error.code; }
  });
  spec("not run", async () => {});

  await run();
  for (let i = 0; i < 5; i += 1) await tick(5);
  assert.equal(outcome, "E_AUTOMATION_PRIVILEGE");
  assert.ok(host.attempts.refused.includes("lxapps.open"));
});

test("a route installed by a body that never settles is removed before the run stops, and a late re-install is refused", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  const host = installFakeHost(world, { attempts: true });
  let release;
  const late = new Promise((resolve) => { release = resolve; });
  releaseWhenStopped(host, release);
  const reinstall = {};

  spec("routes, then hangs", { timeout: 20, forensics: false }, async (t) => {
    await t.app.network.route("/v1/devices", { status: 500 });
    await late;
    try { await t.app.network.route("/v1/again", { status: 500 }); reinstall.fixture = "installed"; } catch (error) { reinstall.fixture = error.message; }
    try { await rawAutomation().lxapp().network.route("/v1/raw", { status: 500 }); reinstall.raw = "installed"; } catch (error) { reinstall.raw = error.code; }
  });
  spec("would use the real backend", async () => {});

  const report = await run();
  for (let i = 0; i < 5; i += 1) await tick(5);
  assert.deepEqual(report.cases.map((c) => c.status), ["timeout", "skipped"]);
  assert.equal(report.partial, true);
  assert.equal(network.routes.size, 0, "no route outlives its spec");
  assert.match(reinstall.fixture, /closed/);
  assert.equal(reinstall.raw, "E_AUTOMATION_PRIVILEGE");
});

test("routes a spec installed through the raw driver are removed by the host when the spec ends", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  const host = installFakeHost(world, { attempts: true });
  let seen;

  spec("installs through the raw driver", async () => {
    await rawAutomation().lxapp().network.route("**", { status: 503 });
  });
  spec("expects the real backend", async () => { seen = network.routes.size; });

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["passed", "passed"]);
  assert.equal(seen, 0);
  assert.equal(host.attempts.open, undefined);
  assert.equal(host.attempts.installs.length, 0);
});

test("a route that cannot be removed fails its spec and stops the run instead of leaking into the next", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  const { events } = installFakeHost(world);
  let secondRan = false;

  spec("routes", async (t) => {
    await t.app.network.route("/v1/devices", { status: 500 });
    network.failUnroute = new Error("route table unreachable");
  });
  spec("would see the stale route", async () => { secondRan = true; });

  const report = await run();
  assert.equal(secondRan, false);
  assert.equal(report.partial, true);
  assert.deepEqual(report.cases.map((c) => c.status), ["failed", "skipped"]);
  assert.equal(report.cases[0].error.phase, "defer");
  assert.match(report.cases[0].error.message, /network routes were not removed \(\/v1\/devices: route table unreachable\)/);
  assert.match(report.cases[1].reason, /^Not run: "routes" left test resources the run could not remove \(network routes were not removed/);
  assert.ok(events.some((e) => e.type === "diagnostic" && e.phase === "run_stopped"));
});

test("a route that is already gone counts as removed", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);

  spec("removes its own route", async (t) => {
    const route = await t.app.network.route("/v1/devices", { status: 500 });
    await route.remove();
  });
  spec("next", async () => {});

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["passed", "passed"]);
  assert.equal(report.partial, false);
});

test("the host failing to remove what an attempt installed fails the spec and stops the run", async () => {
  const world = createWorld();
  const host = installFakeHost(world, { attempts: true });
  let secondRan = false;
  spec("installs", async () => { host.attempts.failSweep = new Error("companion unreachable"); });
  spec("next", async () => { secondRan = true; });

  const report = await run();
  assert.equal(secondRan, false);
  assert.equal(report.partial, true);
  assert.equal(report.cases[0].status, "failed");
  assert.match(report.cases[0].error.message, /the host could not remove what the spec installed: companion unreachable/);
});

test("a scenario and a clock whose removal fails are cleanup errors, not swallowed", async () => {
  const world = createWorld();
  world.app.mock = {
    async reset() { return { generation: 1, function: null }; },
    async use() {
      return { name: "s", variant: undefined, rules: [], async unroute() { throw new Error("scenario store gone"); }, async calls() { return []; } };
    },
  };
  world.app.clock = {
    async install() { return { now: 0, pending: 0 }; },
    async uninstall() { throw new Error("Logic did not answer"); },
  };
  installFakeHost(world);
  let secondRan = false;

  spec("mocks and fakes time", async (t) => {
    await t.app.mock.use({ name: "s", rules: [] });
    await t.app.clock.install();
  });
  spec("next", async () => { secondRan = true; });

  const report = await run();
  assert.equal(secondRan, false);
  assert.equal(report.partial, true);
  const message = report.cases[0].error.message;
  assert.match(message, /mock scenario s was not removed: scenario store gone/);
  assert.match(message, /test clocks were not uninstalled \(demo-app: Logic did not answer\)/);
});

test("a body that returned with a fetch still in flight fails and stops the run when it does not settle", async () => {
  const world = createWorld();
  installFakeHost(world);
  globalThis.fetch = () => new Promise(() => {});
  let secondRan = false;
  spec("fires and forgets", async () => { void fetch("https://backend.example.test/forever"); });
  spec("next", async () => { secondRan = true; });

  const report = await run();
  assert.equal(secondRan, false);
  assert.equal(report.partial, true);
  assert.equal(report.cases[0].status, "failed");
  assert.match(report.cases[0].error.message, /ended while fetch GET https:\/\/backend\.example\.test\/forever .* was still running, and it did not settle within \d+ms/);
});

test("a body's leftover timer is cancelled at its end and never fires into the next spec", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  let fired = false;
  spec("leaves a timer", async () => { setTimeout(() => { fired = true; }, 30); });
  spec("next", async () => { await tick(60); });

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["passed", "passed"]);
  assert.equal(fired, false);
  const note = events.find((e) => e.type === "diagnostic" && e.phase === "cleanup");
  assert.match(note.message, /^"leaves a timer" ended with 1 test-context timer pending; cancelled so it never fires into a later spec\.$/);
});

test("a run started before the host registered its current lxapp waits for it, then pins specs to it by id", async () => {
  const world = createWorld();
  const host = installFakeHost(world, { current: false });
  setTimeout(() => host.setCurrent(true), 300);
  let seen;

  spec("runs once the app is back", async (t) => {
    // The host loses track of "current" again: the spec's app is pinned by id.
    host.setCurrent(false);
    seen = (await t.app.info()).appid;
  });
  spec("still pinned", async (t) => { await t.app.info(); });

  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.status), ["passed", "passed"]);
  assert.equal(seen, "demo-app");
  assert.equal(report.meta.subject.appid, "demo-app");
});
