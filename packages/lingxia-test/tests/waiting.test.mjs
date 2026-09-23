import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset, DEFAULT_ACTION_TIMEOUT_MS, DEFAULT_SPEC_TIMEOUT_MS } from "../dist/index.js";

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
      await t.app.page.testId("home-greet").click({ timeout: 80 });
    });
    await globalThis.__LINGXIA_TEST__.run();
    const message = failedMessage(attachments);
    if (kind === "nothing") assert.match(message, /resolved to nothing/);
    if (kind === "hidden") assert.match(message, /resolved to hidden/);
    if (kind === "many") assert.match(message, /resolved to 2 matches/);
  }
});

test("retrying t.expect reports matcher and last actual, not expected true got false", async () => {
  const world = createWorld();
  world.add({ testId: "home-greeting", visible: true, text: "hi" });
  const { attachments } = installFakeHost(world);

  spec("greeting text", async (t) => {
    await t.expect(t.app.page.testId("home-greeting")).toHaveText("hello", { timeout: 80 });
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
    await t.expect(t.app.page.testId("home-greeting")).toBeVisible();
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
        await t.app.eval({ script: "1" });
        ops.push("eval-ok");
      } catch (error) {
        ops.push(error.name);
      }
      try {
        await t.expect.poll(() => 1).toBe(1);
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
    await t.expect(t.app.page.testId("home-greeting")).toHaveText("Hello, Ada!", { timeout: 400 });
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1, decodeAttachment(attachments, "report.json"));
});

test("an eval gets a share of the spec budget unless the caller pins its own", async () => {
  const world = createWorld();
  installFakeHost(world);
  const seen = [];
  const driver = globalThis.lx.automation().lxapp();
  const inner = driver.eval.bind(driver);
  driver.eval = (options) => {
    seen.push(options.timeoutMs);
    return inner(options);
  };

  spec("takes a third of the spec budget", { timeout: 12_000 }, async (t) => {
    await t.app.eval({ script: "1" });
  });
  spec("caps at the eval ceiling", { timeout: 600_000 }, async (t) => {
    await t.app.eval({ script: "1" });
  });
  spec("honours an explicit budget", async (t) => {
    await t.app.eval({ script: "1", timeoutMs: 250 });
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0);
  // Never the whole budget: a call that eats it leaves no room to retry.
  assert.deepEqual(seen, [4_000, 10_000, 250]);
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
    const value = await t.app.page.eval({ script: "1" });
    assert.equal(value, "ok");
    assert.ok(typeof t.app.page.testId("x").click === "function");
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0, JSON.stringify(protocol.cases[0]?.error));
  assert.deepEqual(budgets, [3_000]);
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
    const confirm = t.app.page.testId("sheet-confirm");
    await confirm.waitFor({ state: "attached", timeout: 80 });
    await confirm.waitFor({ state: "visible", timeout: 80 });
    await t.expect(confirm).toBeVisible({ timeout: 80 });
    await t.expect(confirm).not.toBeHidden({ timeout: 80 });
    await t.expect(confirm).not.toBeInViewport({ timeout: 80 });
    await t.expect(t.app.page.testId("collapsed")).toBeHidden({ timeout: 80 });
    await t.expect(t.app.page.testId("missing")).toHaveCount(0);
    await t.app.page.testId("missing").waitFor({ state: "detached", timeout: 80 });
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
    await t.expect(t.app.page.testId("row")).toBeInViewport({ timeout: 1_000 });
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
    await t.reject(() => t.app.page.testId("sheet-confirm").click({ timeout: 120 }), { message: /obscured/ });
    assert.equal(confirm.clicked, undefined);
    await t.app.page.testId("sheet-confirm").click({ force: true, timeout: 500 });
    assert.equal(confirm.clicked, 1);
    assert.equal(confirm.forced, true);
    await t.app.page.testId("sheet-note").fill("hello", { force: true, timeout: 500 });
    assert.equal(field.value, "hello");
    assert.equal(field.forced, true);
    await t.app.page.testId("sheet-locked").click({ force: true, timeout: 120 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.equal(locked.clicked, undefined);
  const message = failedMessage(attachments);
  assert.match(message, /element is disabled/);
});

test("filter, first and last narrow the matches and act on the right DOM node", async () => {
  const world = createWorld();
  const items = ["Apple", "Banana split", "Cherry", "banana bread"].map((text) =>
    world.add({ css: "li", text }));
  const { events } = installFakeHost(world);

  spec("narrowed", async (t) => {
    const rows = t.app.page.css("li");
    await t.expect(rows).toHaveCount(4);
    await t.expect(rows.filter({ hasText: "BANANA" })).toHaveCount(2);
    await t.expect(rows.filter({ hasText: /^Cherry$/ })).toHaveCount(1);
    await rows.filter({ hasText: "banana" }).last().click();
    await rows.first().click();
    await rows.last().click();
    await t.expect(rows.filter({ hasText: "banana" }).first()).toHaveText("Banana split");
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0, JSON.stringify(events.filter((event) => event.type === "case_finished").map((event) => event.error)));
  assert.deepEqual(items.map((item) => item.clicked ?? 0), [1, 0, 0, 2]);
});

test("fixture nav waits for the landed page unless the caller picks waitUntil", async () => {
  const world = createWorld();
  installFakeHost(world);

  spec("navigates", async (t) => {
    await t.app.nav.to({ page: "detail" });
    await t.app.nav.to({ page: "other", waitUntil: "commit" });
    await t.app.nav.to({ page: "slow", timeoutMs: 20_000 });
    await t.app.nav.back();
    const current = await t.app.nav.current();
    assert.equal(current.name, "detail");
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(world.navCalls, [
    ["to", { page: "detail", waitUntil: "ready" }],
    ["to", { page: "other", waitUntil: "commit" }],
    ["to", { page: "slow", timeoutMs: 20_000, waitUntil: "ready" }],
    ["back", { waitUntil: "ready" }],
  ]);
});

test("a fresh spec relaunches home and waits for it to be ready", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  spec("fresh", { fresh: true }, async () => {});

  await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(world.navCalls, [["relaunch", { page: "home", waitUntil: "ready" }]]);
  const report = JSON.parse(decodeAttachment(attachments, "report.json"));
  assert.equal(report.cases[0].status, "passed");
});

test("a fresh relaunch tolerates the home page handing off, not a timeout", async () => {
  for (const [message, expected] of [
    ["page instance 7 (pages/home/index) was disposed before runtime became ready; current page is pages/login/index", "passed"],
    ["timed out after 15000ms waiting for page 7 to become ready", "failed"],
  ]) {
    reset();
    const world = createWorld();
    world.failRelaunch(new Error(message));
    const { attachments } = installFakeHost(world);
    spec("fresh", { fresh: true }, async () => {});
    await globalThis.__LINGXIA_TEST__.run();
    const report = JSON.parse(decodeAttachment(attachments, "report.json"));
    assert.equal(report.cases[0].status, expected, message);
  }
});
