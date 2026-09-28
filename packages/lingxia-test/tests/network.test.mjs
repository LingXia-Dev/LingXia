import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec } from "../dist/index.js";
import { reset, run } from "../dist/runner.js";
import { renderHtml } from "../dist/report.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

/**
 * Mirrors the host route table: run-wide log with a `seq` per entry,
 * per-route removal, and `drop(n)` for the log's bounds trimming its
 * oldest entries. `read` stands in for `requestsAfter` when set.
 */
function fakeNetwork() {
  const routes = new Map();
  const log = [];
  const dropped = new Map();
  let nextId = 0;
  let nextSeq = 0;
  const network = {
    routes,
    read: undefined,
    recent: undefined,
    hit(url, { method = "GET", body = null, action = "fulfill", status = 200, timestamp } = {}) {
      const route = [...routes.values()].reverse().find((r) => url.includes(r.pattern));
      if (route) {
        nextSeq += 1;
        log.push({
          seq: nextSeq, routeId: route.id, pattern: route.pattern, method, url, headers: { "x-test": "1" }, body,
          bodyTruncated: false, action, status, timestamp: timestamp ?? nextSeq,
        });
      }
    },
    drop(count) {
      for (const entry of log.splice(0, count)) dropped.set(entry.routeId, entry.seq);
    },
    async route(pattern, handler) {
      const id = ++nextId;
      const record = { id, pattern: String(pattern.url ?? pattern), handler };
      routes.set(id, record);
      return {
        id,
        pattern: record.pattern,
        async unroute() { return routes.delete(id); },
        async requests() {
          if (network.recent) return network.recent();
          return log.filter((entry) => entry.routeId === id);
        },
        async requestsAfter(after) {
          const window = {
            requests: log.filter((entry) => entry.routeId === id && entry.seq > after),
            droppedThrough: dropped.get(id) ?? 0,
          };
          return network.read ? network.read(window) : window;
        },
      };
    },
    async unrouteAll() {
      const count = routes.size;
      routes.clear();
      return count;
    },
    async requests() { return [...log]; },
  };
  return network;
}

