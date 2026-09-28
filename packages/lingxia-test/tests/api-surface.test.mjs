import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect, AssertionError, TEST_ERROR_CODES, TimeoutError } from "../dist/index.js";
import { reset, run } from "../dist/runner.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

test("expect(value) checks once; expect.poll(fn) retries until the matcher passes", async () => {
  installFakeHost(createWorld());
  let reads = 0;
  let once;
  const started = Date.now();

  spec("ladder", { forensics: false }, async (t) => {
    await expect.poll(() => ++reads, { timeout: 1_000, interval: 5 }).toBeGreaterThanOrEqual(3);
    expect(reads).toBe(3);
    try {
      expect(reads).toBe(4);
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

test("expect(locator) retries; the fixture has no expect of its own", async () => {
  const world = createWorld();
  const save = world.add({ testId: "save", visible: false });
  setTimeout(() => { save.visible = true; }, 30);
  installFakeHost(world);
  let own;

  spec("locator", { forensics: false }, async (t) => {
    await expect(t.app.view.testId("save")).toBeVisible({ timeout: 1_000 });
    own = "expect" in t;
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(own, false);
});

test("expect(promise) refuses: await it, or poll a read", async () => {
  installFakeHost(createWorld());

  spec("promise", { forensics: false }, async (t) => {
    expect(t.app.info()).toBeTruthy();
  });

  const report = await run();
  assert.equal(report.failed, 1);
  assert.equal(report.cases[0].error.name, "TypeError");
  assert.match(report.cases[0].error.message, /expect\(promise\): await the value first, or retry a read with expect\.poll/);
});

test("expect(locator) and expect.poll need a running spec; a value check does not", () => {
  const locator = { [Symbol.for("lingxia.test.locator")]: true, selector: "#x" };
  assert.throws(() => expect(locator), /expect\(locator\) retries inside a spec's budget/);
  assert.throws(() => expect.poll(() => 1), /expect\.poll\(read\) retries inside a spec's budget/);
  assert.throws(() => expect.poll(1), /expect\.poll\(read\) takes a function/);
  expect([1, 2]).toHaveLength(2);
  expect("abc").not.toHaveLength(2);
  assert.throws(() => expect(3).toHaveLength(1), /received value must have a numeric length/);
  assert.throws(() => expect(() => 1).toBe(1),
    /expect\(fn\)\.toBe: a function is only called by toThrow; to retry a read until it passes, use expect\.poll\(read\)/);
  assert.throws(() => expect(() => 1).not.toBeTruthy(), /expect\(fn\)\.toBeTruthy/);
  expect(() => 1).not.toThrow();
  assert.throws(() => expect(() => 1).toThrow(), AssertionError);
});

test("awaiting expect(...) without a matcher rejects, naming the line", async () => {
  await assert.rejects(async () => { await expect(3); },
    /expect\(value\) checks nothing until a matcher is called: expect\(value\)\.toBe\(expected\)\nat .*api-surface\.test\.mjs:\d+:\d+/);
  await assert.rejects(async () => { await expect(3).not; }, /expect\(value\) checks nothing/);
  await assert.rejects(async () => { await expect(() => 1); }, /expect\(fn\) checks nothing until a matcher is called: expect\(fn\)\.toThrow\(\)/);
  // A matcher's own result is unaffected: a once-check returns nothing to await.
  assert.equal(expect(3).toBe(3), undefined);
});

test("expect.poll(fn).toHaveLength retries until the length matches", async () => {
  installFakeHost(createWorld());
  const items = [];
  const timer = setInterval(() => items.push(items.length), 5);

  spec("length", { forensics: false }, async () => {
    await expect.poll(() => [...items], { timeout: 1_000, interval: 5 }).toHaveLength(3);
  });

  const report = await run();
  clearInterval(timer);
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
});

test("t.app.view has locators, not raw element methods; t.app.page binds a contract and there is no t.app.eval", async () => {
  const world = createWorld();
  const save = world.add({ testId: "save" });
  installFakeHost(world);
  const seen = {};

  spec("view", { forensics: false }, async (t) => {
    seen.pageInput = [typeof t.app.view.pointer, typeof t.app.view.key, typeof (await t.app.page()).view.key];
    seen.viewClick = typeof t.app.view.click;
    seen.viewWaitFor = typeof t.app.view.waitFor;
    seen.viewQuery = typeof t.app.view.query;
    seen.page = typeof t.app.page;
    seen.eval = typeof t.app.eval;
    await t.app.view.testId("save").click();
    await t.app.view.css('[data-testid="save"]').click();
    seen.shot = (await t.app.view.screenshot()).format;
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.deepEqual([seen.viewClick, seen.viewWaitFor, seen.viewQuery], ["undefined", "undefined", "undefined"]);
  assert.deepEqual([seen.page, seen.eval], ["function", "undefined"]);
  assert.deepEqual(seen.pageInput, ["undefined", "undefined", "undefined"]);
  assert.equal(save.clicked, 2);
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

  spec.configure({ timeout: 1_234, start: { page: "dashboard" }, tags: ["routed"], covers: ["DEV-1"], forensics: false });
  spec.configure({ tags: ["smoke"] });
  spec("defaults", async () => {});
  spec("overrides", { timeout: 5_000, start: { page: "detail", query: { id: "d1" } }, tags: ["own"], covers: ["DEV-2"] }, async () => {});

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  const [defaults, overrides] = report.cases;
  assert.equal(defaults.timeout_ms, 1_234);
  assert.deepEqual(defaults.tags, ["routed", "smoke"]);
  assert.deepEqual(defaults.covers, ["DEV-1"]);
  assert.equal(overrides.timeout_ms, 5_000);
  assert.deepEqual(overrides.tags, ["routed", "smoke", "own"]);
  assert.deepEqual(overrides.covers, ["DEV-1", "DEV-2"]);
  // `start`: the file's page for the first spec, the spec's own (with its query) for the second.
  assert.deepEqual(world.navCalls.filter(([name]) => name === "relaunch").map(([, options]) => options), [
    { page: "dashboard", waitUntil: "ready" },
    { page: "detail", query: { id: "d1" }, waitUntil: "ready" },
  ]);
});

test("spec.configure rejects an id and malformed options at registration", () => {
  assert.throws(() => spec.configure({ id: "shared" }), /cannot set `id`/);
  assert.throws(() => spec.configure({ timeout: -1 }), /timeout must be a positive finite number/);
  assert.throws(() => spec.configure({ requires: { args: "PASSWORD" } }), /requires\.args must be an array/);
  assert.throws(() => spec("bad", { requires: { openapi: "yes" } }, async () => {}), /requires\.openapi must be a boolean/);
  assert.throws(() => spec("bad start", { start: "home" }, async () => {}), /start must be \{ page, query\? \}/);
  assert.throws(() => spec.configure({ start: { page: "" } }), /start must be \{ page, query\? \}/);
  assert.throws(() => spec("bad query", { start: { page: "home", query: [1] } }, async () => {}), /start\.query must be an object/);
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

test("each entry exports only its own job", async () => {
  const main = await import("@lingxia/test");
  assert.deepEqual(Object.keys(main).sort(),
    ["AssertionError", "TEST_ERROR_CODES", "TimeoutError", "expect", "spec"]);
  const runner = await import("@lingxia/test/runner");
  assert.deepEqual(Object.keys(runner).sort(), [
    "DEFAULT_ACTION_TIMEOUT_MS", "DEFAULT_SPEC_TIMEOUT_MS", "PACKAGE_NAME", "PUBLIC_CAPABILITIES", "VERSION",
    "list", "rawAutomation", "renderJUnit", "reset", "run", "trackPublicSurface",
  ]);
  assert.deepEqual(Object.keys(await import("@lingxia/test/report")), []);
});

test("native input belongs to the app window and keeps its receiver", async () => {
  const world = createWorld();
  const input = [];
  world.app.page.key = { async type(options) { assert.equal(this, world.app.page.key); input.push(options); } };
  installFakeHost(world);
  spec("window input", { forensics: false }, async (t) => {
    const key = t.app.window.key;
    assert.equal(t.app.view.page, undefined);
    await t.app.page();
    await key.type({ text: "hello" });
  });
  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.deepEqual(input, [{ text: "hello" }]);
  assert.ok(report.cases[0].steps.some((step) => step.name === "window.key.type"));
});
