import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { expect, spec } from "../dist/index.js";
import { reset, run } from "../dist/runner.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

test("swipe, drag and fling send real paths at page points and record their delivery", async () => {
  const world = createWorld();
  world.add({ testId: "card", rect: { left: 100, top: 200, width: 50, height: 40 }, inViewport: true });
  spec("gestures", async (t) => {
    await t.app.view.swipe({ from: [10, 500], to: [10, 100], durationMs: 400 });
    await t.app.view.drag(t.app.view.testId("card"), { dx: 0, dy: 120 });
    await t.app.view.fling("up", 4000);
  });
  installFakeHost(world);
  const report = await run();
  assert.equal(report.cases[0].status, "passed", JSON.stringify(report.cases[0].error));
  const [swipe, drag, fling] = world.gestureCalls;
  assert.deepEqual(swipe.shape, { kind: "line", from: [10, 500], to: [10, 100], durationMs: 400, holdMs: 100 });
  assert.equal(swipe.window, undefined);
  assert.deepEqual(drag.shape.from, [125, 220]);
  assert.deepEqual(drag.shape.to, [125, 340]);
  assert.equal(drag.input, "touch");
  assert.deepEqual(fling.shape, { kind: "fling", from: [200, 600], direction: "up", velocity: 4000 });
  assert.deepEqual(report.cases[0].gestures.map((g) => [g.api, g.delivered]),
    [["swipe", "window"], ["drag", "window"], ["fling", "window"]]);
});

test("a fallback past the input dispatcher is visible in the result", async () => {
  const world = createWorld();
  world.gestureHost.delivered = "view";
  let delivered;
  spec("fallback", async (t) => {
    delivered = (await t.app.view.swipe({ from: [0, 0], to: [0, 10] })).delivered;
  });
  installFakeHost(world);
  const report = await run();
  assert.equal(delivered, "view");
  assert.equal(report.cases[0].gestures[0].delivered, "view");
});

test("pullToRefresh holds one finger, samples the indicator, and reports a refresh", async () => {
  const world = createWorld();
  let pulled = 0;
  let released = false;
  world.gestureHost.onPerform = (options) => {
    if (options.shape.kind === "line") pulled = options.shape.to[1] - 120;
    if (options.end === "up") released = true;
  };
  world.gestureHost.indicator = () => released ? { phase: "refreshing", offset: 64 }
    : pulled >= 120 ? { phase: "armed", offset: 90 } : pulled > 0 ? { phase: "pulling", offset: pulled * 0.6 } : { phase: "idle", offset: 0 };
  let result;
  spec("pull", async (t) => { result = await t.app.view.pullToRefresh(); });
  installFakeHost(world);
  const report = await run();
  assert.equal(report.cases[0].status, "passed", JSON.stringify(report.cases[0].error));
  assert.equal(result.refreshed, true);
  assert.deepEqual(result.phases, ["idle", "pulling", "armed", "refreshing"]);
  assert.equal(result.maxOffset, 90);
  const fingers = new Set(world.gestureCalls.map((call) => call.finger).filter(Boolean));
  assert.equal(fingers.size, 1, "one finger from down to up");
  assert.equal(world.gestureCalls[0].shape.kind, "hold");
  assert.deepEqual(world.gestureCalls[0].shape.at, [200, 120]);
  assert.equal(world.gestureCalls.at(-1).end, "up");
});

test("pullToRefresh on a page without an indicator rejects before any input", async () => {
  const world = createWorld();
  spec("no indicator", async (t) => {
    await t.reject(() => t.app.view.pullToRefresh(), { code: "E_UNSUPPORTED" });
  });
  installFakeHost(world);
  const report = await run();
  assert.equal(report.cases[0].status, "passed", JSON.stringify(report.cases[0].error));
  assert.equal(world.gestureCalls.length, 0);
});

test("a finger continues, then cannot move once lifted", async () => {
  const world = createWorld();
  spec("finger", async (t) => {
    const finger = await t.app.view.touch([50, 50]);
    await finger.moveTo([50, 150], { durationMs: 100 });
    await finger.up();
    expect(finger.at).toEqual([50, 150]);
    await t.reject(() => finger.moveTo([0, 0]), { message: /already lifted/ });
  });
  installFakeHost(world);
  const report = await run();
  assert.equal(report.cases[0].status, "passed", JSON.stringify(report.cases[0].error));
  assert.deepEqual(world.gestureCalls.map((call) => call.end), ["hold", "hold", "up"]);
});

test("requires.input skips with the reason where the host cannot imitate the device", async () => {
  const world = createWorld();
  world.gestureHost.inputs = ["mouse", "trackpad"];
  let ran = false;
  spec("touch only", { requires: { input: ["touch"] } }, async () => { ran = true; });
  installFakeHost(world);
  const report = await run();
  assert.equal(ran, false);
  assert.equal(report.cases[0].status, "skipped");
  assert.equal(report.cases[0].reason, "Not run: requires touch input (this host has mouse, trackpad).");
});

test("requires.input rejects an unknown device", () => {
  assert.throws(() => spec("bad", { requires: { input: ["pen"] } }, async () => {}), /requires.input/);
});

test("edge back reports E_UNSUPPORTED with the host's reason", async () => {
  const world = createWorld();
  spec("edge", async (t) => {
    await t.reject(() => t.app.gesture.edgeBack(), { code: "E_UNSUPPORTED", message: /system gesture/ });
  });
  installFakeHost(world);
  const report = await run();
  assert.equal(report.cases[0].status, "passed", JSON.stringify(report.cases[0].error));
});