test("routes are traced, scoped to their spec, and removed when it ends", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  let seenAfterFirst;

  spec("first", async (t) => {
    const route = await t.app.network.route(
      { url: "/v1/devices", method: "patch", times: 1 },
      { status: 501, json: { error: "unsupported_by_firmware" } },
    );
    network.hit("https://h/v1/devices/d1", { method: "PATCH", body: '{"name":"Office"}', status: 501 });
    const calls = await route.calls();
    assert.deepEqual(calls, [{
      seq: 1, time: 1, kind: "http", method: "PATCH", url: "https://h/v1/devices/d1", status: 501,
      body: { name: "Office" }, headers: { "x-test": "1" }, answeredBy: "route",
    }]);
    assert.equal((await t.app.network.calls()).length, 1);
    assert.equal(network.routes.size, 1);
    assert.equal(await route.remove(), undefined);
    assert.equal(await t.app.network.removeAll(), undefined);
  });
  spec("second", async (t) => {
    seenAfterFirst = network.routes.size;
    await t.app.network.route("/v1/clients", { abort: "failed" });
    await t.app.network.route("/v1/slow", { status: 202, delay: 300 });
    await t.app.network.route("/v1/list", { continue: true, patchJson: { items: [] } });
    await t.app.network.route("/v1/stall", { hang: true });
    network.hit("https://h/v1/clients", { action: "abort", status: null });
    network.hit("https://h/v1/list", { action: "continue", status: null, body: "not json" });
    // The run-wide host log holds both specs' hits; the fixture shows only this spec's.
    const own = await t.app.network.calls();
    assert.deepEqual(own.map((entry) => [entry.url, entry.answeredBy, entry.body]), [
      ["https://h/v1/clients", "route", null],
      ["https://h/v1/list", "real", "not json"],
    ]);
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(seenAfterFirst, 0, "the first spec's route leaked into the second");
  assert.equal(network.routes.size, 0);
  const steps = report.cases[0].steps;
  const routeStep = steps.find((step) => step.name === "network.route");
  assert.ok(routeStep, JSON.stringify(steps));
  assert.equal(routeStep.detail, "PATCH /v1/devices → 501");
  assert.ok(steps.some((step) => step.name === "network.calls"));
  const secondRoutes = report.cases[1].steps.filter((step) => step.name === "network.route");
  assert.deepEqual(secondRoutes.map((step) => step.detail), [
    "/v1/clients → abort failed",
    "/v1/slow → 202 after 300ms",
    "/v1/list → continue + patchJson",
    "/v1/stall → hang",
  ]);
});

test("route.waitForCall hands out calls in order, then times out listing recent calls", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  const seen = [];
  let failure;

  spec("waits", { forensics: false }, async (t) => {
    const route = await t.app.network.route("/v1/save", { status: 204 });
    // A call made before the wait is still the first one handed out.
    network.hit("https://h/v1/save?n=1", { method: "POST", status: 204 });
    seen.push((await route.waitForCall()).url);
    setTimeout(() => network.hit("https://h/v1/save?n=2", { method: "POST", status: 204 }), 30);
    seen.push((await route.waitForCall({ timeout: 1_000, interval: 5 })).url);
    try {
      await route.waitForCall({ timeout: 60, interval: 5 });
    } catch (error) {
      failure = error;
    }
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.deepEqual(seen, ["https://h/v1/save?n=1", "https://h/v1/save?n=2"]);
  assert.equal(failure.name, "TimeoutError");
  assert.equal(failure.code, "E_TIMEOUT");
  assert.match(failure.message, /^route \/v1\/save: no new call within \d+ms \(2 earlier calls were already returned by waitForCall\)\./);
  assert.match(failure.message, /Recent calls:\n  POST https:\/\/h\/v1\/save\?n=1 → 204 \(route\)\n  POST https:\/\/h\/v1\/save\?n=2 → 204 \(route\)/);
  assert.match(failure.message, /at .*network\.test\.mjs:\d+:\d+/);
  // One row per wait, not one per poll; identical passing waits collapse.
  const waits = report.cases[0].steps.filter((step) => step.name === "network.waitForCall");
  assert.deepEqual(waits.map((step) => [step.status, step.repeat]), [["passed", 2], ["timeout", undefined]]);
});

test("reading t.app.network never throws; a host without routing fails the call", async () => {
  const world = createWorld();
  Object.defineProperty(world.app, "network", {
    configurable: true,
    get() { throw new Error("network routing is not built into this host"); },
  });
  installFakeHost(world);
  let read;
  let rejected;

  spec("unsupported host", async (t) => {
    read = typeof t.app.network.route;
    try {
      await t.app.network.route("**", { status: 200 });
    } catch (error) {
      rejected = error.message;
    }
  });

  await run();
  assert.equal(read, "function");
  assert.match(rejected ?? "", /not built into this host/);
});

test("a host without test routing does not break t.app", async () => {
  const world = createWorld();
  installFakeHost(world);
  spec("no network", async (t) => {
    await t.app.info();
  });
  assert.equal((await run()).failed, 0);
});

/**
 * Mirrors the host's `lxapp().mock`: one scenario per run and app, rules
 * resolved with the variant first, calls kept per scenario; `reset()`
 * counts the fresh handler states.
 */
function fakeScenarios(app) {
  const state = { installed: [], current: null, calls: [], unrouted: 0, resets: 0, seq: { http: 0, function: 0 },
    droppedThrough: { http: 0, function: 0 } };
  app.mock = {};
  app.mock.reset = async () => {
    state.resets += 1;
    return { generation: state.resets, function: state.companion ?? null };
  };
  app.mock.use = async (definition, variant) => {
    if (definition.routes) throw new Error("scenario: 'routes' is the old scenario format");
    const own = variant ? definition.variants?.[variant]?.rules : [];
    if (variant && !own) throw new Error(`scenario: no variant '${variant}'`);
    const rules = [...own, ...(definition.rules ?? [])].map((rule, i) => ({
      index: i + 1,
      target: rule.http ?? `function ${rule.function}`,
      kind: rule.http ? "http" : "function",
      hits: rule.http ? 0 : null,
    }));
    const handle = {
      name: definition.name ?? null,
      variant: variant ?? null,
      get rules() { return rules.map((rule) => ({ ...rule })); },
      async calls(filter) {
        return state.calls
          .filter((call) => call.scenario === handle)
          .filter((call) => !filter
            || (filter.function !== undefined && call.function === filter.function)
            || (filter.http !== undefined && call.method === filter.http.split(" ")[0]
              && call.url?.endsWith(filter.http.split(" ")[1].replaceAll("*", "")))
            || (filter.rule !== undefined && call.rule === filter.rule))
          .map(({ scenario: _scenario, ...call }) => call);
      },
      async callsAfter(target, after) {
        const kind = target.http !== undefined
          ? "http"
          : target.function !== undefined ? "function" : rules.find((rule) => rule.index === target.rule)?.kind ?? "http";
        const calls = (await handle.calls(target)).filter((call) => call.kind === kind && call.seq > after);
        return { calls, droppedThrough: state.droppedThrough[kind] };
      },
      async unroute() {
        if (state.current !== handle) return 0;
        state.current = null;
        state.unrouted += 1;
        return rules.length;
      },
    };
    state.current = handle;
    state.installed.push({ definition, variant, handle, rules });
    return handle;
  };
  state.hit = (index, call) => {
    const rule = state.installed.at(-1).rules.find((entry) => entry.index === index);
    if (rule && rule.hits !== null) rule.hits += 1;
    const seq = ++state.seq[call.kind === "function" ? "function" : "http"];
    state.calls.push({ scenario: state.current, seq, rule: index, answeredBy: index === null ? "real" : `rule ${index}`, ...call });
  };
  return state;
}

test("t.scenario.use installs a variant for one spec, traces it, and lists its calls", async () => {
  const world = createWorld();
  const scenarios = fakeScenarios(world.app);
  installFakeHost(world);
  let seen;
  let removed = "not called";
  let replacedBefore;
  const wifi = {
    name: "Wi-Fi",
    rules: [{ http: "GET **/wifi/clients", json: [] }],
    variants: {
      a: { rules: [{ http: "GET **/wifi/main", json: { ssid: "A" } }] },
      b: { rules: [{ http: "GET **/wifi/main", json: { ssid: "B" } }, { function: "orders.submit", fault: "unknown" }] },
    },
  };

  spec("variants", async (t) => {
    const a = await t.scenario.use(wifi, { variant: "a" });
    assert.equal(a.variant, "a");
    // Switching variants mid-spec replaces the first.
    const b = await t.scenario.use(wifi, { variant: "b" });
    replacedBefore = scenarios.current?.variant;
    assert.deepEqual(b.rules.map((rule) => [rule.index, rule.target]), [
      [1, "GET **/wifi/main"], [2, "function orders.submit"], [3, "GET **/wifi/clients"],
    ]);
    scenarios.hit(2, { kind: "function", function: "orders.submit", args: { id: 7 }, time: 1, answeredBy: "rule 2 (Wi-Fi:b)", outcome: "fault" });
    seen = await b.calls({ function: "orders.submit" });
    removed = await b.remove();
  });
  spec("after", async () => {});

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(scenarios.installed.length, 2);
  assert.equal(replacedBefore, "b");
  assert.deepEqual(seen, [{
    seq: 1, time: 1, kind: "function", function: "orders.submit", body: { id: 7 }, outcome: "fault", answeredBy: "rule", rule: 2,
  }]);
  assert.equal(removed, undefined);
  assert.equal(scenarios.current, null, "the scenario lasts one spec");
  const steps = report.cases[0].steps.filter((step) => step.name === "mock.use");
  assert.deepEqual(steps.map((step) => step.detail), ["Wi-Fi:a (2 rules)", "Wi-Fi:b (3 rules)"]);
  assert.ok(report.cases[0].steps.some((step) => step.name === "scenario.calls"));
  assert.ok(report.cases[0].steps.some((step) => step.name === "scenario.remove"));
});

test("scenario calls name what answered; waitForCall waits per target", async () => {
  const world = createWorld();
  const scenarios = fakeScenarios(world.app);
  installFakeHost(world);
  const got = {};
  let failure;

  spec("targets", { forensics: false }, async (t) => {
    const scenario = await t.scenario.use({
      name: "Orders",
      rules: [{ http: "POST **/orders", status: 201, json: {} }, { function: "orders.status", result: "ok" }],
    });
    scenarios.hit(null, { kind: "http", method: "GET", url: "https://h/orders/1", status: 200, time: 1, answeredBy: "real",
      noMatch: "no rule matched GET https://h/orders/1" });
    scenarios.hit(null, { kind: "http", method: "GET", url: "https://h/other", status: 200, time: 2, answeredBy: "route **/other" });
    scenarios.hit(null, { kind: "function", function: "orders.cancel", args: [], time: 3, answeredBy: "companion default", outcome: "default" });
    setTimeout(() => scenarios.hit(1, { kind: "http", method: "POST", url: "https://h/orders", body: { qty: 1 }, status: 201, time: 4,
      answeredBy: "rule 1 (Orders)" }), 20);
    got.post = await scenario.waitForCall({ http: "POST **/orders" }, { timeout: 1_000, interval: 5 });
    got.all = await scenario.calls();
    try {
      await scenario.waitForCall({ function: "orders.status" }, { timeout: 40, interval: 5 });
    } catch (error) {
      failure = error;
    }
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.deepEqual(got.post, { seq: 3, time: 4, kind: "http", method: "POST", url: "https://h/orders", status: 201, body: { qty: 1 },
    answeredBy: "rule", rule: 1 });
  assert.deepEqual(got.all.map((call) => call.answeredBy), ["real", "route", "companion", "rule"]);
  assert.equal(got.all[0].noMatch, "no rule matched GET https://h/orders/1");
  assert.equal(failure.code, "E_TIMEOUT");
  assert.match(failure.message, /^scenario Orders: function orders\.status: no new call within \d+ms\./);
  assert.match(failure.message, /Recent calls:\n  GET https:\/\/h\/orders\/1 → 200 \(real\); no rule matched/);
  assert.match(failure.message, /function orders\.cancel → default \(companion\)/);
  assert.match(failure.message, /POST https:\/\/h\/orders → 201 \(rule 1\)/);
});

test("each spec starts with fresh mock handler state; a companion that cannot is named once", async () => {
  const world = createWorld();
  const scenarios = fakeScenarios(world.app);
  scenarios.companion = { reset: false, reason: "its handlers are rebuilt, not reset" };
  const { events } = installFakeHost(world);
  const seen = [];
  spec("one", async () => { seen.push(scenarios.resets); });
  spec("two", async () => { seen.push(scenarios.resets); });
  spec("three", async (t) => {
    seen.push(scenarios.resets);
    const scenario = await t.scenario.use({ rules: [{ http: "GET **/a", json: {} }] });
    scenarios.hit(null, { kind: "http", method: "GET", url: "https://h/b", status: 200, time: 1, answeredBy: "mock (GET **/b)" });
    scenarios.hit(null, { kind: "function", function: "orders.list", time: 2, answeredBy: "function mock", outcome: "default" });
    const calls = await scenario.calls();
    seen.push(calls.map((call) => call.answeredBy));
  });
  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  // Reset before each spec's hooks and body.
  assert.deepEqual(seen, [1, 2, 3, ["mock", "mock"]]);
  const notes = events.filter((event) => event.type === "diagnostic" && event.phase === "mock");
  assert.equal(notes.length, 1, JSON.stringify(events));
  assert.match(notes[0].message, /not reset per spec: its handlers are rebuilt, not reset/);
});

test("t.scenario.use rejections reach the spec as they are", async () => {
  const world = createWorld();
  fakeScenarios(world.app);
  installFakeHost(world);
  let message;
  spec("old format", async (t) => {
    try {
      await t.scenario.use({ routes: [] });
    } catch (error) {
      message = error.message;
    }
  });
  await run();
  assert.match(message ?? "", /'routes' is the old scenario format/);
});

test("a failed spec reports its scenario, per-rule hits and who answered each call", async () => {
  const world = createWorld();
  const scenarios = fakeScenarios(world.app);
  installFakeHost(world);
  globalThis.__LINGXIA_AUTOMATION_HOST__.networkLog = () => [
    { time: 1000, kind: "fetch", method: "GET", url: "https://h/wifi/main", status: 200, durationMs: 4, source: "route",
      route: { pattern: "**/wifi/main", action: "fulfill", rule: 1, scenario: "Wi-Fi:b" }, answeredBy: "rule 1 (Wi-Fi:b)" },
    { time: 1030, kind: "fetch", method: "PATCH", url: "https://h/devices/1", status: 204, durationMs: 9, source: "network",
      answeredBy: "real", noMatch: "no rule matched PATCH https://h/devices/1 (1 rule for this target: rule 3 match.json.name: missing)" },
  ];

  spec("shows the wrong network", async (t) => {
    await t.scenario.use({
      name: "Wi-Fi",
      rules: [{ http: "PATCH **/devices/*", match: { json: { name: "Office" } }, status: 409 }],
      variants: { b: { rules: [{ http: "GET **/wifi/main", json: { ssid: "B" } }, { function: "orders.submit", fault: "unknown" }] } },
    }, { variant: "b" });
    scenarios.hit(1, { kind: "http", time: 1000 });
    scenarios.hit(2, { kind: "function", function: "orders.submit", time: 1010, rule: 2, answeredBy: "rule 2 (Wi-Fi:b)", outcome: "fault" });
    throw new Error("expected the B network");
  });

  const report = await run();
  assert.equal(report.failed, 1);
  const error = report.cases[0].error;
  assert.equal(error.scenario.label, "Wi-Fi:b");
  assert.deepEqual(error.scenario.rules.map((rule) => [rule.index, rule.hits]), [[1, 1], [2, 1], [3, 0]]);
  assert.deepEqual(error.network.map((call) => call.answeredBy), ["rule 1 (Wi-Fi:b)", "rule 2 (Wi-Fi:b)", "real"]);
  assert.equal(error.network[1].kind, "function");
  assert.equal(error.network[1].function, "orders.submit");
  assert.equal(report.failures[0].scenario.label, "Wi-Fi:b");
  assert.equal(scenarios.unrouted, 1, "the scenario is still removed after the evidence is taken");
  const html = renderHtml(report);
  assert.match(html, /Scenario Wi-Fi:b &middot; 3 rules/);
  assert.match(html, /answered 0×/);
  assert.match(html, /rule 2 \(Wi-Fi:b\)/);
  assert.match(html, /no rule matched PATCH/);
});

test("a failed spec reports the app's last Logic network calls, secrets masked", async () => {
  const world = createWorld();
  installFakeHost(world, { control: { secretArgs: JSON.stringify(["token"]) }, args: { token: "tok-12345" } });
  const asked = [];
  globalThis.__LINGXIA_AUTOMATION_HOST__.networkLog = (since, limit) => {
    asked.push([typeof since, limit]);
    return [
      { time: 1000, kind: "fetch", method: "GET", url: "https://h/v1/me?key=tok-12345", status: 200, durationMs: 12, source: "network" },
      { time: 1040, kind: "fetch", method: "PATCH", url: "https://h/v1/devices/d1", status: 501, durationMs: 3, source: "route", route: { pattern: "**/devices/*", action: "fulfill" } },
      { time: 1100, kind: "sse", method: "GET", url: "https://h/v1/events", status: null, error: "sse request failed", durationMs: 1, source: "network" },
    ];
  };

  spec("passes", async () => {});
  spec("rename fails", async () => {
    throw new Error("rename-error never showed");
  });

  const report = await run();
  assert.equal(report.failed, 1);
  assert.deepEqual(asked, [["number", 20]], "only the failed spec reads the log");
  const failure = report.failures[0];
  assert.equal(failure.network.length, 3);
  assert.equal(failure.network[0].url, "https://h/v1/me?key=***");
  assert.equal(report.cases[1].error.network[1].source, "route");
  assert.equal(report.cases[0].error, undefined);
  const html = renderHtml(report);
  assert.match(html, /Logic network &middot; last 3 calls/);
  assert.match(html, /https:\/\/h\/v1\/me\?key=\*\*\*/);
  assert.doesNotMatch(html, /tok-12345/);
  assert.match(html, /\+40ms/);
  assert.match(html, /sse request failed/);
});

test("--record-network attaches each spec's scenario and masks secrets", async () => {
  const world = createWorld();
  const { attachments } = installFakeHost(world, {
    control: { recordNetwork: "1", secretArgs: JSON.stringify(["token"]) },
    args: { token: "tok-12345" },
  });
  const calls = [];
  globalThis.__LINGXIA_AUTOMATION_HOST__.networkRecord = (command, name) => {
    calls.push([command, name]);
    return command === "stop"
      ? { name, rules: [{ http: "GET https://h/v1/me?key=tok-12345", status: 200, json: { ok: true } }] }
      : null;
  };

  spec("records", { id: "rec-1" }, async () => {});
  const report = await run();
  assert.equal(report.failed, 0);
  assert.deepEqual(calls, [["start", undefined], ["stop", "records"]]);
  const artifact = attachments.get("attachments/rec-1/attempt-0/network.scenario.json");
  assert.ok(artifact, [...attachments.keys()].join(", "));
  const scenario = JSON.parse(Buffer.from(artifact.base64, "base64").toString("utf8"));
  assert.equal(scenario.rules[0].http, "GET https://h/v1/me?key=***");
  assert.ok(report.cases[0].attachments.some((entry) => entry.name === "network.scenario.json"));
});

test("--record-network on a host without recording fails the run", async () => {
  const world = createWorld();
  installFakeHost(world, { control: { recordNetwork: "1" } });
  spec("records nothing", async () => {});
  await assert.rejects(() => run(), /--record-network needs a host that records network traffic/);
});


/** Run one spec and hand back what it caught, with the report. */
async function catching(body) {
  const caught = {};
  spec("waits", { forensics: false }, async (t) => { await body(t, caught); });
  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  return caught;
}

async function settle(promise) {
  const started = Date.now();
  try {
    return { value: await promise, ms: Date.now() - started };
  } catch (error) {
    return { error, ms: Date.now() - started };
  }
}

test("waitForCall times out on time while a read of calls never returns", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  const caught = await catching(async (t, caught) => {
    const route = await t.app.network.route("/v1/save", { status: 204 });
    network.read = () => new Promise(() => {});
    caught.result = await settle(route.waitForCall({ timeout: 40, interval: 5 }));
  });
  const { error, ms } = caught.result;
  assert.equal(error?.code, "E_TIMEOUT", String(error));
  assert.ok(ms < 40 + 60, `took ${ms}ms`);
  assert.match(error.message, /^route \/v1\/save: no new call within \d+ms\./);
  assert.match(error.message, /last read of its calls had not returned/);
});

test("a read that returns after the deadline does not turn the wait into a success", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  const caught = await catching(async (t, caught) => {
    const route = await t.app.network.route("/v1/save", { status: 204 });
    network.hit("https://h/v1/save?n=1", { method: "POST", status: 204 });
    network.read = (window) => new Promise((resolve) => setTimeout(() => resolve(window), 60));
    caught.late = await settle(route.waitForCall({ timeout: 20, interval: 5 }));
    network.read = undefined;
    // Not handed out, so the next wait still returns it.
    caught.next = await route.waitForCall({ timeout: 200, interval: 5 });
  });
  assert.equal(caught.late.error?.code, "E_TIMEOUT", String(caught.late.error));
  assert.ok(caught.late.ms < 20 + 60, `took ${caught.late.ms}ms`);
  assert.equal(caught.next.url, "https://h/v1/save?n=1");
});

test("reading recent calls for a timeout gets a bounded slice", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  const caught = await catching(async (t, caught) => {
    const route = await t.app.network.route("/v1/save", { status: 204 });
    network.recent = () => new Promise(() => {});
    caught.result = await settle(route.waitForCall({ timeout: 30, interval: 5 }));
  });
  const { error, ms } = caught.result;
  assert.equal(error?.code, "E_TIMEOUT", String(error));
  // 30ms of waiting plus at most the 100ms evidence slice.
  assert.ok(ms < 30 + 100 + 60, `took ${ms}ms`);
  assert.match(error.message, /Recent calls were not read: the read took longer than 100ms\./);
});

test("waitForCall resumes by seq: calls in one millisecond stay distinct", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  const caught = await catching(async (t, caught) => {
    const route = await t.app.network.route("/v1/burst", { status: 204 });
    for (const n of [1, 2, 3]) network.hit(`https://h/v1/burst?n=${n}`, { method: "POST", status: 204, timestamp: 1000 });
    caught.urls = [];
    for (let i = 0; i < 3; i += 1) caught.urls.push((await route.waitForCall({ timeout: 200, interval: 5 })).url);
  });
  assert.deepEqual(caught.urls, ["https://h/v1/burst?n=1", "https://h/v1/burst?n=2", "https://h/v1/burst?n=3"]);
});

