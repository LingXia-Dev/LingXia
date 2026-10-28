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

test("a skip that names no spec file fails the run, so the list cannot go stale", async () => {
  spec("one", async () => {});
  skipSpecFiles({ "tests/gone.test.ts": "removed" });
  installFakeHost(createWorld());
  await assert.rejects(run(), /registered no spec/);
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
