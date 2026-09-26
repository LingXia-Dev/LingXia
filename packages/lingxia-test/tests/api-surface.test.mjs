import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect, reset, run, AssertionError, TEST_ERROR_CODES, TimeoutError } from "../dist/index.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

test("t.expect(value) checks once; t.expect(fn) retries until the matcher passes", async () => {
  installFakeHost(createWorld());
  let reads = 0;
  let once;
  const started = Date.now();

  spec("ladder", { forensics: false }, async (t) => {
    await t.expect(() => ++reads, { timeout: 1_000, interval: 5 }).toBeGreaterThanOrEqual(3);
    t.expect(reads).toBe(3);
    try {
      t.expect(reads).toBe(4);
    } catch (error) {
      once = error;
    }
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(reads, 3, "the read ran until the matcher passed, then stopped");
  assert.ok(once instanceof AssertionError, "a once-check fails at once, without awaiting");
  assert.match(once.message, /Expected: 4\nReceived: 3/);
  assert.ok(Date.now() - started < 1_000);
});

test("t.expect(locator) retries; the deprecated t.expect.poll still works", async () => {
  const world = createWorld();
  const save = world.add({ testId: "save", visible: false });
  setTimeout(() => { save.visible = true; }, 30);
  installFakeHost(world);
  let polled = 0;

  spec("locator", { forensics: false }, async (t) => {
    await t.expect(t.app.view.testId("save")).toBeVisible({ timeout: 1_000 });
    await t.expect.poll(() => ++polled, { interval: 5 }).toBe(2);
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(polled, 2);
});

test("expect(locator) refuses, pointing at t.expect", async () => {
  const world = createWorld();
  world.add({ testId: "save" });
  installFakeHost(world);

  spec("trap", { forensics: false }, async (t) => {
    expect(t.app.view.testId("save")).toBeTruthy();
  });

  const report = await run();
  assert.equal(report.failed, 1);
  assert.equal(report.cases[0].error.name, "TypeError");
  assert.match(report.cases[0].error.message, /expect\(locator\) checks once and cannot read the element; use t\.expect\(locator\)/);
});

test("t.app.view has locators, not raw element methods; the page alias keeps 0.18's", async () => {
  const world = createWorld();
  const save = world.add({ testId: "save" });
  installFakeHost(world);
  const seen = {};

  spec("view", { forensics: false }, async (t) => {
    seen.viewClick = typeof t.app.view.click;
    seen.viewWaitFor = typeof t.app.view.waitFor;
    seen.viewQuery = typeof t.app.view.query;
    await t.app.view.testId("save").click();
    // Deprecated, still working.
    await t.app.page.click({ css: '[data-testid="save"]' });
    await t.app.page.testId("save").click();
    seen.shot = (await t.app.view.screenshot()).format;
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.deepEqual([seen.viewClick, seen.viewWaitFor, seen.viewQuery], ["undefined", "undefined", "undefined"]);
  assert.equal(save.clicked, 3);
  assert.equal(seen.shot, "png");
});

test("host tiers read lazily: the read never throws, the call rejects", async () => {
  const world = createWorld();
  const unavailable = () => { throw new Error("desktop automation is not built into this host"); };
  const lxapps = { async list() { return []; } };
  installFakeHost(world, { lxapps });
  const root = globalThis.lx.automation;
  globalThis.lx.automation = () => {
    const automation = root();
    Object.defineProperty(automation, "desktop", { get: unavailable });
    Object.defineProperty(automation, "terminal", { get: unavailable });
    automation.browser = { async tabs() { return [{ id: "t1" }]; } };
    return automation;
  };
  const seen = {};

  spec("tiers", { forensics: false }, async (t) => {
    seen.read = typeof t.automation.desktop;
    seen.nested = typeof t.automation.desktop.window.status;
    await t.reject(() => t.automation.desktop.window.status({ title: "x" }), { message: /not built into this host/ });
    await t.reject(() => t.automation.terminal.snapshot({}), { message: /not built into this host/ });
    seen.tabs = await t.automation.browser.tabs();
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(seen.read, "function");
  assert.equal(seen.nested, "function");
  assert.deepEqual(seen.tabs, [{ id: "t1" }]);
  const steps = report.cases[0].steps.map((step) => step.name);
  assert.ok(steps.includes("desktop.window.status"), steps.join(", "));
  assert.ok(steps.includes("browser.tabs"), steps.join(", "));
});

test("spec.configure sets file defaults; a spec's own options override them", async () => {
  const world = createWorld();
  installFakeHost(world);

  spec.configure({ timeout: 1_234, fresh: true, tags: ["routed"], covers: ["DEV-1"], forensics: false });
  spec.configure({ tags: ["smoke"] });
  spec("defaults", async () => {});
  spec("overrides", { timeout: 5_000, fresh: false, tags: ["own"], covers: ["DEV-2"] }, async () => {});

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  const [defaults, overrides] = report.cases;
  assert.equal(defaults.timeout_ms, 1_234);
  assert.deepEqual(defaults.tags, ["routed", "smoke"]);
  assert.deepEqual(defaults.covers, ["DEV-1"]);
  assert.equal(overrides.timeout_ms, 5_000);
  assert.deepEqual(overrides.tags, ["routed", "smoke", "own"]);
  assert.deepEqual(overrides.covers, ["DEV-1", "DEV-2"]);
  // `fresh` from the file relaunched the first spec only.
  assert.equal(world.navCalls.filter(([name]) => name === "relaunch").length, 1);
});

test("spec.configure rejects an id and malformed options at registration", () => {
  assert.throws(() => spec.configure({ id: "shared" }), /cannot set `id`/);
  assert.throws(() => spec.configure({ timeout: -1 }), /timeout must be a positive finite number/);
  assert.throws(() => spec.configure({ requires: { args: "PASSWORD" } }), /requires\.args must be an array/);
  assert.throws(() => spec("bad", { requires: { openapi: "yes" } }, async () => {}), /requires\.openapi must be a boolean/);
});

test("requires skips a spec whose run inputs are missing, naming what to pass", async () => {
  installFakeHost(createWorld(), { args: { ACCOUNT: "qa" }, control: {} });
  let ran = 0;

  spec.configure({ requires: { args: ["ACCOUNT"] } });
  spec("has its args", async () => { ran += 1; });
  spec("needs a password", { requires: { args: ["PASSWORD", "OTP"] } }, async () => { ran += 1; });
  spec("needs a contract", { requires: { openapi: true } }, async () => { ran += 1; });

  const report = await run();
  assert.equal(ran, 1);
  assert.deepEqual(report.cases.map((item) => item.status), ["passed", "skipped", "skipped"]);
  assert.equal(report.cases[1].reason,
    "Not run: requires --arg PASSWORD=<value>, --arg OTP=<value> (or --secret-arg).");
  assert.equal(report.cases[2].reason, "Not run: requires --openapi <document>.");
});

test("error codes: TimeoutError is E_TIMEOUT and spec.fail can pin it", async () => {
  installFakeHost(createWorld());

  spec.fail("waits in vain", { expected: { code: "E_TIMEOUT" }, forensics: false }, async (t) => {
    await t.waitFor(() => false, { timeout: 30, interval: 5 });
  });

  const report = await run();
  assert.equal(report.cases[0].status, "xfail", JSON.stringify(report.cases[0].error));
  assert.equal(new TimeoutError("x").code, "E_TIMEOUT");
  for (const code of ["E_TIMEOUT", "E_SKIPPED", "E_OPENAPI_CONTRACT", "E_PAGE_NOT_ACTIVE", "E_CLOCK_INSTALLED"]) {
    assert.ok(TEST_ERROR_CODES.includes(code), code);
  }
});

test("a dropped trace event never fails the action it describes", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  const host = globalThis.__LINGXIA_AUTOMATION_HOST__;
  const emit = host.emit;
  let dropped = 0;
  host.emit = (event) => {
    if (event.type === "step_started" && dropped < 3) {
      dropped += 1;
      throw new Error("Resource temporarily unavailable (os error 35)");
    }
    return emit(event);
  };

  spec("traced", { forensics: false }, async (t) => {
    await t.app.info();
    await t.app.pages();
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(dropped, 3);
  assert.deepEqual(report.cases[0].steps.map((step) => step.name), ["app.info", "app.pages"]);
  assert.ok(events.some((event) => event.type === "step_finished" && event.name === "app.info"));
});

test("an idempotent read the transport dropped is retried; input is not", async () => {
  const world = createWorld();
  const save = world.add({ testId: "save" });
  installFakeHost(world);
  const current = world.app.nav.current;
  let currentFailures = 1;
  world.app.nav.current = async () => {
    if (currentFailures-- > 0) throw new Error("channel closed");
    return current();
  };
  const click = world.app.page.click;
  let clickFailures = 1;
  world.app.page.click = async (options) => {
    if (clickFailures-- > 0) throw new Error("connection reset by peer (os error 54)");
    return click(options);
  };

  spec("reads", { forensics: false }, async (t) => {
    const page = await t.app.nav.current();
    assert.equal(page.name, "home");
    await t.reject(() => t.app.view.testId("save").click(), { message: /connection reset/ });
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(save.clicked, undefined, "a click the transport lost is never resent");
});