test("a trim of calls already handed out loses nothing; one past the cursor is reported", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  const caught = await catching(async (t, caught) => {
    const route = await t.app.network.route("/v1/save", { status: 204 });
    network.hit("https://h/v1/save?n=1", { method: "POST", status: 204 });
    caught.first = (await route.waitForCall({ timeout: 200, interval: 5 })).url;
    // The log drops the call already handed out: the cursor is past it.
    network.hit("https://h/v1/save?n=2", { method: "POST", status: 204 });
    network.drop(1);
    caught.second = (await route.waitForCall({ timeout: 200, interval: 5 })).url;
    // Then it drops two calls nobody read yet, an index cursor would skip them.
    network.hit("https://h/v1/save?n=3", { method: "POST", status: 204 });
    network.hit("https://h/v1/save?n=4", { method: "POST", status: 204 });
    network.hit("https://h/v1/save?n=5", { method: "POST", status: 204 });
    network.drop(3);
    caught.lost = await settle(route.waitForCall({ timeout: 200, interval: 5 }));
    caught.after = (await route.waitForCall({ timeout: 200, interval: 5 })).url;
  });
  assert.equal(caught.first, "https://h/v1/save?n=1");
  assert.equal(caught.second, "https://h/v1/save?n=2");
  const { error } = caught.lost;
  assert.ok(error, "the wait must not skip the dropped calls");
  assert.notEqual(error.code, "E_TIMEOUT");
  assert.match(error.message, /^route \/v1\/save: calls after the last one waitForCall returned \(seq 2\) were dropped from a bounded call log before they were read \(seq 3–4, 2 entries of that log\)/);
  assert.equal(caught.after, "https://h/v1/save?n=5");
});

