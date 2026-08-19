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
  assert.equal(protocol.failed, 1);
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
