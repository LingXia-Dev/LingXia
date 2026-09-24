import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, expect, reset, run } from "../dist/index.js";
import { validateSchema } from "../dist/schema.js";
import { OpenApiIndex, compileTemplate, pathOf, templateMatches } from "../dist/openapi.js";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

const check = (value, schema, dialect = "3.1", root = {}) =>
  validateSchema(value, schema, "#/s", { root, dialect });

test("3.0 nullable widens the type; 3.1 spells null in a type array", () => {
  assert.deepEqual(check(null, { type: "string", nullable: true }, "3.0"), []);
  assert.match(check(null, { type: "string" }, "3.0")[0].message, /expected string, got null/);
  // `nullable` means nothing in 3.1.
  assert.equal(check(null, { type: "string", nullable: true }, "3.1").length, 1);
  assert.deepEqual(check(null, { type: ["string", "null"] }, "3.1"), []);
  assert.deepEqual(check("a", { type: ["string", "null"] }, "3.1"), []);
  assert.match(check(1, { type: ["string", "null"] }, "3.1")[0].message, /expected string or null, got integer 1/);
  // A nullable enum admits null without listing it.
  assert.deepEqual(check(null, { type: "string", enum: ["a"], nullable: true }, "3.0"), []);
  assert.match(check("b", { type: "string", enum: ["a"] })[0].message, /expected one of \["a"\]/);
});

