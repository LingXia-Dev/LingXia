import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect } from "../dist/index.js";
import { reset, run, DEFAULT_ACTION_TIMEOUT_MS, DEFAULT_SPEC_TIMEOUT_MS } from "../dist/runner.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

function decodeAttachment(attachments, name) {
  const artifact = attachments.get(name);
  assert.ok(artifact, `missing attachment ${name}`);
  return Buffer.from(artifact.base64, "base64").toString("utf8");
}

function failedMessage(attachments) {
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.equal(report.cases[0].status, "failed");
  return report.cases[0].error.message;
}

test("locator click miss names nothing, hidden, and N matches", async () => {
  for (const [kind, setup] of [
    ["nothing", (world) => world],
    ["hidden", (world) => {
      world.add({ testId: "home-greet", visible: false, text: "Say Hello" });
      return world;
    }],
    ["many", (world) => {
      world.add({ testId: "home-greet", visible: true, text: "A" });
      world.add({ testId: "home-greet", visible: true, text: "B" });
      return world;
    }],
  ]) {
    reset();
    const world = createWorld();
    setup(world);
    const { attachments } = installFakeHost(world);
    spec(`miss ${kind}`, async (t) => {
      await t.app.view.testId("home-greet").click({ timeout: 80 });
    });
    await globalThis.__LINGXIA_TEST__.run();
    const message = failedMessage(attachments);
    if (kind === "nothing") assert.match(message, /resolved to nothing/);
    if (kind === "hidden") assert.match(message, /resolved to hidden/);
    if (kind === "many") assert.match(message, /resolved to 2 matches/);
  }
});