test("another route's trimmed calls do not fail this route's wait", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  const caught = await catching(async (t, caught) => {
    const noisy = await t.app.network.route("/v1/noisy", { status: 204 });
    const quiet = await t.app.network.route("/v1/quiet", { status: 204 });
    for (let i = 0; i < 5; i += 1) network.hit(`https://h/v1/noisy?n=${i}`);
    network.hit("https://h/v1/quiet");
    network.drop(5);
    caught.quiet = (await quiet.waitForCall({ timeout: 200, interval: 5 })).url;
    caught.noisy = await settle(noisy.waitForCall({ timeout: 200, interval: 5 }));
  });
  assert.equal(caught.quiet, "https://h/v1/quiet");
  assert.match(caught.noisy.error?.message ?? "", /were dropped from a bounded call log before they were read \(seq 1–5/);
});

test("scenario waits report calls their log dropped per kind", async () => {
  const world = createWorld();
  const scenarios = fakeScenarios(world.app);
  installFakeHost(world);
  const caught = await catching(async (t, caught) => {
    const scenario = await t.scenario.use({
      name: "Orders",
      rules: [{ http: "POST **/orders", status: 201, json: {} }, { function: "orders.status", result: "ok" }],
    });
    scenarios.hit(1, { kind: "http", method: "POST", url: "https://h/orders", status: 201, time: 1 });
    scenarios.hit(2, { kind: "function", function: "orders.status", time: 1, outcome: "result" });
    scenarios.hit(2, { kind: "function", function: "orders.status", time: 1, outcome: "result" });
    // The companion's log dropped its first Function call; HTTP lost none.
    scenarios.droppedThrough.function = 1;
    caught.http = (await scenario.waitForCall({ http: "POST **/orders" }, { timeout: 200, interval: 5 })).seq;
    caught.lost = await settle(scenario.waitForCall({ rule: 2 }, { timeout: 200, interval: 5 }));
    caught.next = (await scenario.waitForCall({ rule: 2 }, { timeout: 200, interval: 5 })).seq;
  });
  assert.equal(caught.http, 1);
  assert.match(caught.lost.error?.message ?? "", /^scenario Orders: rule 2: calls after the start were dropped from a bounded call log before they were read \(seq 1, 1 entry/);
  assert.equal(caught.next, 2);
});
