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

test("an eval gets a share of the spec budget", async () => {
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
    await t.app.logic.eval(() => 1);
  });
  spec("caps at the eval ceiling", { timeout: 600_000 }, async (t) => {
    await t.app.logic.eval(() => 1);
  });
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0);
  // Never the whole budget: a call that eats it leaves no room to retry.
  assert.deepEqual(seen, [4_000, 10_000]);
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
  let raised = 0;
  globalThis.__LINGXIA_AUTOMATION_HOST__.raiseWindow = async () => { raised++; return true; };
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
  assert.equal(raised, 0);
  assert.equal(probes.count, 0);
});

/** Page eval that answers the visibility probe per target page. */
function pageVisibility(world, answerFor) {
  const original = world.app.page.eval;
  const probed = [];
  world.app.page.eval = async (options) => {
    if (/visibilityState/.test(options.script) && /requestAnimationFrame/.test(options.script)) {
      probed.push(options.page);
      return answerFor(options.page);
    }
    return original(options);
  };
  return probed;
}
const HIDDEN = { state: "hidden", animationFrames: false };
const VISIBLE = { state: "visible", animationFrames: true };

async function missOn(locator) {
  try {
    await locator.waitFor({ timeout: 60 });
    return "found";
  } catch (error) {
    // Evidence is a separate operation; it never lengthens waitFor itself.
    const note = await locator.hiddenPageNote();
    return [error.message, note].filter(Boolean).join("\n");
  }
}

test("hidden-page evidence belongs to the page it was observed on", async () => {
  const world = createWorld();
  const probed = pageVisibility(world, (page) => (page === "cart" ? HIDDEN : VISIBLE));
  installFakeHost(world);
  const messages = [];

  spec("two pages", { forensics: false }, async (t) => {
    await t.app.nav.to({ page: "cart" });
    await t.app.nav.to({ page: "profile" });
    // Hidden first, then visible: the visible page is not called hidden.
    messages.push(await missOn(t.app.view.page("cart").testId("missing")));
    messages.push(await missOn(t.app.view.page("profile").testId("missing")));
    // Visible first, then hidden again: the hidden page is still named.
    messages.push(await missOn(t.app.view.page("profile").testId("missing")));
    messages.push(await missOn(t.app.view.page("cart").testId("missing")));
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.match(messages[0], /page looked hidden/);
  assert.doesNotMatch(messages[1], /hidden/);
  assert.doesNotMatch(messages[2], /hidden/);
  assert.match(messages[3], /page looked hidden or paused: .* \[observed on this page \d+ms earlier\]/);
  // The visible page is probed each time (cheap); the hidden one once.
  assert.deepEqual(probed, ["cart", "profile", "profile"]);
});

test("a navigation invalidates hidden-page evidence", async () => {
  const world = createWorld();
  let hidden = true;
  const probed = pageVisibility(world, () => (hidden ? HIDDEN : VISIBLE));
  installFakeHost(world);
  const messages = [];

  spec("navigates", { forensics: false }, async (t) => {
    messages.push(await missOn(t.app.view.testId("missing")));
    hidden = false;
    await t.app.nav.to({ page: "next" });
    messages.push(await missOn(t.app.view.testId("missing")));
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.match(messages[0], /page looked hidden/);
  assert.doesNotMatch(messages[1], /hidden/, "the new page instance is probed, not described by the old one");
  assert.equal(probed.length, 2);
});

test("a page that became visible is re-observed once the earlier sample is stale", async () => {
  const world = createWorld();
  let hidden = true;
  const probed = pageVisibility(world, () => (hidden ? HIDDEN : VISIBLE));
  installFakeHost(world);
  const messages = [];

  spec("hidden then visible", { forensics: false, timeout: 10_000 }, async (t) => {
    messages.push(await missOn(t.app.view.testId("missing")));
    hidden = false;
    await new Promise((resolve) => setTimeout(resolve, 2_100));
    messages.push(await missOn(t.app.view.testId("missing")));
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.match(messages[0], /page looked hidden/);
  assert.doesNotMatch(messages[1], /hidden/);
  assert.equal(probed.length, 2);
});

test("a hidden page on a locked screen names the lock", async () => {
  const world = createWorld();
  world.add({ testId: "sheet", visible: false, text: "" });
  hidePage(world);
  const { attachments } = installFakeHost(world);
  let locked = false;
  globalThis.__LINGXIA_AUTOMATION_HOST__.screenLocked = () => locked;
  let message;

  spec("locks mid-spec", { forensics: false }, async (t) => {
    locked = true;
    message = await missOn(t.app.view.testId("sheet"));
    await t.app.view.testId("sheet").waitFor({ timeout: 80 });
  });

  await globalThis.__LINGXIA_TEST__.run();
  assert.match(message, /the screen is locked; unlock it — animations and sheets are paused/);
  assert.doesNotMatch(message, /page looked hidden/);
  assert.match(failedMessage(attachments), /Timed out/);
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

test("view.page(name) binds locators, eval, screenshot and scroll to a page; calls cannot change the target", async () => {
  const world = createWorld();
  world.add({ testId: "todo-input" });
  installFakeHost(world);
  const queried = [];
  const query = world.app.page.query;
  world.app.page.query = (options) => { queried.push(options.page); return query(options); };
  const shots = [];
  const screenshot = world.app.page.screenshot;
  world.app.page.screenshot = (options) => { shots.push(options?.page); return screenshot(options); };

  spec("bound", { forensics: false }, async (t) => {
    const todo = t.app.view.page("todo");
    await todo.testId("todo-input").fill("milk");
    await expect(todo.testId("todo-input")).toHaveValue("milk");
    assert.throws(() => todo.testId("todo-input", { page: "other" }), /bound page/);
    await todo.eval(({ document }) => document.title);
    assert.throws(() => todo.eval({ page: "cart" }, ({ document }) => document.title), /bound page/);
    await todo.screenshot();
    await t.app.view.screenshot();
    await t.reject(() => Promise.resolve().then(() => t.app.view.page("")), { message: /takes a configured page name/ });
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.ok(queried.includes("todo") && !queried.includes("other"), JSON.stringify(queried));
  assert.ok(!queried.includes(undefined), JSON.stringify(queried));
  assert.ok(world.evaluatedPages.includes("todo"));
  assert.ok(!world.evaluatedPages.includes("cart"));
  assert.deepEqual(shots, ["todo", undefined]);
});