test("a retrying expect reports matcher and last actual, not expected true got false", async () => {
  const world = createWorld();
  world.add({ testId: "home-greeting", visible: true, text: "hi" });
  const { attachments } = installFakeHost(world);

  spec("greeting text", async (t) => {
    await expect(t.app.view.testId("home-greeting")).toHaveText("hello", { timeout: 80 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const message = failedMessage(attachments);
  assert.match(message, /toHaveText/);
  assert.match(message, /hello/);
  assert.match(message, /hi/);
  assert.doesNotMatch(message, /expected true, got false/i);
  assert.doesNotMatch(message, /Expected: true\nReceived: false/);
});

test("default 5s assertion budget fails faster than the 30s spec budget", async () => {
  assert.equal(DEFAULT_ACTION_TIMEOUT_MS, 5_000);
  assert.equal(DEFAULT_SPEC_TIMEOUT_MS, 30_000);
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  spec("missing greeting", { timeout: DEFAULT_SPEC_TIMEOUT_MS }, async (t) => {
    await expect(t.app.view.testId("home-greeting")).toBeVisible();
  });

  const started = Date.now();
  await globalThis.__LINGXIA_TEST__.run();
  const elapsed = Date.now() - started;
  const message = failedMessage(attachments);
  assert.match(message, /toBeVisible/);
  assert.match(message, /resolved to nothing/);
  assert.ok(elapsed < 12_000, `assertion burned ${elapsed}ms, should be ~5s not 30s`);
  assert.ok(elapsed >= 4_500, `assertion finished too fast (${elapsed}ms)`);
});

test("timeout aborts later fixture operations", async () => {
  const world = createWorld();
  installFakeHost(world);
  const ops = [];
  let zombie;

  spec("hangs then tries more work", { timeout: 60, forensics: false }, async (t) => {
    zombie = (async () => {
      await new Promise((resolve) => setTimeout(resolve, 120));
      ops.push("after-sleep");
      try {
        await t.app.logic.eval(() => 1);
        ops.push("eval-ok");
      } catch (error) {
        ops.push(error.name);
      }
      try {
        await expect.poll(() => 1).toBe(1);
        ops.push("expected");
      } catch (error) {
        ops.push(error.name);
      }
    })();
    await new Promise((resolve) => setTimeout(resolve, 250));
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  await zombie;
  assert.equal(protocol.timeout, 1);
  assert.ok(ops.includes("after-sleep"));
  assert.deepEqual(ops.filter((item) => item !== "after-sleep"), ["TimeoutError", "TimeoutError"]);
  assert.ok(!ops.includes("eval-ok"));
  assert.ok(!ops.includes("expected"));
});

test("locator re-resolves across mutations", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  const node = world.add({ testId: "home-greeting", visible: false, text: "" });

  spec("appears later", async (t) => {
    setTimeout(() => {
      node.visible = true;
      node.text = "Hello, Ada!";
    }, 40);
    await expect(t.app.view.testId("home-greeting")).toHaveText("Hello, Ada!", { timeout: 400 });
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1, decodeAttachment(attachments, "report.json"));
});

test("an eval without its own timeout gets 10 s, clamped to the spec budget", async () => {
  const world = createWorld();
  installFakeHost(world);
  const seen = [];
  const driver = globalThis.lx.automation().lxapp();
  const inner = driver.eval.bind(driver);
  driver.eval = (options) => {
    seen.push(options.timeoutMs);
    return inner(options);
  };

  spec("a roomy budget", { timeout: 12_000 }, async (t) => {
    await t.app.logic.eval(() => 1);
  });
  spec("a long budget", { timeout: 600_000 }, async (t) => {
    await t.app.logic.eval(() => 1);
  });
  spec("a short budget", { timeout: 4_000 }, async (t) => {
    await t.app.logic.eval(() => 1);
  });
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0);
  // A fixed default, not a share of the budget; never more than the spec has left.
  assert.deepEqual(seen.slice(0, 2), [10_000, 10_000]);
  assert.ok(seen[2] > 3_000 && seen[2] < 4_000, String(seen[2]));
});

test("wrapping the page driver never writes back onto the driver", async () => {
  const world = createWorld();
  installFakeHost(world);
  const driver = globalThis.lx.automation().lxapp();
  const original = { eval: driver.page.eval, testId: driver.page.testId, css: driver.page.css };
  const budgets = [];
  driver.page.eval = (options) => {
    budgets.push(options.timeoutMs);
    return Promise.resolve("ok");
  };
  const wrappedOriginal = driver.page.eval;

  spec("uses the page driver", { timeout: 9_000 }, async (t) => {
    // Recursion here would blow the stack instead of failing an assertion.
    const value = await t.app.view.eval(() => 1);
    assert.equal(value, "ok");
    assert.ok(typeof t.app.view.testId("x").click === "function");
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0, JSON.stringify(protocol.cases[0]?.error));
  assert.equal(budgets.length, 1);
  assert.ok(budgets[0] > 8_000 && budgets[0] < 9_000, String(budgets[0]));
  assert.equal(driver.page.eval, wrappedOriginal, "the driver's own eval was replaced");
  assert.equal(driver.page.testId, original.testId);
  assert.equal(driver.page.css, original.css);
});

test("visible means rendered: an out-of-viewport match is visible, not in the viewport, not hidden", async () => {
  const world = createWorld();
  world.add({ testId: "sheet-confirm", inViewport: false, text: "Confirm" });
  world.add({ testId: "collapsed", visible: false, text: "" });
  const { attachments } = installFakeHost(world);

  spec("below the fold", async (t) => {
    const confirm = t.app.view.testId("sheet-confirm");
    await confirm.waitFor({ state: "attached", timeout: 80 });
    await confirm.waitFor({ state: "visible", timeout: 80 });
    await expect(confirm).toBeVisible({ timeout: 80 });
    await expect(confirm).not.toBeHidden({ timeout: 80 });
    await expect(confirm).not.toBeInViewport({ timeout: 80 });
    await expect(t.app.view.testId("collapsed")).toBeHidden({ timeout: 80 });
    await expect(t.app.view.testId("missing")).toHaveCount(0);
    await t.app.view.testId("missing").waitFor({ state: "detached", timeout: 80 });
    await confirm.waitFor({ state: "inViewport", timeout: 80 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const message = failedMessage(attachments);
  assert.match(message, /to be inViewport/);
  assert.match(message, /visible element outside the viewport/);
});

test("toBeInViewport passes once the match scrolls into the viewport", async () => {
  const world = createWorld();
  const row = world.add({ testId: "row", inViewport: false, text: "Row" });
  const { events } = installFakeHost(world);

  spec("scrolls in", async (t) => {
    setTimeout(() => { row.inViewport = true; }, 40);
    await expect(t.app.view.testId("row")).toBeInViewport({ timeout: 1_000 });
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0, JSON.stringify(events.filter((event) => event.type === "case_finished")));
});

test("click({ force }) skips the viewport and hit-test waits but not enabled", async () => {
  const world = createWorld();
  const confirm = world.add({ testId: "sheet-confirm", inViewport: false, text: "Confirm" });
  const locked = world.add({ testId: "sheet-locked", inViewport: false, enabled: false, text: "Locked" });
  const field = world.add({ testId: "sheet-note", inViewport: false, value: "" });
  const { attachments } = installFakeHost(world);

  spec("forced", async (t) => {
    await t.reject(() => t.app.view.testId("sheet-confirm").click({ timeout: 120 }), { message: /obscured/ });
    assert.equal(confirm.clicked, undefined);
    await t.app.view.testId("sheet-confirm").click({ force: true, timeout: 500 });
    assert.equal(confirm.clicked, 1);
    assert.equal(confirm.forced, true);
    await t.app.view.testId("sheet-note").fill("hello", { force: true, timeout: 500 });
    assert.equal(field.value, "hello");
    assert.equal(field.forced, true);
    await t.app.view.testId("sheet-locked").click({ force: true, timeout: 120 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.equal(locked.clicked, undefined);
  const message = failedMessage(attachments);
  assert.match(message, /element is disabled/);
});

test("filter, first and last narrow the matches and act on the right DOM node", async () => {
  const world = createWorld();
  const items = ["Apple", "Banana split", "Cherry", "banana bread"].map((text) =>
    world.add({ css: "li", text, attributes: { "data-kind": text.split(" ")[0].toLowerCase() } }));
  const { events } = installFakeHost(world);

  spec("narrowed", async (t) => {
    const rows = t.app.view.css("li");
    await expect(rows).toHaveCount(4);
    await expect(rows.filter({ hasText: "BANANA" })).toHaveCount(2);
    await expect(rows.filter({ hasText: /^Cherry$/ })).toHaveCount(1);
    await rows.filter({ hasText: "banana" }).last().click();
    await rows.first().click();
    await rows.last().click();
    await expect(rows.filter({ hasText: "banana" }).first()).toHaveText("Banana split");
    await expect(rows.filter({ hasText: "banana" }).nth(1)).toContainText("bread");
    await expect(rows.filter({ hasText: "Cherry" })).toHaveAttribute("data-kind", "cherry");
    await expect(rows.filter({ hasText: "Cherry" })).toHaveAttribute("data-kind", /^ch/);
    await expect(rows.filter({ hasText: "Cherry" })).toHaveAttribute("data-kind");
    await expect(rows.filter({ hasText: "Cherry" })).not.toHaveAttribute("aria-busy");
    await expect(rows.filter({ hasText: "Cherry" })).not.toHaveAttribute("data-kind", "apple");
    await expect(rows.first()).not.toContainText("Banana");
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0, JSON.stringify(events.filter((event) => event.type === "case_finished").map((event) => event.error)));
  assert.deepEqual(items.map((item) => item.clicked ?? 0), [1, 0, 0, 2]);
});

test("toHaveAttribute and toContainText failures name what was found", async () => {
  const world = createWorld();
  world.add({ testId: "badge", text: "3 unread", attributes: { "aria-label": "Inbox" } });
  const { attachments } = installFakeHost(world);

  spec("attribute miss", async (t) => {
    await expect(t.app.view.testId("badge")).toContainText("unread", { timeout: 80 });
    await expect(t.app.view.testId("badge")).toHaveAttribute("aria-label", "Outbox", { timeout: 80 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const message = failedMessage(attachments);
  assert.match(message, /aria-label="Outbox"/);
  assert.match(message, /aria-label="Inbox"/);
});

test("fixture nav waits for the landed page unless the caller picks waitUntil; it takes { timeout }", async () => {
  const world = createWorld();
  installFakeHost(world);
  let refused;

  spec("navigates", { timeout: 10_000 }, async (t) => {
    await t.app.nav.to({ page: "detail" });
    await t.app.nav.to({ page: "other", waitUntil: "commit" });
    await t.app.nav.to({ page: "slow", timeout: 5_000 });
    await t.app.nav.to({ page: "slower", timeout: 60_000 });
    await t.app.nav.back();
    const current = await t.app.nav.current();
    assert.equal(current.name, "slow");
    refused = await t.reject(() => t.app.nav.to({ page: "x", timeoutMs: 1 }));
  });

  const report = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  const calls = world.navCalls.map(([verb, options]) => [verb, { ...options, ...(options.timeoutMs ? { timeoutMs: options.timeoutMs <= 10_000 && options.timeoutMs > 9_000 ? "room" : options.timeoutMs } : {}) }]);
  assert.deepEqual(calls, [
    ["to", { page: "detail", waitUntil: "ready", timeoutMs: "room" }],
    ["to", { page: "other", waitUntil: "commit" }],
    ["to", { page: "slow", waitUntil: "ready", timeoutMs: 5_000 }],
    ["to", { page: "slower", waitUntil: "ready", timeoutMs: "room" }],
    ["back", { waitUntil: "ready", timeoutMs: "room" }],
  ]);
  assert.match(refused.message, /takes \{ timeout \} in ms/);
});

test("a driver timeout reaches the spec as E_TIMEOUT, the driver's code as its cause", async () => {
  const world = createWorld();
  installFakeHost(world);
  world.app.nav.to = async () => {
    throw Object.assign(new Error("page did not become ready within 50ms"), { code: "E_AUTOMATION_TIMEOUT", data: { page: "slow" } });
  };
  world.app.eval = async () => {
    throw Object.assign(new Error("eval did not settle"), { code: "E_EVAL_TIMEOUT" });
  };
  const seen = {};

  spec("times out", async (t) => {
    seen.nav = await t.reject(() => t.app.nav.to({ page: "slow", timeout: 50 }), { code: "E_TIMEOUT" });
    seen.eval = await t.reject(() => t.app.logic.eval({ timeout: 100 }, () => 1), { code: "E_TIMEOUT" });
  });

  const report = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(seen.nav.name, "TimeoutError");
  assert.equal(seen.nav.cause.code, "E_AUTOMATION_TIMEOUT");
  assert.deepEqual(seen.nav.data, { page: "slow", driverCode: "E_AUTOMATION_TIMEOUT" });
  assert.match(seen.nav.message, /did not become ready/);
  assert.equal(seen.eval.cause.code, "E_EVAL_TIMEOUT");
  assert.deepEqual(seen.eval.data, { driverCode: "E_EVAL_TIMEOUT" });
});

test("a `start` spec relaunches on that page, with its query, before beforeEach, and waits for it to be ready", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);
  const order = [];

  spec.beforeEach(async (t) => { order.push(`beforeEach on ${(await t.app.nav.current()).name}`); });
  spec("home", { start: { page: "home" } }, async () => {});
  spec("detail", { start: { page: "detail", query: { id: "d1" } } }, async (t) => {
    order.push(`body on ${(await t.app.nav.current()).name}`);
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(world.navCalls, [
    ["relaunch", { page: "home", waitUntil: "ready" }],
    ["relaunch", { page: "detail", query: { id: "d1" }, waitUntil: "ready" }],
  ]);
  assert.deepEqual(order, ["beforeEach on home", "beforeEach on detail", "body on detail"]);
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.deepEqual(report.cases.map((item) => item.status), ["passed", "passed"]);
});

test("a start relaunch tolerates the home page handing off, not a timeout", async () => {
  for (const [message, expected] of [
    ["page instance 7 (pages/home/index) was disposed before runtime became ready; current page is pages/login/index", "passed"],
    ["timed out after 15000ms waiting for page 7 to become ready", "failed"],
  ]) {
    reset();
    const world = createWorld();
    world.failRelaunch(new Error(message));
    const { attachments } = installFakeHost(world);
    spec("start", { start: { page: "home" } }, async () => {});
    await globalThis.__LINGXIA_TEST__.run();
    const report = JSON.parse(decodeAttachment(attachments, "report.json"));
    assert.equal(report.cases[0].status, expected, message);
  }
});

test("text matchers see whitespace-normalised text, as the user reads it", async () => {
  const world = createWorld();
  // WebKit's innerText of a flex row ends in a line break.
  world.add({ testId: "band", text: "Good\n" });
  world.add({ testId: "status", text: "  Following\n\n   up\t" });
  world.add({ testId: "miss", text: "Good\nnight" });
  const { attachments } = installFakeHost(world);
  const seen = [];

  spec("normalised", async (t) => {
    const view = t.app.view;
    await expect(view.testId("band")).toHaveText("Good");
    await expect(view.testId("band")).toHaveText(/^Good$/);
    await expect(view.testId("status")).toHaveText("Following up");
    await expect(view.testId("status")).toHaveText(" Following   up ");
    await expect(view.testId("status")).toContainText("Following up");
    await expect(view.testId("status")).toHaveText(/^Following up$/);
    seen.push("passed");
    await expect(view.testId("miss")).toHaveText("Good", { timeout: 80 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(seen, ["passed"]);
  const message = failedMessage(attachments);
  assert.match(message, /Expected: "Good"\nReceived: "Good night"/);
});

/** Page eval that answers the visibility probe as a hidden page does. */
function hidePage(world, answer = { state: "hidden", animationFrames: false }) {
  const original = world.app.page.eval;
  const probes = { count: 0 };
  world.app.page.eval = async (options) => {
    if (/visibilityState/.test(options.script) && /requestAnimationFrame/.test(options.script)) {
      probes.count += 1;
      return answer;
    }
    return original(options);
  };
  return probes;
}

test("hidden-page actions exhaust their original budget without raising a window or probing afterward", async () => {
  const world = createWorld();
  world.add({ testId: "sheet", visible: false });
  const probes = hidePage(world);
  installFakeHost(world);
  const errors = [];
  spec("bounded hidden page", { forensics: false }, async t => {
    for (const action of [
      () => t.app.view.testId("sheet").waitFor({ timeout: 80 }),
      () => t.app.view.testId("sheet").click({ timeout: 80 }),
    ]) {
      const start = Date.now();
      try { await action(); } catch (error) { errors.push(error); }
      assert.ok(Date.now() - start < 500, "no extra raise/retry budget");
    }
  });
  const report = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(report.passed, 1);
  assert.deepEqual(errors.map(error => error.code), ["E_TIMEOUT", "E_TIMEOUT"]);
  assert.equal(probes.count, 0);
});

test("a locked screen stops the run once, before the next spec, instead of failing each", async () => {
  const world = createWorld();
  world.add({ testId: "ready", text: "Ready" });
  const { events, attachments } = installFakeHost(world);
  let locked = false;
  globalThis.__LINGXIA_AUTOMATION_HOST__.screenLocked = () => locked;
  const ran = [];

  spec("first", async (t) => {
    ran.push("first");
    await expect(t.app.view.testId("ready")).toBeVisible();
    locked = true;
  });
  spec("second", async () => { ran.push("second"); });
  spec("third", async () => { ran.push("third"); });

  const report = await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(ran, ["first"]);
  const locks = events.filter((event) => event.type === "diagnostic" && event.phase === "screen_locked");
  assert.equal(locks.length, 1);
  assert.match(locks[0].message, /^The screen is locked; unlock it — animations and sheets are paused\. Stopped after 1\/3 specs/);
  assert.equal(report.partial, true);
  const saved = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.deepEqual(saved.cases.map((item) => item.status), ["passed", "skipped", "skipped"]);
  assert.match(saved.cases[1].reason, /^Not run: the screen is locked/);
});

test("a run that starts on a locked screen runs nothing", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  globalThis.__LINGXIA_AUTOMATION_HOST__.screenLocked = () => true;
  let ran = false;

  spec("only", async () => { ran = true; });

  const report = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(ran, false);
  assert.equal(report.partial, true);
  assert.equal(events.filter((event) => event.phase === "screen_locked").length, 1);
});

test("a host that cannot tell, or throws, is never read as locked", async () => {
  for (const screenLocked of [undefined, () => undefined, () => { throw new Error("no session"); }]) {
    const world = createWorld();
    installFakeHost(world);
    if (screenLocked) globalThis.__LINGXIA_AUTOMATION_HOST__.screenLocked = screenLocked;
    let ran = false;
    spec("runs", async () => { ran = true; });
    const report = await globalThis.__LINGXIA_TEST__.run();
    assert.equal(ran, true);
    assert.equal(report.partial, false);
    reset();
  }
});

test("waits that pass never probe the page's visibility", async () => {
  const world = createWorld();
  world.add({ testId: "ready", text: "Ready" });
  const probes = hidePage(world, { state: "visible", animationFrames: true });
  installFakeHost(world);

  spec("happy path", { forensics: false }, async (t) => {
    await t.app.view.testId("ready").click();
    await t.app.view.testId("ready").waitFor();
    await expect(t.app.view.testId("ready")).toHaveText("Ready");
  });

  const report = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(report.passed, 1);
  assert.equal(probes.count, 0);
});

test("a visible page with running frames adds no hidden-page note", async () => {
  const world = createWorld();
  world.add({ testId: "sheet", visible: false, text: "" });
  hidePage(world, { state: "visible", animationFrames: true });
  const { attachments } = installFakeHost(world);

  spec("visible page", { forensics: false }, async (t) => {
    await expect(t.app.view.testId("sheet")).toBeVisible({ timeout: 80 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.doesNotMatch(failedMessage(attachments), /page looked hidden/);
});

test("failure forensics record the page's visibility", async () => {
  const world = createWorld();
  world.add({ testId: "sheet", visible: false, text: "" });
  hidePage(world, { state: "visible", animationFrames: false });
  const { attachments } = installFakeHost(world);

  spec("frames paused", async (t) => {
    await expect(t.app.view.testId("sheet")).toBeVisible({ timeout: 80 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const forensics = JSON.parse(decodeAttachment(attachments, "attachments/frames-paused/attempt-0/forensics.json"));
  assert.deepEqual(
    { state: forensics.visibility.state, animationFrames: forensics.visibility.animationFrames },
    { state: "visible", animationFrames: false });
  assert.match(forensics.visibility.note, /no animation frame observed within 300ms \(a covered window or a sleeping display pauses a page's animations\)/);
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.match(report.cases[0].error.page.hidden, /page looked hidden or paused/);
  assert.match(report.failures[0].page.hidden, /no animation frame/);
});

test("locator reads are one-shot and traced: count, isVisible, textContent, inputValue, getAttribute", async () => {
  const world = createWorld();
  world.add({ testId: "title", text: "  Hello\n  world ", attributes: { "aria-level": "1" } });
  world.add({ testId: "name", text: "", value: "Ada" });
  world.add({ testId: "row", text: "a" });
  world.add({ testId: "row", text: "b" });
  world.add({ testId: "folded", visible: false, text: "x" });
  installFakeHost(world);
  const seen = {};
  const errors = {};
  const failure = async (name, read) => {
    try { await read(); errors[name] = "resolved"; } catch (error) { errors[name] = error.message; }
  };
  const result = await (async () => {
    spec("reads", { forensics: false }, async (t) => {
      const view = t.app.view;
      seen.counts = [await view.testId("row").count(), await view.testId("missing").count(), await view.testId("title").count()];
      seen.visible = [await view.testId("title").isVisible(), await view.testId("folded").isVisible(), await view.testId("missing").isVisible()];
      seen.text = await view.testId("title").textContent();
      seen.value = await view.testId("name").inputValue();
      seen.attributes = [await view.testId("title").getAttribute("aria-level"), await view.testId("title").getAttribute("hidden")];
      seen.nth = await view.testId("row").nth(1).textContent();
      await failure("isVisibleMany", () => view.testId("row").isVisible());
      await failure("textMany", () => view.testId("row").textContent());
      await failure("textNone", () => view.testId("missing").textContent());
      await failure("valueNone", () => view.testId("title").inputValue());
      await failure("attributeMany", () => view.testId("row").getAttribute("id"));
      await failure("attributeName", () => view.testId("title").getAttribute(""));
      // No wait: an element that appears later is not seen by a read.
      setTimeout(() => world.add({ testId: "late", text: "late" }), 30);
      seen.late = await view.testId("late").count();
      seen.query = typeof view.testId("title").query;
    });
    return (await run()).cases[0];
  })();
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.deepEqual(seen.counts, [2, 0, 1]);
  assert.deepEqual(seen.visible, [true, false, false]);
  assert.equal(seen.text, "Hello world");
  assert.equal(seen.value, "Ada");
  assert.deepEqual(seen.attributes, ["1", null]);
  assert.equal(seen.nth, "b");
  assert.equal(seen.late, 0);
  assert.equal(seen.query, "undefined");
  assert.match(errors.isVisibleMany, /^isVisible\(\): locator .* resolved to 2 matches; narrow it with nth\(\)/);
  assert.match(errors.textMany, /^textContent\(\): locator .* resolved to 2 matches; narrow it/);
  assert.match(errors.textNone, /^textContent\(\): locator .* resolved to nothing$/);
  assert.match(errors.valueNone, /^inputValue\(\): \[data-testid="title"\] has no value/);
  assert.match(errors.attributeMany, /^getAttribute\(\): locator .* resolved to 2 matches/);
  assert.match(errors.attributeName, /getAttribute\(name\) takes an attribute name/);
  const traced = new Set(result.steps.map((step) => step.name));
  for (const verb of ["count", "isVisible", "textContent", "inputValue", "getAttribute"]) assert.ok(traced.has(`page.${verb}`), verb);
});

test("t.app.view takes no options on testId, css or screenshot, and no page on eval or scroll", async () => {
  installFakeHost(createWorld());
  const messages = [];
  spec("options", { forensics: false }, async (t) => {
    for (const call of [
      () => t.app.view.testId("x", { page: "other" }),
      () => t.app.view.css("#x", { index: 1 }),
      () => t.app.view.screenshot({ page: "other" }),
      () => t.app.view.eval({ page: "other" }, () => 1),
      () => t.app.view.scroll({ page: "other", dy: 1 }),
    ]) {
      try { await call(); messages.push("accepted"); } catch (error) { messages.push(`${error.name}: ${error.message}`); }
    }
  });
  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases[0]?.error));
  assert.deepEqual(messages.map((message) => message.split(":")[0]), ["TypeError", "TypeError", "TypeError", "TypeError", "TypeError"]);
  assert.match(messages[0], /view\.testId\(\) takes no options: bind another page with t\.app\.page\(\{ name \}\) and use its view/);
  assert.match(messages[1], /view\.css\(\) takes no options/);
  assert.match(messages[2], /view\.screenshot\(\) takes no options/);
  assert.match(messages[3], /page is not an eval option/);
  assert.match(messages[4], /view\.scroll\(\) takes no page option: bind another page/);
});

test("a retrying locator assertion that times out is an AssertionError with code E_TIMEOUT", async () => {
  const world = createWorld();
  world.add({ testId: "status", text: "loading" });
  installFakeHost(world);
  let caught;
  spec("times out", { forensics: false }, async (t) => {
    caught = await t.reject(() => expect(t.app.view.testId("status")).toHaveText("done", { timeout: 60 }), { code: "E_TIMEOUT" });
    await expect(t.app.view.testId("status")).toHaveText("done", { timeout: 60 });
  });
  const report = await run();
  assert.equal(caught.name, "AssertionError");
  assert.equal(caught.code, "E_TIMEOUT");
  assert.match(caught.message, /^Timed out after \d+ms retrying toHaveText\./);
  assert.equal(report.cases[0].error.code, "E_TIMEOUT");
  assert.equal(report.cases[0].error.matcher, "toHaveText");
  // A once-check is not a timeout.
  try { expect(1).toBe(2); } catch (error) { assert.equal(error.code, undefined); }
});

test("a remote TypeError from Logic fails a retry at once; one from the page retries", async () => {
  const world = createWorld();
  let pageReads = 0;
  world.usePage({ window: {}, document: { querySelector: () => (++pageReads < 3 ? null : { textContent: "ready" }) } });
  world.useLogic({ lx: {}, getCurrentPages: () => [] });
  installFakeHost(world);
  const seen = {};
  spec("type errors", { forensics: false }, async (t) => {
    // In Logic a TypeError is a programming mistake: no retry.
    let logicReads = 0;
    seen.waitFor = await t.reject(() => t.waitFor(() => { logicReads += 1; return t.app.logic.eval(({ getCurrentPages }) => getCurrentPages()[0].data); },
      { timeout: 500, interval: 5 }));
    seen.waitForReads = logicReads;
    logicReads = 0;
    seen.poll = await t.reject(() => expect.poll(() => { logicReads += 1; return t.app.logic.eval(({ getCurrentPages }) => getCurrentPages()[0].data); },
      { timeout: 500, interval: 5 }).toBe(1));
    seen.pollReads = logicReads;
    // In the page a null read can throw until rendering catches up.
    seen.page = await t.waitFor(() => t.app.view.eval(({ document }) => document.querySelector("#total").textContent),
      { timeout: 500, interval: 5 });
  });
  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases[0]?.error));
  assert.equal(seen.waitFor.name, "TypeError");
  assert.equal(seen.waitForReads, 1);
  assert.equal(seen.poll.name, "TypeError");
  assert.equal(seen.pollReads, 1);
  assert.equal(seen.page, "ready");
  assert.equal(pageReads, 3);
});

test("a hidden-page note explains only a locator failure; a bound locator off the current page says so instead", async () => {
  const cases = [
    ["action", async (t) => { await t.app.view.testId("sheet").click({ timeout: 80 }); }, "hidden"],
    // `#sheet` is a CSS id on the current page, not a bound instance.
    ["css id action", async (t) => { await t.app.view.css("#sheet").click({ timeout: 80 }); }, "hidden"],
    ["read", async (t) => { await t.app.view.testId("missing").textContent(); }, "hidden"],
    ["assertion", async (t) => { await expect(t.app.view.testId("sheet")).toBeVisible({ timeout: 80 }); }, "hidden"],
    ["logic eval", async (t) => {
      await t.app.view.testId("sheet").count();
      await t.app.logic.eval(() => { throw new Error("backend said no"); });
    }, "none"],
    ["plain throw", async (t) => {
      await t.app.view.testId("sheet").count();
      throw new Error("the spec's own check");
    }, "none"],
    ["caught locator failure", async (t) => {
      await t.reject(() => t.app.view.testId("missing").textContent());
      throw new Error("a later failure");
    }, "none"],
    ["bound current page", async (t) => {
      const page = await t.app.page();
      await page.view.testId("sheet").click({ timeout: 80 });
    }, "hidden"],
    ["bound action off the current page", async (t) => {
      const home = await t.app.page();
      await t.app.nav.to({ page: "other" });
      await home.view.testId("sheet").click({ timeout: 80 });
    }, "elsewhere"],
    ["bound assertion off the current page", async (t) => {
      const home = await t.app.page();
      await t.app.nav.to({ page: "other" });
      await expect(home.view.testId("sheet")).toBeVisible({ timeout: 80 });
    }, "elsewhere"],
  ];
  for (const [name, body, expected] of cases) {
    reset();
    const world = createWorld();
    world.add({ testId: "sheet", id: "sheet", visible: false, text: "" });
    hidePage(world);
    installFakeHost(world);
    spec(name, body);
    const report = await run();
    const failed = report.cases[0];
    assert.equal(failed.status, "failed", name);
    const page = failed.error.page;
    assert.ok(page, `${name}: the failure names the current page`);
    if (expected === "hidden") assert.match(page.hidden ?? "", /page looked hidden or paused/, name);
    if (expected === "none") assert.equal(page.hidden, undefined, name);
    if (expected === "elsewhere") {
      const bound = world.stack[0].instanceId;
      assert.notEqual(page.instanceId, bound, name);
      assert.equal(page.hidden, `the locator's page #${bound} was not the current page`, name);
    }
  }
});

test("app.page() binds locators, eval, screenshot and scroll to an instance; calls cannot change the target", async () => {
  const world = createWorld();
  world.add({ testId: "todo-input" });
  installFakeHost(world);
  const queried = [];
  const query = world.app.page.query;
  world.app.page.query = (options) => { queried.push(options.page); return query(options); };
  const shots = [];
  const screenshot = world.app.page.screenshot;
  world.app.page.screenshot = (options) => { shots.push(options?.page); return screenshot(options); };

  let instanceId;
  spec("bound", { forensics: false }, async (t) => {
    await t.app.nav.to({ page: "todo" });
    const page = await t.app.page({ name: "todo" });
    instanceId = page.instanceId;
    const todo = page.view;
    await t.app.nav.to({ page: "other" });
    await todo.testId("todo-input").fill("milk");
    await expect(todo.testId("todo-input")).toHaveValue("milk");
    assert.throws(() => todo.testId("todo-input", { page: "other" }), /takes no options/);
    await todo.eval(({ document }) => document.title);
    assert.throws(() => todo.eval({ page: "cart" }, ({ document }) => document.title), /page is not an eval option/);
    await todo.screenshot();
    await t.app.view.screenshot();
    await todo.scroll({ dy: 10 });
    await t.app.view.scroll({ dy: 10 });
    assert.throws(() => todo.scroll({ page: "cart" }), /takes no page option/);
    await t.reject(() => t.app.page({ name: "" }), { message: /non-empty name or instanceId/ });
    await t.reject(() => t.app.page({ name: "todo", instanceId: instanceId }), { message: /exactly one/ });
    await t.reject(() => t.app.page({ name: "todo" }, { timeout: 0 }), { message: /positive number of ms/ });
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.ok(queried.includes(instanceId) && !queried.includes("other"), JSON.stringify(queried));
  assert.ok(!queried.includes(undefined), JSON.stringify(queried));
  assert.ok(world.evaluatedPages.includes(instanceId));
  assert.ok(!world.evaluatedPages.includes("cart"));
  assert.deepEqual(shots, [instanceId, undefined]);
  assert.deepEqual(world.pageTargets.filter(([method]) => method === "scroll"), [["scroll", instanceId], ["scroll", undefined]]);
});

test("app.page() waits for a page to open, times out naming it, refuses an ambiguous name, and rejects once its instance is gone", async () => {
  const world = createWorld();
  world.add({ testId: "title", page: "home", text: "Home" });
  world.add({ testId: "title", page: "detail", text: "Detail" });
  installFakeHost(world);
  const seen = {};
  spec("binding", { forensics: false }, async (t) => {
    // Opens later: the bind waits for it.
    setTimeout(() => { void world.app.nav.to({ page: "detail" }); }, 60);
    const started = Date.now();
    const detail = await t.app.page({ name: "detail" }, { timeout: 2_000 });
    seen.waited = Date.now() - started;
    seen.detailName = detail.name;
    const home = await t.app.page({ name: "home" });
    // Each view reads its own instance; t.app.view reads whichever is current.
    seen.titles = [await home.view.testId("title").textContent(), await detail.view.testId("title").textContent(),
      await t.app.view.testId("title").textContent()];
    // Never opens: the bind times out naming it.
    seen.timeout = await t.reject(() => t.app.page({ name: "settings" }, { timeout: 120 }), { code: "E_TIMEOUT" });
    // A second live instance of the name is ambiguous; its instance id is not.
    await t.app.nav.to({ page: "detail" });
    const second = await t.app.page();
    seen.ambiguous = await t.reject(() => t.app.page({ name: "detail" }));
    seen.byId = (await t.app.page({ instanceId: second.instanceId })).instanceId === second.instanceId;
    // The handle never follows navigation: once its instance is gone, calls reject.
    await t.app.nav.back();
    seen.current = (await t.app.nav.current()).instanceId;
    seen.gone = await t.reject(() => second.view.testId("title").textContent(), { code: "E_PAGE_NOT_ACTIVE" });
    seen.goneData = await t.reject(() => second.data(), { message: `page instance #${second.instanceId} is gone` });
    seen.goneAction = await t.reject(() => second.actions.save(), { code: "E_PAGE_NOT_ACTIVE" });
    seen.goneBind = await t.reject(() => t.app.page({ instanceId: second.instanceId }, { timeout: 60 }), { code: "E_TIMEOUT" });
    seen.detailStill = await detail.view.testId("title").textContent();
    seen.ids = [detail.instanceId, second.instanceId];
  });
  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases[0]?.error));
  assert.ok(seen.waited >= 40, String(seen.waited));
  assert.equal(seen.detailName, "detail");
  assert.deepEqual(seen.titles, ["Home", "Detail", "Detail"]);
  assert.match(seen.timeout.message, /^Timed out after \d+ms waiting for page "settings" to open/);
  assert.equal(seen.timeout.name, "TimeoutError");
  assert.match(seen.ambiguous.message, new RegExp(`page "detail" has 2 live instances \\(#${seen.ids[0]}, #${seen.ids[1]}\\); select one by instanceId`));
  assert.equal(seen.byId, true);
  assert.equal(seen.current, seen.ids[0]);
  assert.match(seen.goneBind.message, new RegExp(`waiting for page instance #${seen.ids[1]} to open`));
  assert.equal(seen.detailStill, "Detail");
  const bind = report.cases[0].steps.filter((step) => step.name === "page.bind").map((step) => step.detail);
  assert.ok(bind.includes("detail"), JSON.stringify(bind));
});
