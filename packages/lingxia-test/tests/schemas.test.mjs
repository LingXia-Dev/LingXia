import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { validateSchema } from "../dist/schema.js";

const load = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), "utf8"));
const scenarioSchema = load("../schemas/scenario.schema.json");
const lxdevSchema = load("../schemas/lxdev.schema.json");

function issues(value, schema) {
  return validateSchema(value, schema, "#", { root: schema, dialect: "3.1" }).map(
    (issue) => `${issue.path}: ${issue.message}`,
  );
}

test("the scenario schema takes rules and variants", () => {
  const rules = [
    { http: "GET **/v1/status", times: 2, sequence: [{ status: 503 }, { json: { up: true } }] },
    { http: "* /devices\\/\\w+$/i", status: 404, note: "gone" },
    { http: "GET **/icon.png", bodyBase64: "iVBORw==", contentType: "image/png" },
    { http: "GET **/events", sse: [{ event: "status", data: { n: 1 }, id: "e1" }, { delayMs: 500 }, { drop: true }] },
    { http: "POST **/offline", abort: "failed" },
    { http: "PATCH **/devices/*", match: { json: { name: "Office", tags: ["/^floor-/"] } }, status: 409, json: { error: "conflict" } },
    { function: "coupons.apply", match: { args: { code: "SPRING" } }, error: { code: "COUPON_EXPIRED" } },
    { function: "orders.submit", fault: "unknown", delay: 100 },
    { function: "orders.status", sequence: [{ result: { state: "pending" } }, { result: { state: "paid" } }] },
  ];
  assert.deepEqual(issues({ $schema: "x", name: "checkout", description: "d", rules }, scenarioSchema), []);
  assert.deepEqual(
    issues({ name: "wifi", variants: { a: { rules: [rules[0]] }, "off-line.2": { description: "x", rules } } }, scenarioSchema),
    [],
  );
  // The showcase's files are valid scenarios.
  for (const path of [
    "../../../examples/lingxia-showcase/lxapp/tests/fixtures/network/outage.json",
    "../../../examples/lingxia-showcase/lxapp/tests/scenarios/route/status.json",
  ]) {
    assert.deepEqual(issues(load(path), scenarioSchema), [], path);
  }
});

test("the scenario schema rejects what the host rejects", () => {
  for (const file of [
    { rules: [] },
    { routes: [{ url: "**", status: 200 }] },
    { http: { routes: [{ url: "**", status: 200 }] } },
    { rule: [{ http: "GET **", status: 200 }] },
    { rules: [{ status: 200 }] },
    { rules: [{ http: "**/no-method", status: 200 }] },
    { rules: [{ http: "GET **", stauts: 200 }] },
    { rules: [{ http: "GET **", function: "f", status: 200 }] },
    { rules: [{ http: "GET **", match: { args: {} }, status: 200 }] },
    { rules: [{ http: "GET **", match: { query: {} }, status: 200 }] },
    { rules: [{ http: "GET **", sse: [{ drop: false }] }] },
    { rules: [{ function: "f" }] },
    { rules: [{ function: "f", result: 1, error: { code: "X" } }] },
    { rules: [{ function: "f", fault: "timeout" }] },
    { rules: [{ function: "f", error: {} }] },
    { rules: [{ function: "orders.*", result: 1 }] },
    { rules: [{ function: "f", status: 200 }] },
    { rules: [{ function: "f", sequence: [{ result: 1 }], delay: 5 }] },
    { variants: { a: { rules: [] } } },
    { variants: { "a b": { rules: [{ http: "GET **", status: 200 }] } } },
    { variants: { a: { rules: [{ http: "GET **", status: 200 }], extra: 1 } } },
  ]) {
    assert.notDeepEqual(issues(file, scenarioSchema), [], JSON.stringify(file));
  }
});

test("the lxdev.json schema describes test presets", () => {
  const file = {
    $schema: "./node_modules/@lingxia/test/schemas/lxdev.schema.json",
    test: {
      entry: "tests/",
      outputDir: "test-results",
      openapi: ["api.yaml"],
      tags: "unit",
      presets: { ci: ["--tag", "unit,routed", "--profile", "empty"], nightly: [] },
    },
  };
  assert.deepEqual(issues(file, lxdevSchema), []);
  for (const bad of [
    { presets: {} },
    { test: { profiles: {} } },
    { test: { presets: { ci: "--profile" } } },
    { test: { presets: { ci: [1] } } },
    { test: { entry: ["tests/"] } },
    { test: { tags: [1] } },
    { test: { outputDir: "" } },
  ]) {
    assert.notDeepEqual(issues(bad, lxdevSchema), [], JSON.stringify(bad));
  }
});
