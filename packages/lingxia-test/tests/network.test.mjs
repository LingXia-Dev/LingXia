import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset, run } from "../dist/index.js";

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
      if (route) log.push({ routeId: route.id, pattern: route.pattern, method: "GET", url, action: "fulfill", status: 200, timestamp: 0 });
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
  assert.equal(report.cases[1].steps.find((step) => step.name === "network.route").detail, "/v1/clients → abort failed");
});

test("a host without test routing does not break t.app", async () => {
  const world = createWorld();
  installFakeHost(world);
  spec("no network", async (t) => {
    await t.app.info();
  });
  assert.equal((await run()).failed, 0);
});
