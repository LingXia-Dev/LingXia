import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect, reset, DEFAULT_ACTION_TIMEOUT_MS, DEFAULT_SPEC_TIMEOUT_MS } from "../dist/index.js";

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

function reportJson(attachments) {
  return JSON.parse(decodeAttachment(attachments, "report.json"));
}

test("registers specs and runs them sequentially through __LINGXIA_TEST__.run", async () => {
  const world = createWorld();
  installFakeHost(world);
  const order = [];

  spec("first case", async () => {
    order.push("first");
  });
  spec("second case", async () => {
    order.push("second");
  });

  assert.equal(typeof globalThis.test, "undefined");
  assert.equal(typeof globalThis.describe, "undefined");
  assert.equal(typeof globalThis.__LINGXIA_TEST__.run, "function");
  assert.notEqual(globalThis.__LINGXIA_TEST__, globalThis.__RONG_TEST__);

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(order, ["first", "second"]);
  assert.equal(protocol.total, 2);
  assert.equal(protocol.passed, 2);
  assert.equal(protocol.failed, 0);
});

test("nests t.step in report.json and emits covers on case_started", async () => {
  const world = createWorld();
  const { events, attachments } = installFakeHost(world);

  spec("tab bar rejects a bad index", { covers: ["lx.tabBar.update"], id: "UI-TABBAR-001" }, async (t) => {
    await t.step("outer", async () => {
      await t.step("inner", async () => {
        expect(1).toBe(1);
      });
    });
  });

  await globalThis.__LINGXIA_TEST__.run();
  const started = events.find((event) => event.type === "case_started");
  assert.ok(started);
  assert.deepEqual(started.covers, ["lx.tabBar.update"]);
  assert.equal(started.timeout_ms, DEFAULT_SPEC_TIMEOUT_MS);

  const report = reportJson(attachments);
  const steps = report.cases[0].steps;
  assert.equal(steps[0].name, "outer");
  assert.equal(steps[0].steps[0].name, "inner");
  assert.equal(steps[0].steps[0].path, "outer > inner");
  assert.equal(steps[0].steps[0].assertions[0].matcher, "toBe");
  assert.equal(steps[0].steps[0].assertions[0].passed, true);
  assert.deepEqual(report.cases[0].covers, ["lx.tabBar.update"]);
  assert.equal(report.cases[0].id, "UI-TABBAR-001");
});

test("ASCII titles slug; non-ASCII titles use file-n unless id is set", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world);

  spec("Home greets by name", async () => {});
  spec("首页打招呼", async () => {});
  spec("中文稳定 id", { id: "UI-HOME-009" }, async () => {});

  await globalThis.__LINGXIA_TEST__.run();
  const report = reportJson(attachments);
  assert.equal(report.cases[0].id, "home-greets-by-name");
  assert.match(report.cases[1].id, /^clock-\d+$/);
  assert.equal(report.cases[2].id, "UI-HOME-009");
});

test("grep filters by title or id and marks the report filtered", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, { args: { grep: "keep-me|UI-KEEP" } });
  const ran = [];
  spec("keep-me visible", async () => {
    ran.push("title");
  });
  spec("other case", { id: "UI-KEEP" }, async () => {
    ran.push("id");
  });
  spec("skip this", async () => {
    ran.push("no");
  });
  await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(ran, ["title", "id"]);
  assert.equal(reportJson(attachments).filtered, true);
});

test("spec.only runs only those cases; forbidOnly refuses to start", async () => {
  const world = createWorld();
  installFakeHost(world);
  const ran = [];
  spec("background", async () => {
    ran.push("background");
  });
  spec.only("focused", async () => {
    ran.push("focused");
  });
  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(ran, ["focused"]);
  assert.equal(protocol.total, 1);

  reset();
  const world2 = createWorld();
  installFakeHost(world2, { args: { forbidOnly: "1" } });
  spec.only("must not run", async () => {
    ran.push("only");
  });
  await assert.rejects(() => globalThis.__LINGXIA_TEST__.run(), /forbid-only/);
});

test("a timeout forces the next spec to relaunch home", async () => {
  const world = createWorld();
  installFakeHost(world);
  const pages = [];
  spec("times out", { timeout: 40, forensics: false }, async () => {
    await new Promise((resolve) => setTimeout(resolve, 120));
  });
  spec("after timeout", async (t) => {
    pages.push((await t.app.nav.current()).name);
  });
  const original = world.app.nav.relaunch.bind(world.app.nav);
  let relaunched = 0;
  world.app.nav.relaunch = async (options) => {
    relaunched += 1;
    return original(options);
  };
  await globalThis.__LINGXIA_TEST__.run();
  assert.equal(relaunched, 1);
});

test("duplicate spec ids fail before any case runs", async () => {
  const world = createWorld();
  installFakeHost(world);
  const ran = [];
  spec("one", { id: "dup" }, async () => {
    ran.push("one");
  });
  spec("two", { id: "dup" }, async () => {
    ran.push("two");
  });
  await assert.rejects(() => globalThis.__LINGXIA_TEST__.run(), /Duplicate spec id/);
  assert.deepEqual(ran, []);
});

test("cleanup gets the spec's budget, and only a wedged spec gets the short one", async () => {
  const world = createWorld();
  installFakeHost(world);
  const cleaned = [];

  spec("slow cleanup on a healthy spec", { timeout: 20_000 }, async (t) => {
    t.defer(async () => {
      // Longer than the 2s post-timeout budget; a healthy spec must still finish.
      await new Promise((resolve) => setTimeout(resolve, 2_600));
      await t.app.eval({ script: "1" });
      cleaned.push("healthy");
    });
  });

  spec("wedged spec bails out of cleanup", { timeout: 60 }, async (t) => {
    t.defer(async () => {
      await new Promise((resolve) => setTimeout(resolve, 2_600));
      await t.app.eval({ script: "1" });
      cleaned.push("wedged");
    });
    await new Promise((resolve) => setTimeout(resolve, 300));
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.deepEqual(cleaned, ["healthy"]);
  assert.equal(protocol.cases[0].status, "passed");
  assert.equal(protocol.cases[1].status, "failed");
});
