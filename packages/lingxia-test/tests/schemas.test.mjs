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

test("the scenario schema takes flat and sectioned files", () => {
  const routes = [
    { url: "**/v1/status", method: "GET", times: 2, sequence: [{ status: 503 }, { json: { up: true } }] },
    { url: "/devices\\/\\w+$/i", status: 404, note: "gone" },
    { url: "**/icon.png", bodyBase64: "iVBORw==", contentType: "image/png" },
    { url: "**/events", sse: [{ event: "status", data: { n: 1 }, id: "e1" }, { delayMs: 500 }, { drop: true }] },
    { url: "**/offline", abort: "failed" },
  ];
  assert.deepEqual(issues({ $schema: "x", name: "outage", description: "d", routes }, scenarioSchema), []);
  assert.deepEqual(issues({ name: "outage", http: { routes } }, scenarioSchema), []);
  // The showcase's fixture is a valid scenario.
  const outage = load("../../../examples/lingxia-showcase/lxapp/tests/fixtures/network/outage.json");
  assert.deepEqual(issues(outage, scenarioSchema), []);
});

test("the scenario schema rejects what the host rejects", () => {
  for (const file of [
    { routes: [] },
    { route: [{ url: "**", status: 200 }] },
    { routes: [{ status: 200 }] },
    { routes: [{ url: "**", stauts: 200 }] },
    { routes: [{ url: "**", status: 200 }], http: { routes: [{ url: "**", status: 200 }] } },
    { http: { routes: [{ url: "**", status: 200 }], fixtures: {} } },
    { routes: [{ url: "**", sse: [{ drop: false }] }] },
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
