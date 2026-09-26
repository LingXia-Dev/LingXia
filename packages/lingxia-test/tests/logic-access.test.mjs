import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset } from "../dist/index.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

function logicWorld(pages = []) {
  const world = createWorld();
  const lx = { env: { USER_DATA_PATH: "lx://userdata" } };
  world.useLogic({ lx, getApp: () => ({ name: "demo" }), getCurrentPages: () => pages });
  return world;
}

async function runOne(body, options = {}) {
  spec("under test", { forensics: false, ...options }, body);
  const report = await globalThis.__LINGXIA_TEST__.run();
  return report.cases[0];
}

test("function eval runs in Logic with the scope and JSON args", async () => {
  const world = logicWorld([{ route: "pages/home/index", data: { count: 2 } }]);
  installFakeHost(world);
  const seen = [];

  const result = await runOne(async (t) => {
    seen.push(await t.app.logic.eval(({ lx, getApp, getCurrentPages }, extra, label) => ({
      path: lx.env.USER_DATA_PATH,
      app: getApp().name,
      count: getCurrentPages()[0].data.count + extra.add,
      label,
    }), { add: 3 }, "x \"y"));
    // A leading comment, which the string form's statement detection misses.
    seen.push(await t.app.logic.eval(() => {
      // just a comment first
      return 7;
    }));
    seen.push(await t.app.logic.eval(async () => undefined));
  });

  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.deepEqual(seen, [
    { path: "lx://userdata", app: "demo", count: 5, label: "x \"y" },
    7,
    undefined,
  ]);
  const action = result.steps.find((step) => step.name === "logic.eval");
  assert.match(action.detail, /^\(\{ lx, getApp, getCurrentPages \}, extra, label\) =>/);
  // The script sent is one call expression, not a statement body.
  assert.match(world.evaluated[0], /^\(\(__lxFn, __lxArgs\) => __lxFn\(/);
});

test("the deprecated string form still works on t.app.eval", async () => {
  const world = createWorld();
  world.setEval("return 1", 1);
  installFakeHost(world);
  let value;
  const result = await runOne(async (t) => {
    value = await t.app.eval({ script: "return 1" });
  });
  assert.equal(result.status, "passed");
  assert.equal(value, 1);
});

test("a closure over spec variables fails with an explanation", async () => {
  const world = logicWorld();
  installFakeHost(world);
  const specLocal = 42;
  const result = await runOne(async (t) => {
    await t.app.logic.eval(() => specLocal + 1);
  });
  assert.equal(result.status, "failed");
  assert.equal(result.error.name, "ReferenceError");
  assert.equal(result.error.code, "E_EVAL");
  assert.match(result.error.message, /specLocal is not defined/);
  assert.match(result.error.message, /cannot use variables, imports or helpers of the spec/);
});

test("methods and bound functions are rejected before anything is sent", async () => {
  const world = logicWorld();
  installFakeHost(world);
  const holder = { read() { return 1; } };
  const messages = [];
  const result = await runOne(async (t) => {
    for (const fn of [holder.read, (() => 1).bind(null)]) {
      try {
        await t.app.logic.eval(fn);
      } catch (error) {
        messages.push(error.message);
      }
    }
  });
  assert.equal(result.status, "passed");
  assert.match(messages[0], /not a method or class/);
  assert.match(messages[1], /bound or native function/);
  assert.deepEqual(world.evaluated, []);
});

test("view function eval gets document and window", async () => {
  const world = createWorld();
  world.usePage({ document: { title: "Home" }, window: { innerWidth: 390 } });
  installFakeHost(world);
  let value;
  const result = await runOne(async (t) => {
    value = await t.app.view.eval(({ document, window }, suffix) => `${document.title}:${window.innerWidth}${suffix}`, "!");
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.equal(value, "Home:390!");
  assert.ok(result.steps.some((step) => step.name === "view.eval"));
});

test("the fixture takes eval functions; a script string names the raw driver", async () => {
  const world = logicWorld();
  world.usePage({ document: { title: "Home" }, window: {} });
  installFakeHost(world);
  const messages = [];
  const result = await runOne(async (t) => {
    for (const call of [() => t.app.logic.eval({ script: "1" }), () => t.app.view.eval({ script: "1" })]) {
      try {
        await call();
      } catch (error) {
        messages.push(error.message);
      }
    }
    // The deprecated page alias keeps both forms.
    messages.push(await t.app.page.eval(({ document }) => document.title));
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.match(messages[0], /t\.app\.logic\.eval\(fn, \.\.\.args\) takes a function.*lx\.automation\(\)\.lxapp\(\)\.eval\(\{ script \}\)/);
  assert.match(messages[1], /t\.app\.view\.eval\(fn, \.\.\.args\) takes a function/);
  assert.equal(messages[2], "Home");
  // Only the page alias reached a target.
  assert.equal(world.evaluated.length, 1);
});

test("logic.data reads the current or a named page, logic.call invokes a method", async () => {
  const calls = [];
  const pages = [
    { route: "pages/home/index", data: { greeting: "hi" } },
    {
      route: "/pages/devices/index",
      data: { devices: [{ id: "d1" }] },
      async rename(id, patch) {
        calls.push([id, patch]);
        return { ok: true, id };
      },
    },
  ];
  const world = logicWorld(pages);
  installFakeHost(world);
  const seen = {};
  const result = await runOne(async (t) => {
    seen.current = await t.app.logic.data();
    seen.home = await t.app.logic.data({ page: "home" });
    seen.byRoute = await t.app.logic.data({ page: "pages/devices/index" });
    seen.renamed = await t.app.logic.call("rename", "d1", { name: "Office" });
    await t.reject(() => t.app.logic.data({ page: "settings" }), { message: "t.app.logic.data: page \"settings\" is not in the page stack" });
    await t.reject(() => t.app.logic.call("missing"), { message: 't.app.logic.call: page "/pages/devices/index" has no method "missing"' });
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.deepEqual(seen.current, { devices: [{ id: "d1" }] });
  assert.deepEqual(seen.home, { greeting: "hi" });
  assert.deepEqual(seen.byRoute, { devices: [{ id: "d1" }] });
  assert.deepEqual(seen.renamed, { ok: true, id: "d1" });
  assert.deepEqual(calls, [["d1", { name: "Office" }]]);
  const names = result.steps.map((step) => step.name);
  assert.ok(names.includes("logic.data"));
  assert.ok(names.includes("logic.call"));
  // One row per call: the underlying eval is not traced twice.
  assert.ok(!names.includes("logic.eval"));
});

test("waitFor resolves to the accepted value and traces one row", async () => {
  installFakeHost(createWorld());
  let reads = 0;
  let value;
  const result = await runOne(async (t) => {
    value = await t.waitFor(async function readCount() {
      reads += 1;
      if (reads < 2) throw new Error("not loaded yet");
      return reads;
    }, { until: (count) => count >= 4, timeout: 1_000, interval: 5 });
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.equal(value, 4);
  const rows = result.steps.filter((step) => step.name === "waitFor");
  assert.equal(rows.length, 1);
  assert.equal(rows[0].detail, "readCount");
});

test("waitFor defaults to a truthy value", async () => {
  installFakeHost(createWorld());
  let reads = 0;
  let value;
  const result = await runOne(async (t) => {
    value = await t.waitFor(() => (++reads >= 3 ? "ready" : ""), { interval: 5 });
  });
  assert.equal(result.status, "passed");
  assert.equal(value, "ready");
});

test("waitFor fails fast on programming errors", async () => {
  installFakeHost(createWorld());
  let reads = 0;
  const started = Date.now();
  const result = await runOne(async (t) => {
    await t.waitFor(() => {
      reads += 1;
      return undefined.devices;
    });
  });
  assert.equal(result.status, "failed");
  assert.equal(result.error.name, "TypeError");
  assert.equal(reads, 1);
  assert.ok(Date.now() - started < 1_000);
});

test("waitFor honours retryIf", async () => {
  installFakeHost(createWorld());
  let reads = 0;
  const result = await runOne(async (t) => {
    await t.waitFor(() => {
      reads += 1;
      throw new Error("permanent");
    }, { retryIf: () => false });
  });
  assert.equal(result.status, "failed");
  assert.equal(result.error.message, "permanent");
  assert.equal(reads, 1);
});

test("waitFor timeout names the last value and the last error", async () => {
  installFakeHost(createWorld());
  const result = await runOne(async (t) => {
    await t.waitFor(() => ({ state: "loading" }), { until: (value) => value.state === "done", timeout: 60, interval: 5 });
  });
  assert.equal(result.status, "failed");
  assert.equal(result.error.name, "TimeoutError");
  assert.equal(result.error.code, "E_TIMEOUT");
  assert.match(result.error.message, /t\.waitFor timed out after \d+ms/);
  assert.match(result.error.message, /Last value: .*loading/);
  assert.match(result.error.message, /rejected by until/);

  reset();
  installFakeHost(createWorld());
  const errored = await runOne(async (t) => {
    await t.waitFor(() => { throw new Error("503 from status"); }, { timeout: 60, interval: 5 });
  });
  assert.match(errored.error.message, /Last error: Error: 503 from status/);
});

test("waitFor is clamped to the remaining spec budget", async () => {
  installFakeHost(createWorld());
  const started = Date.now();
  const result = await runOne(async (t) => {
    await t.waitFor(() => false, { timeout: 10_000, interval: 5 });
  }, { timeout: 400 });
  // The wait's own failure, carrying its last value, beats the spec timer.
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /Clamped from 10000ms/);
  assert.match(result.error.message, /Last value: false/);
  assert.ok(Date.now() - started < 2_000);
});

test("waitFor refuses a positional acceptance callback", async () => {
  installFakeHost(createWorld());
  const result = await runOne(async (t) => {
    await t.waitFor(() => 1, (value) => value > 0);
  });
  assert.equal(result.status, "failed");
  assert.match(result.error.message, /takes its acceptance test as `until`/);
});

test("t.arg reads, defaults, or names the missing --arg", async () => {
  installFakeHost(createWorld(), { args: { baseUrl: "http://fixture" } });
  const seen = {};
  const result = await runOne(async (t) => {
    seen.present = t.arg("baseUrl");
    seen.defaulted = t.arg("mode", { default: "mock" });
    seen.optional = t.arg("token", { required: false });
    seen.missingField = t.args.token;
    try {
      t.arg("statusUrl");
    } catch (error) {
      seen.error = error.message;
    }
  });
  assert.equal(result.status, "passed");
  assert.equal(seen.present, "http://fixture");
  assert.equal(seen.defaulted, "mock");
  assert.equal(seen.optional, undefined);
  assert.equal(seen.missingField, undefined);
  assert.match(seen.error, /Missing test arg "statusUrl": pass --arg statusUrl=<value>/);
  assert.doesNotMatch(seen.error, /case-sensitive/);
});

test("t.arg names a key given in another case, without reading it", async () => {
  installFakeHost(createWorld(), { args: { PASSWORD: "from-env" } });
  const seen = {};
  const result = await runOne(async (t) => {
    seen.optional = t.arg("password", { required: false });
    try {
      t.arg("password");
    } catch (error) {
      seen.error = error.message;
    }
  });
  assert.equal(result.status, "passed");
  assert.equal(seen.optional, undefined);
  assert.match(seen.error, /Missing test arg "password".*"PASSWORD" was given, and arg keys are case-sensitive/);
  assert.doesNotMatch(seen.error, /from-env/);
});
