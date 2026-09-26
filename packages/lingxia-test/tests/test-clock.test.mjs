import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset, run } from "../dist/index.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

/** Mirrors the host driver: one clock per app, pending timers counted. */
function fakeClock(world) {
  const clock = {
    installed: false,
    now: 0,
    pending: 0,
    calls: [],
    async install(options) {
      clock.calls.push(["install", options]);
      if (clock.installed) {
        throw Object.assign(new Error("the test clock is already installed"), { code: "E_CLOCK_INSTALLED" });
      }
      clock.installed = true;
      clock.now = options?.now ?? 1_000;
      return clock.legacy ? clock.now : { now: clock.now, pending: 0 };
    },
    async tick(ms) {
      clock.calls.push(["tick", ms]);
      if (!clock.installed) {
        throw Object.assign(new Error("the test clock is not installed"), { code: "E_CLOCK_NOT_INSTALLED" });
      }
      clock.now += ms;
      return { now: clock.now, fired: 1, pending: clock.pending };
    },
    async runAll(options) {
      clock.calls.push(["runAll", options]);
      return { now: clock.now, fired: 0, pending: 0 };
    },
    async setSystemTime(time) {
      clock.calls.push(["setSystemTime", time]);
      clock.now = typeof time === "string" ? Date.parse(time) : time;
      return clock.legacy ? clock.now : { now: clock.now, pending: clock.pending };
    },
    async uninstall() {
      clock.calls.push(["uninstall"]);
      const result = { uninstalled: clock.installed, dropped: clock.installed ? clock.pending : 0 };
      clock.installed = false;
      return result;
    },
  };
  world.app.clock = clock;
  return clock;
}

test("t.app.clock is traced and uninstalled when the spec ends", async () => {
  const world = createWorld();
  const clock = fakeClock(world);
  installFakeHost(world);
  let ticked;
  let installed;
  let set;
  let ran;

  spec("polls", async (t) => {
    installed = await t.app.clock.install({ now: new Date("2030-01-01T00:00:00Z") });
    ticked = await t.app.clock.tick(3_000);
    set = await t.app.clock.setSystemTime("2031-01-01T00:00:00Z");
    ran = await t.app.clock.runAll({ maxTimers: 5 });
  });
  spec("next", async () => {});

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  // A Date crosses as epoch milliseconds; strings pass through for Logic to parse.
  assert.deepEqual(clock.calls[0], ["install", { now: Date.UTC(2030, 0, 1) }]);
  assert.deepEqual(clock.calls[2], ["setSystemTime", "2031-01-01T00:00:00Z"]);
  // ClockState from every call; tick and runAll add what they fired.
  assert.deepEqual(installed, { now: Date.UTC(2030, 0, 1), pending: 0 });
  assert.deepEqual(ticked, { now: Date.UTC(2030, 0, 1) + 3_000, fired: 1, pending: 0 });
  assert.deepEqual(set, { now: Date.UTC(2031, 0, 1), pending: 0 });
  assert.deepEqual(ran, { now: Date.UTC(2031, 0, 1), fired: 0, pending: 0 });
  assert.deepEqual(clock.calls.at(-1), ["uninstall"], "the spec's clock is removed at its end");
  assert.equal(clock.installed, false);
  const traced = report.cases[0].steps.map((step) => `${step.name} ${step.detail ?? ""}`.trim());
  assert.deepEqual(traced, [
    "clock.install 2030-01-01T00:00:00.000Z",
    "clock.tick 3000ms",
    "clock.setSystemTime 2031-01-01T00:00:00Z",
    "clock.runAll max 5",
  ]);
  assert.equal(world.navCalls.filter(([name]) => name === "relaunch").length, 0, "nothing was dropped");
});

test("dropping pending test timers relaunches the next spec", async () => {
  const world = createWorld();
  const clock = fakeClock(world);
  const { events } = installFakeHost(world);

  spec("leaves a poll pending", async (t) => {
    await t.app.clock.install();
    clock.pending = 1;
  });
  spec("starts clean", async () => {});

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.deepEqual(clock.calls.map(([name]) => name), ["install", "uninstall"]);
  assert.equal(world.navCalls.filter(([name]) => name === "relaunch").length, 1);
  const notes = events.filter((event) => event.type === "diagnostic" && event.phase === "clock");
  assert.equal(notes.length, 1);
  assert.match(notes[0].message, /uninstalled the test clock of .* and dropped 1 pending test timer; they never fired/);
});

test("uninstall resolves nothing and notes the timers it dropped", async () => {
  const world = createWorld();
  const clock = fakeClock(world);
  const { events } = installFakeHost(world);
  let result = "not called";

  spec("drops two", async (t) => {
    await t.app.clock.install();
    clock.pending = 2;
    result = await t.app.clock.uninstall();
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(result, undefined);
  const notes = events.filter((event) => event.type === "diagnostic" && event.phase === "clock");
  assert.deepEqual(notes.map((event) => event.message), ["clock.uninstall dropped 2 pending test timers; they never fired"]);
});

test("a host that resolves a bare time still yields a ClockState", async () => {
  const world = createWorld();
  const clock = fakeClock(world);
  clock.legacy = true;
  installFakeHost(world);
  let installed;

  spec("older host", async (t) => {
    installed = await t.app.clock.install({ now: 5 });
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.deepEqual(installed, { now: 5, pending: 0 });
});

test("reading t.app.clock never throws; a host without it fails the call", async () => {
  const world = createWorld();
  installFakeHost(world);
  let read;

  spec("old host", async (t) => {
    read = t.app.clock;
    await t.reject(() => t.app.clock.install(), { message: /not supported by this host/ });
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(typeof read.tick, "function");
});

test("clock driver failures keep their codes", async () => {
  const world = createWorld();
  fakeClock(world);
  installFakeHost(world);

  spec("twice", async (t) => {
    await t.reject(() => t.app.clock.tick(10), { code: "E_CLOCK_NOT_INSTALLED" });
    await t.app.clock.install();
    await t.reject(() => t.app.clock.install(), { code: "E_CLOCK_INSTALLED" });
    const removed = await t.app.clock.uninstall();
    assert.equal(removed, undefined);
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
});
