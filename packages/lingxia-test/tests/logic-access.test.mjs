import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect } from "../dist/index.js";
import { reset } from "../dist/runner.js";

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
  assert.match(world.evaluated[0], /^\(\(__lxFn, __lxArgs, __lxResult\) => Promise\.resolve\(__lxFn\(/);
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

test("arguments that are not JSON are refused with their path before anything is sent", async () => {
  const world = logicWorld([{ route: "pages/home/index", data: {}, save() { return 1; } }]);
  world.usePage({ document: { title: "Home" }, window: {} });
  installFakeHost(world);
  class Point { constructor() { this.x = 1; } }
  const cyclic = { name: "loop" };
  cyclic.self = cyclic;
  const messages = [];
  const result = await runOne(async (t) => {
    const home = await t.app.page();
    const attempts = [
      () => t.app.logic.eval((_, a, b) => [a, b], 1, { items: [1, 2, undefined] }),
      () => t.app.logic.eval((_, a) => a, [() => 1]),
      () => t.app.logic.eval((_, a) => a, { drop: undefined }),
      () => t.app.logic.eval((_, a) => a, { total: NaN }),
      () => t.app.logic.eval((_, a) => a, [1, Infinity]),
      () => t.app.logic.eval((_, a) => a, { "odd key": { at: new Date(0) } }),
      () => t.app.logic.eval((_, a) => a, new Point()),
      () => t.app.logic.eval((_, a) => a, cyclic),
      () => t.app.logic.eval((_, a) => a, [1, , 3]), // eslint-disable-line no-sparse-arrays
      () => t.app.logic.eval((_, a) => a, 10n),
      () => home.actions.save({ onDone: () => 1 }),
      () => t.app.view.eval((_, a) => a, { items: [undefined] }),
    ];
    for (const attempt of attempts) {
      try {
        await attempt();
        messages.push("sent");
      } catch (error) {
        messages.push(`${error.name}: ${error.message.split(" is not JSON")[0]}`);
      }
    }
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.deepEqual(messages, [
    "TypeError: t.app.logic.eval: args[1][\"items\"][2]: undefined",
    "TypeError: t.app.logic.eval: args[0][0]: function",
    "TypeError: t.app.logic.eval: args[0][\"drop\"]: undefined",
    "TypeError: t.app.logic.eval: args[0][\"total\"]: non-finite number; use JSON values",
    "TypeError: t.app.logic.eval: args[0][1]: non-finite number; use JSON values",
    "TypeError: t.app.logic.eval: args[0][\"odd key\"][\"at\"]: non-plain object",
    "TypeError: t.app.logic.eval: args[0]: non-plain object",
    "TypeError: t.app.logic.eval: args[0][\"self\"]: circular reference; use JSON values",
    "TypeError: t.app.logic.eval: args[0]: array index 1 is missing or an accessor; use JSON values",
    "TypeError: t.app.logic.eval: args[0]: bigint",
    "TypeError: page.actions.save: args[0][\"onDone\"]: function",
    "TypeError: t.app.view.eval: args[0][\"items\"][0]: undefined"
  ]);
  // Nothing reached either side.
  assert.deepEqual(world.evaluated, []);
  assert.deepEqual(world.evaluatedPages, []);
  assert.deepEqual(world.actionCalls, []);
});

test("JSON arguments cross unchanged, shared references included", async () => {
  const world = logicWorld();
  installFakeHost(world);
  const shared = { id: 1 };
  let value;
  const result = await runOne(async (t) => {
    value = await t.app.logic.eval((_, a) => a, { a: shared, b: shared, list: [null, -1.5, "", false], proto: Object.create(null) });
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.deepEqual(value, { a: { id: 1 }, b: { id: 1 }, list: [null, -1.5, "", false], proto: {} });
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

test("view eval takes only { timeout }: `page` is a TypeError; a bound page's view evals on its instance", async () => {
  const world = createWorld();
  world.usePage({ document: { title: "Surface" }, window: {} });
  installFakeHost(world);
  const values = [];
  const messages = [];
  let surfaceId;
  const result = await runOne(async (t) => {
    await t.app.nav.to({ page: "surface" });
    const surface = await t.app.page({ name: "surface" });
    surfaceId = surface.instanceId;
    values.push(await surface.view.eval(({ document }, suffix) => document.title + suffix, "!"));
    values.push(await t.app.view.eval(({ document }) => document.title));
    for (const view of [t.app.view, surface.view]) {
      try { await view.eval({ page: "surface" }, ({ document }) => document.title); } catch (error) { messages.push(`${error.name}: ${error.message}`); }
    }
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.deepEqual(values, ["Surface!", "Surface"]);
  assert.deepEqual(world.evaluatedPages, [surfaceId, undefined]);
  assert.deepEqual(messages, [
    "TypeError: t.app.view.eval: page is not an eval option; bind the page with t.app.page({ name }) and use its view",
    "TypeError: page.view.eval: page is not an eval option; bind the page with t.app.page({ name }) and use its view",
  ]);
  const actions = result.steps.filter((step) => step.name === "view.eval");
  assert.match(actions[0].detail, new RegExp(`^#${surfaceId} `));
});

test("an eval's own timeout is sent, clamped to the spec budget", async () => {
  const world = logicWorld();
  world.usePage({ document: { title: "Home" }, window: {} });
  installFakeHost(world);
  let surfaceId;
  const result = await runOne(async (t) => {
    await t.app.logic.eval(() => 1);
    await t.app.logic.eval({ timeout: 20_000 }, (_, n) => n, 2);
    await t.app.logic.eval({ timeout: 600_000 }, () => 3);
    await t.app.nav.to({ page: "surface" });
    const surface = await t.app.page({ name: "surface" });
    await surface.view.eval({ timeout: 15_000 }, ({ document }) => document.title);
    await surface.view.eval(({ document }) => document.title);
    assert.throws(() => t.app.logic.eval({ timeout: -1 }, () => 4), /positive number of ms/);
    surfaceId = surface.instanceId;
  }, { timeout: 60_000 });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  const [byDefault, own, clamped, view, viewDefault] = world.evalTimeouts;
  // Without its own timeout an eval gets a fixed 10 s, not a share of the budget.
  assert.equal(byDefault, 10_000);
  assert.equal(own, 20_000);
  assert.ok(clamped > 50_000 && clamped < 60_000, String(clamped));
  assert.equal(view, 15_000);
  assert.equal(viewDefault, 10_000);
  assert.equal(world.evaluatedPages.at(-1), surfaceId);
});

test("the default eval timeout is clamped to what the spec has left", async () => {
  const world = logicWorld();
  world.usePage({ document: { title: "Home" }, window: {} });
  installFakeHost(world);
  const result = await runOne(async (t) => {
    await t.app.logic.eval(() => 1);
    await t.app.view.eval(({ document }) => document.title);
  }, { timeout: 3_000 });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.equal(world.evalTimeouts.length, 2);
  for (const timeout of world.evalTimeouts) assert.ok(timeout > 2_000 && timeout < 3_000, String(timeout));
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
    messages.push(await t.app.view.eval(({ document }) => document.title));
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.match(messages[0], /t\.app\.logic\.eval\(fn, \.\.\.args\) takes a function.*rawAutomation\(\)\.lxapp\(\)\.eval\(\{ script \}\) from @lingxia\/test/);
  assert.match(messages[1], /t\.app\.view\.eval\(fn, \.\.\.args\) takes a function/);
  assert.equal(messages[2], "Home");
  // Only the function reached a target.
  assert.equal(world.evaluated.length, 1);
});

test("page.data() reads the bound instance's Logic data; page.actions invoke its public actions", async () => {
  const world = createWorld();
  world.setPageData("home", { greeting: "hi" });
  world.setPageData("devices", { devices: [{ id: "d1" }] });
  const calls = [];
  world.setAction("rename", (payload, page) => { calls.push([page.instanceId, payload]); return { ok: true, id: payload.id }; });
  world.setAction("refresh", (payload) => ({ refreshed: payload === undefined }));
  world.setAction("broken", () => { throw new ReferenceError("helper is not defined"); });
  world.setAction("quota", () => { throw Object.assign(new Error("over quota"), { code: "E_QUOTA" }); });
  world.setAction("slow", () => { throw Object.assign(new Error("action did not settle"), { code: "E_AUTOMATION_TIMEOUT" }); });
  installFakeHost(world);
  const seen = {};
  const result = await runOne(async (t) => {
    const home = await t.app.page();
    await t.app.nav.to({ page: "devices" });
    const current = await t.app.page();
    const byName = await t.app.page({ name: "home" });
    const byId = await t.app.page({ instanceId: current.instanceId });
    seen.ids = [home.instanceId, current.instanceId, byName.instanceId, byId.instanceId];
    seen.names = [home.name, current.name];
    seen.home = await home.data();
    seen.current = await current.data();
    seen.byId = await byId.data();
    seen.renamed = await current.actions.rename({ id: "d1", name: "Office" });
    seen.refreshed = await home.actions.refresh();
    await t.reject(() => current.actions.missing(), { code: "E_PAGE_ACTION", message: 'no public action "missing"' });
    seen.quota = await t.reject(() => current.actions.quota(), { code: "E_QUOTA" });
    seen.slow = await t.reject(() => current.actions.slow(), { code: "E_TIMEOUT" });
    seen.broken = await t.reject(() => current.actions.broken(), { code: "E_PAGE_ACTION" });
    seen.removed = [typeof t.app.logic.data, typeof t.app.logic.call];
    seen.thenable = typeof current.actions.then;
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  const [homeId, devicesId] = seen.ids;
  assert.deepEqual(seen.ids, [homeId, devicesId, homeId, devicesId]);
  assert.deepEqual(seen.names, ["home", "devices"]);
  assert.deepEqual(seen.home, { greeting: "hi" });
  assert.deepEqual(seen.current, { devices: [{ id: "d1" }] });
  assert.deepEqual(seen.byId, seen.current);
  assert.deepEqual(seen.renamed, { ok: true, id: "d1" });
  assert.deepEqual(seen.refreshed, { refreshed: true });
  assert.deepEqual(seen.removed, ["undefined", "undefined"]);
  assert.equal(seen.thenable, "undefined", "awaiting page.actions must not call an action named then");
  assert.deepEqual(calls, [[devicesId, { id: "d1", name: "Office" }]]);
  // The app's own code reaches the spec; a driver timeout is E_TIMEOUT with the driver's code as its cause.
  assert.equal(seen.quota.message, "over quota");
  assert.equal(seen.slow.name, "TimeoutError");
  assert.equal(seen.slow.cause.code, "E_AUTOMATION_TIMEOUT");
  // A failure inside the app's action is the app's: no "closes over the spec" advice.
  assert.equal(seen.broken.message, "page action broken rejected: ReferenceError: helper is not defined");
  assert.equal(seen.broken.name, "Error");
  assert.deepEqual(seen.broken.data, { action: "broken", cause: "ReferenceError: helper is not defined" });
  // The driver gets the bound instance, the payload only when one was given, and the spec's room.
  const [rename, refresh] = world.actionCalls;
  assert.deepEqual({ ...rename, timeoutMs: typeof rename.timeoutMs }, { page: devicesId, name: "rename", payload: { id: "d1", name: "Office" }, timeoutMs: "number" });
  assert.deepEqual(Object.keys(refresh).sort(), ["name", "page", "timeoutMs"]);
  assert.ok(rename.timeoutMs > 0 && rename.timeoutMs <= 30_000, String(rename.timeoutMs));
  const names = result.steps.map((step) => step.name);
  assert.ok(names.includes("page.bind"));
  assert.ok(names.includes("page.data"));
  assert.ok(names.includes("page.action"));
  assert.ok(result.steps.some((step) => step.name === "page.action" && step.detail === `rename #${devicesId}`));
  // One row per call: the underlying eval is not traced twice.
  assert.ok(!names.includes("logic.eval"));
});

test("page.actions take at most one JSON payload", async () => {
  const world = createWorld();
  world.setAction("move", () => "moved");
  installFakeHost(world);
  const messages = [];
  const result = await runOne(async (t) => {
    const page = await t.app.page();
    await t.reject(() => page.actions.move(1, 2), { message: "page.actions.move takes at most one JSON payload" });
    try { await page.actions.move(new Date(0)); } catch (error) { messages.push(`${error.name}: ${error.message}`); }
    messages.push(await page.actions.move({ to: 2 }));
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.match(messages[0], /^TypeError: page\.actions\.move: args\[0\]: non-plain object/);
  assert.equal(messages[1], "moved");
  assert.equal(world.actionCalls.length, 1);
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
    seen.duplicates = ["args", "apps", "profile"].filter((name) => name in t);
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
  assert.deepEqual(seen.duplicates, [], "one reader per input: t.arg, t.app.profile, t.automation.lxapp");
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

for (const [name, fn] of [
  ["array undefined", () => [undefined]],
  ["object undefined", () => ({ missing: undefined })],
  ["date", () => new Date(0)],
  ["method", () => ({ save() {} })],
  ["non-finite number", () => ({ n: Infinity })],
  ["cycle", () => { const item = {}; item.self = item; return item; }],
  ["accessor", () => ({ get value() { return 1; } })],
]) {
  test(`eval refuses a lossy ${name} result in Logic and View`, async () => {
    const world = logicWorld();
    world.usePage({ document: {}, window: {} });
    installFakeHost(world);
    const failures = [];
    const result = await runOne(async (t) => {
      for (const target of [t.app.logic, t.app.view]) {
        try { await target.eval(fn); }
        catch (error) { failures.push(error.message); }
      }
    });
    assert.equal(result.status, "passed", JSON.stringify(result.error));
    assert.equal(failures.length, 2);
    for (const message of failures) assert.match(message, /result.*JSON|result.*reference|result.*accessor/);
  });
}

test("JSON arguments reject before dispatch and do not invoke getters", async () => {
  const world = logicWorld();
  installFakeHost(world);
  let reads = 0;
  const result = await runOne(async (t) => {
    for (const value of [undefined, [undefined], new Date(0), { get id() { reads++; return 1; } }]) {
      assert.throws(() => t.app.logic.eval((_, item) => item, value), TypeError);
    }
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.equal(reads, 0);
  assert.deepEqual(world.evaluated, []);
});

test("JSON results preserve tuples, repeated references and a literal __proto__ key", async () => {
  installFakeHost(logicWorld());
  let result;
  const outcome = await runOne(async (t) => {
    result = await t.app.logic.eval(() => {
      const item = { id: 1 };
      return { list: [item, item], ["__proto__"]: "data" };
    });
  });
  assert.equal(outcome.status, "passed", JSON.stringify(outcome.error));
  assert.deepEqual(result, JSON.parse('{"list":[{"id":1},{"id":1}],"__proto__":"data"}'));
});

test("JSON arguments keep special keys as own data in Logic and View", async () => {
  const world = logicWorld();
  world.usePage({ document: {}, window: {} });
  installFakeHost(world);
  const payload = JSON.parse('{"__proto__":{"role":"admin"},"nested":{"__proto__":null},"nodeType":1,"label":"quotes \\" and \\\\ and \\n"}');
  const seen = [];
  const result = await runOne(async (t) => {
    for (const target of [t.app.logic, t.app.view]) {
      seen.push(await target.eval((_, value) => ({
        own: Object.prototype.hasOwnProperty.call(value, "__proto__"),
        hasRole: "role" in value,
        ordinary: Object.getPrototypeOf(value) === Object.prototype,
        value,
      }), payload));
    }
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.deepEqual(seen, [
    { own: true, hasRole: false, ordinary: true, value: payload },
    { own: true, hasRole: false, ordinary: true, value: payload },
  ]);
});


test("page handles keep the selected identity; their view takes no page option", async () => {
  const world = logicWorld();
  world.app.nav.info = async () => ({ path: 'pages/editor/index', instanceId: 'editor:1' });
  world.app.nav.stack = async () => [{ path: 'pages/editor/index', instanceId: 'editor:1' }];
  world.stack.push({ path: 'pages/editor/index', instanceId: 'editor:1' });
  world.useLogic({ lx: {}, __lxGetPage: id => {
    assert.equal(id, 'editor:1');
    return { data: { title: 'Before' } };
  } });
  world.usePage({ document: { title: 'Editor' }, window: {} });
  world.app.page.action = async (options) => { world.actionCalls.push(options); return true; };
  installFakeHost(world);
  const result = await runOne(async t => {
    const page = await t.app.page({ name: 'editor' });
    assert.equal(page.instanceId, 'editor:1');
    assert.equal(page.name, 'pages/editor/index', 'a page the host does not name is named by its path');
    assert.deepEqual(await page.data(), { title: 'Before' });
    assert.equal(await page.actions.rename({ title: 'After' }), true);
    assert.throws(() => page.view.css('button', { page: 'other' }), /view\.css\(\) takes no options: bind another page with t\.app\.page\(\{ name \}\)/);
    assert.throws(() => page.view.testId('save', {}), /view\.testId\(\) takes no options/);
    assert.throws(() => page.view.screenshot({ page: 'other' }), /view\.screenshot\(\) takes no options/);
    assert.equal(await page.view.eval(({ document }) => document.title), 'Editor');
  });
  assert.equal(result.status, 'passed', JSON.stringify(result.error));
  assert.deepEqual(world.actionCalls.map(({ page, name, payload }) => [page, name, payload]), [['editor:1', 'rename', { title: 'After' }]]);
  assert.deepEqual(world.evaluatedPages, ['editor:1']);
});


test("page data uses View JSON serialization instead of the eval validator", async () => {
  const data = { error: undefined, at: new Date("2026-01-01T00:00:00Z"),
    record: new (class Record { count = 2; method() {} })(), items: [undefined, 1] };
  const page = { route: "pages/home/index", data };
  const world = logicWorld([page]);
  world.app.nav.current = async () => ({ path: page.route, instanceId: "home:1" });
  world.useLogic({ lx: {}, getCurrentPages: () => [page], __lxGetPage: () => page });
  installFakeHost(world);
  const result = await runOne(async t => {
    const expected = { at: "2026-01-01T00:00:00.000Z", record: { count: 2 }, items: [null, 1] };
    assert.deepEqual(await (await t.app.page()).data(), expected);
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
});

test("poll retries remote null reads but rejects malformed JSON results immediately", async () => {
  const world = createWorld();
  let reads = 0;
  world.usePage({ window: {}, document: { querySelector: () => ++reads < 3 ? null : { textContent: "ready" } } });
  installFakeHost(world);
  const result = await runOne(async t => {
    await expect.poll(() => t.app.view.eval(({ document }) => document.querySelector("#total").textContent),
      { interval: 5, timeout: 500 }).toBe("ready");
    let invalidReads = 0;
    await t.reject(() => t.waitFor(() => {
      invalidReads++;
      return t.app.view.eval(() => ({ invalid: undefined }));
    }), { message: "[E_NON_JSON_VALUE]" });
    assert.equal(invalidReads, 1);
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.equal(reads, 3);
});

test("waitFor retries an inner locator timeout within its own budget", async () => {
  const world = createWorld();
  const element = world.add({ testId: "ready", visible: false });
  installFakeHost(world);
  let reads = 0;
  const result = await runOne(async t => {
    await t.waitFor(async () => {
      if (++reads === 2) element.visible = true;
      await t.app.view.testId("ready").waitFor({ timeout: 30, interval: 5 });
      return true;
    }, { timeout: 500, interval: 5 });
  });
  assert.equal(result.status, "passed", JSON.stringify(result.error));
  assert.equal(reads, 2);
});