test("$ref resolves locally, siblings apply only in 3.1", () => {
  const root = {
    components: {
      schemas: {
        Device: {
          type: "object",
          required: ["id", "name", "secret"],
          properties: {
            id: { type: "string" },
            name: { type: "string", minLength: 1 },
            secret: { type: "string", writeOnly: true },
            tags: { type: "array", items: { $ref: "#/components/schemas/Tag" }, uniqueItems: true },
          },
          additionalProperties: false,
        },
        Tag: { type: "string", pattern: "^[a-z]+$" },
        Node: { type: "object", properties: { next: { $ref: "#/components/schemas/Node" } } },
      },
    },
  };
  // `secret` is writeOnly: never required in a response.
  assert.deepEqual(check({ id: "d1", name: "Office" }, { $ref: "#/components/schemas/Device" }, "3.1", root), []);
  const issues = check({ id: 7, name: "", tags: ["ok", "Bad", "ok"], extra: 1 }, { $ref: "#/components/schemas/Device" }, "3.1", root);
  const byPath = Object.fromEntries(issues.map((issue) => [issue.path, issue]));
  assert.match(byPath["/id"].message, /expected string, got integer 7/);
  assert.equal(byPath["/id"].schema, "#/components/schemas/Device/properties/id");
  assert.match(byPath["/name"].message, /at least 1 characters/);
  assert.match(byPath["/tags"].message, /unique items/);
  assert.match(byPath["/tags/1"].message, /match \/\^\[a-z\]\+\$\//);
  assert.equal(byPath["/tags/1"].schema, "#/components/schemas/Tag");
  assert.match(byPath["/extra"].message, /unexpected property "extra"/);
  // Recursive schemas terminate on finite values.
  assert.deepEqual(check({ next: { next: {} } }, { $ref: "#/components/schemas/Node" }, "3.1", root), []);
  assert.match(check({}, { $ref: "#/components/schemas/Missing" }, "3.1", root)[0].message, /does not resolve/);
  // 3.0 ignores the siblings of a $ref; 3.1 applies them.
  const withSibling = { $ref: "#/components/schemas/Tag", maxLength: 2 };
  assert.deepEqual(check("abc", withSibling, "3.0", root), []);
  assert.match(check("abc", withSibling, "3.1", root)[0].message, /at most 2 characters/);
});

test("deep recursive data is not a cycle; a $ref loop that never reaches data is", () => {
  const root = {
    components: {
      schemas: {
        Node: {
          type: "object",
          required: ["id"],
          properties: { id: { type: "integer" }, child: { $ref: "#/components/schemas/Node" } },
        },
        A: { $ref: "#/components/schemas/B" },
        B: { allOf: [{ $ref: "#/components/schemas/A" }] },
        Leaf: { type: "object", properties: { v: { type: "integer" } } },
        Choice: {
          anyOf: [
            { $ref: "#/components/schemas/Wrap" },
            { $ref: "#/components/schemas/Wrap" },
            { $ref: "#/components/schemas/Wrap" },
          ],
        },
        Wrap: { anyOf: [{ $ref: "#/components/schemas/Leaf" }, { $ref: "#/components/schemas/Leaf" }] },
        Tree: {
          type: "object",
          properties: { v: { type: "integer" }, next: { $ref: "#/components/schemas/Choice2" } },
        },
        Choice2: {
          anyOf: [{ $ref: "#/components/schemas/Tree" }, { $ref: "#/components/schemas/Tree" }, { $ref: "#/components/schemas/Tree" }],
        },
      },
    },
  };
  // 200 levels deep: far past any hop budget, and still valid.
  let deep = { id: 200 };
  for (let id = 199; id >= 0; id -= 1) deep = { id, child: deep };
  assert.deepEqual(check(deep, { $ref: "#/components/schemas/Node" }, "3.1", root), []);
  let bad = { id: "x" };
  for (let id = 49; id >= 0; id -= 1) bad = { id, child: bad };
  const issues = check(bad, { $ref: "#/components/schemas/Node" }, "3.1", root);
  assert.equal(issues.length, 1);
  assert.equal(issues[0].path, "/child".repeat(50) + "/id");
  // A loop of $refs for one value is reported, not followed forever.
  assert.match(check({}, { $ref: "#/components/schemas/A" }, "3.1", root)[0].message, /circular/);
  // Identical alternatives nested 25 deep: 3^25 paths unless memoized.
  let chain = { v: 0 };
  for (let v = 1; v < 25; v += 1) chain = { v, next: chain };
  const started = Date.now();
  assert.deepEqual(check(chain, { $ref: "#/components/schemas/Tree" }, "3.1", root), []);
  assert.deepEqual(check({ v: 1 }, { $ref: "#/components/schemas/Choice" }, "3.1", root), []);
  assert.ok(Date.now() - started < 2000, "identical alternatives must not explode");
});

test("oneOf needs exactly one match; a discriminator picks the branch", () => {
  const root = {
    components: {
      schemas: {
        Cat: { type: "object", required: ["kind", "meows"], properties: { kind: { const: "Cat" }, meows: { type: "boolean" } } },
        Dog: { type: "object", required: ["kind", "barks"], properties: { kind: { const: "Dog" }, barks: { type: "boolean" } } },
      },
    },
  };
  const pet = { oneOf: [{ $ref: "#/components/schemas/Cat" }, { $ref: "#/components/schemas/Dog" }] };
  assert.deepEqual(check({ kind: "Cat", meows: true }, pet, "3.1", root), []);
  const none = check({ kind: "Cat" }, pet, "3.1", root);
  assert.match(none[0].message, /matches none of the 2 oneOf alternatives; closest is #0/);
  assert.match(none[1].message, /missing required property "meows"/);
  const loose = { oneOf: [{ type: "number" }, { type: "integer" }] };
  assert.match(check(3, loose)[0].message, /matches 2 oneOf alternatives \(#0, #1\)/);
  assert.deepEqual(check(3.5, loose), []);

  const discriminated = { ...pet, discriminator: { propertyName: "kind" } };
  const wrong = check({ kind: "Dog", barks: "loud" }, discriminated, "3.1", root);
  assert.deepEqual(wrong.map((issue) => issue.path), ["/barks"]);
  assert.match(check({ kind: "Fish" }, discriminated, "3.1", root)[0].message, /names no oneOf alternative \(known: Cat, Dog\)/);
  const mapped = { ...pet, discriminator: { propertyName: "kind", mapping: { cat: "#/components/schemas/Cat" } } };
  assert.match(check({ kind: "cat" }, mapped, "3.1", root)[0].message, /missing required property "meows"/);
});

test("allOf, anyOf, not and the numeric bounds of both dialects", () => {
  assert.deepEqual(check({ a: 1, b: "x" }, { allOf: [{ required: ["a"] }, { required: ["b"] }] }), []);
  assert.match(check({ a: 1 }, { allOf: [{ required: ["a"] }, { required: ["b"] }] })[0].message, /"b"/);
  assert.deepEqual(check("x", { anyOf: [{ type: "integer" }, { type: "string" }] }), []);
  assert.match(check(true, { anyOf: [{ type: "integer" }, { type: "string" }] })[0].message, /none of the 2 anyOf/);
  assert.match(check("x", { not: { type: "string" } })[0].message, /under `not`/);
  assert.match(check(0, { type: "integer", minimum: 0, exclusiveMinimum: true }, "3.0")[0].message, /expected > 0/);
  assert.match(check(5, { exclusiveMaximum: 5 }, "3.1")[0].message, /expected < 5/);
  assert.match(check(1.5, { type: "integer" })[0].message, /expected integer, got number 1.5/);
  assert.match(check(0.3, { multipleOf: 0.2 })[0].message, /multiple of 0.2/);
  assert.deepEqual(check(0.6, { multipleOf: 0.2 }), []);
  assert.deepEqual(check({ n: 1 }, { additionalProperties: { type: "integer" } }), []);
  assert.match(check({ n: "1" }, { additionalProperties: { type: "integer" } })[0].path, /^\/n$/);
  assert.deepEqual(check([1, "a"], { prefixItems: [{ type: "integer" }, { type: "string" }], items: false }), []);
  assert.match(check([1, "a", 2], { prefixItems: [{ type: "integer" }, { type: "string" }], items: false })[0].message, /no value is allowed/);
  // `format` is an annotation, never asserted.
  assert.deepEqual(check("not-an-email", { type: "string", format: "email" }), []);
});

test("path templates match one segment per parameter, after the server base path", () => {
  assert.equal(templateMatches(compileTemplate("/devices/{id}"), "/devices/d1"), true);
  assert.equal(templateMatches(compileTemplate("/devices/{id}"), "/devices/"), false);
  assert.equal(templateMatches(compileTemplate("/devices/{id}"), "/devices/d1/ports"), false);
  assert.equal(templateMatches(compileTemplate("/files/a b"), "/files/a%20b"), true);
  assert.equal(pathOf("https://api.test:8443/v1/x?y=1#z"), "/v1/x");
  assert.equal(pathOf("/v1/x"), "/v1/x");

  const index = new OpenApiIndex([{
    name: "api.yaml",
    doc: {
      openapi: "3.1.0",
      servers: [{ url: "https://{host}/{base}", variables: { host: { default: "api.test" }, base: { default: "v1" } } }],
      paths: {
        "/devices/{id}": { get: { responses: {} } },
        "/devices/me": { get: { responses: {} } },
        "/devices": { get: { responses: {} }, post: { responses: {} } },
      },
    },
  }]);
  assert.equal(index.match("GET", "https://staging.test/v1/devices/d1").template, "/devices/{id}");
  // A concrete path wins over the template that also fits.
  assert.equal(index.match("GET", "https://api.test/v1/devices/me").template, "/devices/me");
  assert.equal(index.match("post", "https://api.test/v1/devices/").template, "/devices");
  assert.equal(index.match("GET", "https://api.test/devices/d1"), undefined);
  assert.equal(index.match("DELETE", "https://api.test/v1/devices/d1"), undefined);
});

const API = {
  openapi: "3.0.3",
  info: { title: "Devices", version: "1" },
  servers: [{ url: "https://api.example.com/v1" }],
  paths: {
    "/devices": {
      get: {
        operationId: "listDevices",
        responses: {
          200: { description: "ok", content: { "application/json": { schema: { $ref: "#/components/schemas/DeviceList" } } } },
          "4XX": { $ref: "#/components/responses/Problem" },
        },
      },
    },
    "/devices/{id}": {
      patch: {
        responses: {
          200: { description: "ok", content: { "application/json": { schema: { $ref: "#/components/schemas/Device" } } } },
          204: { description: "no content" },
        },
      },
    },
  },
  components: {
    schemas: {
      Device: {
        type: "object",
        required: ["id", "name"],
        properties: { id: { type: "string" }, name: { type: "string" }, room: { type: "string", nullable: true } },
      },
      DeviceList: {
        type: "object",
        required: ["items", "total"],
        properties: { items: { type: "array", items: { $ref: "#/components/schemas/Device" } }, total: { type: "integer" } },
      },
      Problem: { type: "object", required: ["error"], properties: { error: { type: "string" } } },
    },
    responses: {
      Problem: { description: "problem", content: { "application/problem+json": { schema: { $ref: "#/components/schemas/Problem" } } } },
    },
  },
};

const response = (overrides) => ({
  method: "GET",
  url: "https://api.example.com/v1/devices",
  status: 200,
  contentType: "application/json",
  body: JSON.stringify({ items: [], total: 0 }),
  bodyTruncated: false,
  ...overrides,
});

test("a response is checked by operation, status and JSON media type", () => {
  const index = new OpenApiIndex([{ name: "devices.yaml", doc: API }]);
  assert.equal(index.check(response()).kind, "valid");
  const invalid = index.check(response({ body: JSON.stringify({ items: [{ id: 1 }], total: "0" }) }));
  assert.equal(invalid.kind, "invalid");
  assert.equal(invalid.operation, "GET /devices");
  assert.equal(invalid.schema, "#/components/schemas/DeviceList");
  assert.deepEqual(invalid.issues.map((issue) => issue.path).sort(), ["/items/0", "/items/0/id", "/total"]);
  // `4XX` via a $ref'd response object, matched on a +json type.
  const problem = index.check(response({ status: 404, contentType: "application/problem+json", body: "{}" }));
  assert.equal(problem.kind, "invalid");
  assert.match(problem.issues[0].message, /"error"/);
  assert.equal(index.check(response({ status: 500 })).kind, "undocumented");
  assert.equal(index.check(response({ url: "https://cdn.example.com/logo.png" })).kind, "unmatched");
  assert.deepEqual(index.check(response({ contentType: "text/html", body: null })), { kind: "skipped", operation: "GET /devices", status: 200, reason: "not_json" });
  assert.equal(index.check(response({ bodyTruncated: true })).reason, "truncated");
  assert.equal(index.check(response({ method: "PATCH", url: "https://api.example.com/v1/devices/d1", status: 204, contentType: null, body: null })).reason, "no_schema");
  assert.equal(index.check(response({ body: "{oops" })).issues[0].message.startsWith("the body is not JSON"), true);
  assert.throws(() => new OpenApiIndex([{ name: "old.json", doc: { swagger: "2.0" } }]), /OpenAPI 3.0 or 3.1/);
});

test("toMatchSchema fails clearly without --openapi, and checks with it", async () => {
  assert.throws(() => expect({}).toMatchSchema("Device"), /toMatchSchema needs an OpenAPI document/);
  assert.throws(() => expect({}).not.toMatchSchema("Device"), /toMatchSchema needs an OpenAPI document/);

  const world = createWorld();
  installFakeHost(world, { control: { openapi: JSON.stringify([{ name: "devices.yaml", doc: API }]) } });
  const seen = [];
  spec("schema assertions", async () => {
    expect({ id: "d1", name: "Office", room: null }).toMatchSchema("Device");
    expect({ id: "d1" }).not.toMatchSchema("Device");
    expect({ error: "x" }).toMatchSchema({ ref: "#/components/schemas/Problem", document: "devices.yaml" });
    try {
      expect({ id: 1, name: "x" }).toMatchSchema("#/components/schemas/Device");
    } catch (error) {
      seen.push(error.message);
    }
    try {
      expect({}).toMatchSchema("Nope");
    } catch (error) {
      seen.push(error.message);
    }
  });
  const report = await run();
  assert.equal(report.failed, 0, JSON.stringify(report.cases[0].error));
  assert.match(seen[0], /at \/id: expected string, got integer 1/);
  assert.match(seen[1], /#\/components\/schemas\/Nope is not in devices\.yaml \(component schemas: Device, DeviceList, Problem\)/);
  const matchers = report.cases[0].assertions.map((entry) => entry.matcher);
  assert.ok(matchers.includes("not.toMatchSchema"), JSON.stringify(matchers));
});

test("t.openapi names the run's documents; without them a spec can skip itself", async () => {
  const world = createWorld();
  installFakeHost(world);
  const contractOnly = async (t) => {
    if (!t.openapi) t.skip("needs its OpenAPI document: run with --openapi");
    expect({ id: "d1", name: "Office", room: null }).toMatchSchema("Device");
  };
  spec("contract only", contractOnly);
  let report = await run();
  assert.equal(report.cases[0].status, "skipped");
  assert.match(report.cases[0].reason, /needs its OpenAPI document/);

  reset();
  installFakeHost(createWorld(), { control: { openapi: JSON.stringify([{ name: "devices.yaml", doc: API }]) } });
  let documents;
  spec("contract only", async (t) => {
    documents = t.openapi?.documents;
    await contractOnly(t);
  });
  report = await run();
  assert.equal(report.cases[0].status, "passed", JSON.stringify(report.cases[0].error));
  assert.deepEqual(documents.map((doc) => doc.name), ["devices.yaml"]);
});

/** A host network driver that captures like the Rust one. */
function capturingNetwork(records, options = {}) {
  let enabled = 0;
  return {
    get enabled() { return enabled; },
    async route() { throw new Error("not used"); },
    async unrouteAll() { return 0; },
    async requests() { return []; },
    async captureResponses() {
      if (options.unsupported) throw new Error("captureResponses is not a function");
      enabled += 1;
    },
    async responses({ since = 0 } = {}) {
      return records.filter((record) => record.seq > since);
    },
  };
}

test("routed mismatches fail the spec; the server's are warnings", async () => {
  const world = createWorld();
  const records = [];
  const network = capturingNetwork(records);
  world.app.network = network;
  const { events } = installFakeHost(world, {
    control: { openapi: JSON.stringify([{ name: "devices.yaml", doc: API }]), openapiFiles: "devices.yaml" },
  });
  let seq = 0;
  const hit = (overrides) => records.push({ seq: ++seq, source: "network", pattern: null, timestamp: 0, ...response(overrides) });

  spec("routed list", async () => {
    hit({ source: "route", pattern: "**/v1/devices", body: JSON.stringify({ items: [{ id: "d1" }], total: 1 }) });
  });
  spec("live list", async () => {
    hit({ body: JSON.stringify({ items: [], total: "none" }) });
    hit({ url: "https://metrics.example.com/beacon", contentType: "text/plain", body: null });
  });
  spec("clean", async () => {
    hit({ source: "patch", pattern: "**/v1/devices" });
  });
  spec.fail("known stale fixture", { expected: { code: "E_OPENAPI_CONTRACT" } }, async () => {
    hit({ source: "route", pattern: "**/v1/devices", body: JSON.stringify({ items: [], total: "0" }) });
  });

  const report = await run();
  assert.ok(network.enabled >= 3);
  const [routed, live, clean, stale] = report.cases;
  assert.equal(stale.status, "xfail", JSON.stringify(stale.error));
  assert.equal(routed.status, "failed");
  assert.equal(routed.error.code, "E_OPENAPI_CONTRACT");
  assert.equal(routed.error.phase, "contract");
  assert.match(routed.error.message, /GET \/devices → 200 \(fulfilled by route \*\*\/v1\/devices\): the response does not match #\/components\/schemas\/DeviceList/);
  assert.match(routed.error.message, /at \/items\/0: missing required property "name"/);
  assert.equal(routed.contract.violations.length, 1);

  assert.equal(live.status, "passed");
  assert.equal(live.contract.warnings.length, 1);
  assert.equal(clean.status, "passed");
  assert.equal(clean.contract.checked, 1);

  const summary = report.openapi;
  assert.deepEqual(summary.documents, [{ name: "devices.yaml", version: "3.0.3", title: "Devices", operations: 2 }]);
  assert.equal(summary.capture, "ok");
  assert.equal(summary.responses, 5);
  assert.deepEqual(summary.routed, { validated: 3, failed: 2 });
  assert.deepEqual(summary.network, { validated: 1, mismatched: 1 });
  assert.deepEqual(summary.unmatched, [{ method: "GET", path: "/beacon", count: 1 }]);
  assert.equal(summary.warnings[0].case, "live-list");
  assert.equal(report.meta.run.openapi, undefined, "the documents are not repeated in meta");
  assert.equal(report.meta.run.openapiFiles, "devices.yaml");
  const warning = events.find((event) => event.type === "diagnostic" && event.phase === "contract");
  assert.match(warning.message, /live-list: GET \/devices → 200 from the server does not match/);
});

test("a host without response capture still runs, and says so", async () => {
  const world = createWorld();
  world.app.network = capturingNetwork([], { unsupported: true });
  const { events } = installFakeHost(world, { control: { openapi: JSON.stringify([{ name: "api.json", doc: API }]) } });
  spec("one", async () => {});
  spec("two", async () => {});
  const report = await run();
  assert.equal(report.passed, 2);
  assert.match(report.openapi.capture, /^unavailable: /);
  assert.equal(events.filter((event) => event.type === "diagnostic" && event.phase === "contract").length, 1);
});
