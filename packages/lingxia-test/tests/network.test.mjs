import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset, run } from "../dist/index.js";
import { renderHtml } from "../dist/report.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

/** Mirrors the host route table: run-wide log, per-route removal. */
function fakeNetwork() {
  const routes = new Map();
  const log = [];
  let nextId = 0;
  const network = {
    routes,
    hit(url) {
      const route = [...routes.values()].reverse().find((r) => url.includes(r.pattern));
      if (route) {
        log.push({
          routeId: route.id, pattern: route.pattern, method: "GET", url, headers: {}, body: null,
          bodyTruncated: false, action: "fulfill", status: 200, timestamp: 0,
        });
      }
    },
    async route(pattern, handler) {
      const id = ++nextId;
      const record = { id, pattern: String(pattern.url ?? pattern), handler };
      routes.set(id, record);
      return {
        id,
        pattern: record.pattern,
        async unroute() { return routes.delete(id); },
        async requests() { return log.filter((entry) => entry.routeId === id); },
      };
    },
    async scenario(definition) {
      const handles = [];
      for (const entry of definition.routes ?? definition.http.routes) handles.push(await network.route(entry.url, entry));
      return {
        name: definition.name ?? null,
        get routes() { return [...handles]; },
        async unroute() {
          let removed = 0;
          for (const handle of handles) if (await handle.unroute()) removed += 1;
          return removed;
        },
        async requests() { return log.filter((entry) => handles.some((handle) => handle.id === entry.routeId)); },
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
    network.hit("https://h/v1/devices/d1");
    assert.equal((await route.requests()).length, 1);
    assert.equal((await t.app.network.requests()).length, 1);
    assert.equal(network.routes.size, 1);
  });
  spec("second", async (t) => {
    seenAfterFirst = network.routes.size;
    await t.app.network.route("/v1/clients", { abort: "failed" });
    await t.app.network.route("/v1/slow", { status: 202, delay: 300 });
    await t.app.network.route("/v1/list", { continue: true, patchJson: { items: [] } });
    await t.app.network.route("/v1/stall", { hang: true });
    network.hit("https://h/v1/clients");
    // The run-wide host log holds both specs' hits; the fixture shows only this spec's.
    const own = await t.app.network.requests();
    assert.deepEqual(own.map((entry) => entry.pattern), ["/v1/clients"]);
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(seenAfterFirst, 0, "the first spec's route leaked into the second");
  assert.equal(network.routes.size, 0);
  const steps = report.cases[0].steps;
  const routeStep = steps.find((step) => step.name === "network.route");
  assert.ok(routeStep, JSON.stringify(steps));
  assert.equal(routeStep.detail, "PATCH /v1/devices → 501");
  assert.ok(steps.some((step) => step.name === "network.requests"));
  const secondRoutes = report.cases[1].steps.filter((step) => step.name === "network.route");
  assert.deepEqual(secondRoutes.map((step) => step.detail), [
    "/v1/clients → abort failed",
    "/v1/slow → 202 after 300ms",
    "/v1/list → continue + patchJson",
    "/v1/stall → hang",
  ]);
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

test("a scenario installs its routes for one spec and reports their requests", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  let installed;
  let names;
  let handleRequests;
  let specRequests;
  let seenAfter;

  spec("scenario", async (t) => {
    const scenario = await t.app.network.scenario({
      name: "outage",
      routes: [
        { url: "/v1/status", sequence: [{ status: 503 }, { json: { up: true } }] },
        { url: "/v1/events", sse: [{ data: "hello" }, { drop: true }] },
      ],
    });
    installed = network.routes.size;
    names = scenario.routes.map((route) => route.pattern);
    network.hit("https://h/v1/status");
    handleRequests = (await scenario.requests()).length;
    specRequests = (await t.app.network.requests()).length;
    assert.equal(scenario.name, "outage");
  });
  spec("after", async () => {
    seenAfter = network.routes.size;
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(installed, 2);
  assert.deepEqual(names, ["/v1/status", "/v1/events"]);
  assert.equal(handleRequests, 1);
  assert.equal(specRequests, 1);
  assert.equal(seenAfter, 0, "a scenario route leaked into the next spec");
  const step = report.cases[0].steps.find((entry) => entry.name === "network.scenario");
  assert.equal(step?.detail, "outage (2 routes)");
});

test("a sectioned scenario file installs its http routes", async () => {
  const world = createWorld();
  const network = fakeNetwork();
  world.app.network = network;
  installFakeHost(world);
  let name;
  let installed;

  spec("sectioned", async (t) => {
    const scenario = await t.app.network.scenario({
      $schema: "../../node_modules/@lingxia/test/schemas/scenario.schema.json",
      name: "gateway offline",
      description: "the status card shows offline",
      http: { routes: [{ url: "/v1/status", status: 503 }, { url: "/v1/devices", json: [] }] },
    });
    name = scenario.name;
    installed = network.routes.size;
  });

  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases));
  assert.equal(installed, 2);
  assert.equal(name, "gateway offline");
  assert.equal(network.routes.size, 0, "the routes last one spec");
  const step = report.cases[0].steps.find((entry) => entry.name === "network.scenario");
  assert.equal(step?.detail, "gateway offline (2 routes)");
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
      ? { name, routes: [{ url: "https://h/v1/me?key=tok-12345", method: "GET", status: 200, json: { ok: true } }] }
      : null;
  };

  spec("records", { id: "rec-1" }, async () => {});
  const report = await run();
  assert.equal(report.failed, 0);
  assert.deepEqual(calls, [["start", undefined], ["stop", "records"]]);
  const artifact = attachments.get("attachments/rec-1/attempt-0/network.scenario.json");
  assert.ok(artifact, [...attachments.keys()].join(", "));
  const scenario = JSON.parse(Buffer.from(artifact.base64, "base64").toString("utf8"));
  assert.equal(scenario.routes[0].url, "https://h/v1/me?key=***");
  assert.ok(report.cases[0].attachments.some((entry) => entry.name === "network.scenario.json"));
});

test("--record-network on a host without recording says so once per spec", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world, { control: { recordNetwork: "1" } });
  spec("records nothing", async () => {});
  const report = await run();
  assert.equal(report.failed, 0);
  assert.ok(events.some((event) => event.type === "diagnostic" && event.phase === "record-network"));
});
