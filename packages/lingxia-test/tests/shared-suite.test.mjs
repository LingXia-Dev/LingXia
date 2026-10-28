import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { expect, spec } from "../dist/index.js";
import { reset, run, skipSpecFiles } from "../dist/runner.js";

const here = fileURLToPath(import.meta.url).replaceAll("\\", "/");

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

test("skipSpecFiles skips a file's specs with the reason", async () => {
  let ran = false;
  spec("webview only", async () => { ran = true; });
  skipSpecFiles({ [here]: "reads the WebView DOM" });
  installFakeHost(createWorld());
  const report = await run();
  assert.equal(ran, false);
  assert.equal(report.cases[0].status, "skipped");
  assert.equal(report.cases[0].reason, "reads the WebView DOM");
});

test("a skip naming a file the run does not carry warns and is listed as skipUnused", async () => {
  spec("one", async () => {});
  skipSpecFiles({ "tests/gone.test.ts": "removed" });
  const host = installFakeHost(createWorld());
  const report = await run();
  assert.equal(report.cases[0].status, "passed");
  assert.deepEqual(report.skipUnused, ["tests/gone.test.ts"]);
  assert.ok(host.events.some((e) => e.type === "diagnostic" && /not part of this run/.test(e.message)));
});

test("a skip naming a bundled file that registers no spec warns and is listed as skipUnused", async () => {
  spec("one", async () => {});
  skipSpecFiles({ "tests/empty.test.ts": "renderer reason" });
  globalThis.__LINGXIA_TEST_SOURCE_MAP__ = { sources: ["tests/empty.test.ts", here], mappings: "" };
  const host = installFakeHost(createWorld());
  try {
    const report = await run();
    assert.equal(report.cases[0].status, "passed");
    assert.deepEqual(report.skipUnused, ["tests/empty.test.ts"]);
    assert.ok(host.events.some((e) => e.type === "diagnostic" && /registered no spec/.test(e.message)));
  } finally {
    delete globalThis.__LINGXIA_TEST_SOURCE_MAP__;
  }
});

test("a skip needs a reason", () => {
  assert.throws(() => skipSpecFiles({ [here]: " " }), /needs a reason/);
});

test("each case records the renderers its locators read", async () => {
  const world = createWorld();
  world.add({ testId: "gpu-title", renderer: "gpu" });
  world.add({ testId: "web-title" });
  spec("gpu page", async (t) => { await expect(t.app.view.testId("gpu-title")).toBeVisible(); });
  spec("web page", async (t) => { await expect(t.app.view.testId("web-title")).toBeVisible(); });
  spec("no page", async () => {});
  installFakeHost(world);
  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.renderers), [["gpu"], ["webview"], undefined]);
});
