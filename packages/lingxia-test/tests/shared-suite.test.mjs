import assert from "node:assert/strict";
import { fileURLToPath, pathToFileURL } from "node:url";
import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { expect, test as lxTest } from "../dist/index.js";
import { reset, run, skipTestFiles } from "../dist/runner.js";

const here = fileURLToPath(import.meta.url).replaceAll("\\", "/");

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

test("skipTestFiles skips a file's specs with the reason", async () => {
  let ran = false;
  lxTest("webview only", async () => { ran = true; });
  skipTestFiles({ [here]: "reads the WebView DOM" });
  installFakeHost(createWorld());
  const report = await run();
  assert.equal(ran, false);
  assert.equal(report.cases[0].status, "skipped");
  assert.equal(report.cases[0].reason, "reads the WebView DOM");
});

test("a skip naming a file the run does not carry warns and is listed as skipUnused", async () => {
  lxTest("one", async () => {});
  skipTestFiles({ "tests/gone.test.ts": "removed" });
  const host = installFakeHost(createWorld());
  const report = await run();
  assert.equal(report.cases[0].status, "passed");
  assert.deepEqual(report.skipUnused, ["tests/gone.test.ts"]);
  assert.ok(host.events.some((e) => e.type === "diagnostic" && /not part of this run/.test(e.message)));
});

test("a skip naming a bundled file that registers no spec warns and is listed as skipUnused", async () => {
  lxTest("one", async () => {});
  skipTestFiles({ "tests/empty.test.ts": "renderer reason" });
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

test("a root-relative skip matches a shared spec reached through a symlinked package", async () => {
  // A checkout whose package dir links to another checkout's, as npm links it (a junction on Windows,
  // which needs no privilege; "junction" is a plain dir link elsewhere): the spec's source is the real path.
  const base = mkdtempSync(join(tmpdir(), "lx-shared-"));
  const real = join(base, "LingXia-gpu", "packages", "lingxia-test", "showcase", "pages");
  mkdirSync(real, { recursive: true });
  const dist = pathToFileURL(fileURLToPath(new URL("../dist/index.js", import.meta.url))).href;
  writeFileSync(join(real, "render.test.mjs"), `import { test } from ${JSON.stringify(dist)};\ntest("webview render", async () => { globalThis.__ranShared = true; });\n`);
  mkdirSync(join(base, "LingXia-gpu-build", "packages", "node_modules", "@lingxia"), { recursive: true });
  symlinkSync(join(base, "LingXia-gpu", "packages", "lingxia-test"), join(base, "LingXia-gpu-build", "packages", "node_modules", "@lingxia", "test"), "junction");
  try {
    await import(pathToFileURL(join(base, "LingXia-gpu-build", "packages", "node_modules", "@lingxia", "test", "showcase", "pages", "render.test.mjs")).href);
    skipTestFiles({ "pages/render.test.mjs": "reads the WebView DOM" }, { root: ["lingxia-test/showcase", "@lingxia/test/showcase"] });
    installFakeHost(createWorld());
    const report = await run();
    assert.equal(globalThis.__ranShared, undefined, "the shared spec did not run");
    assert.equal(report.cases[0].status, "skipped");
    assert.equal(report.cases[0].reason, "reads the WebView DOM");
    assert.equal(report.skipUnused, undefined, "nothing unused");
  } finally {
    delete globalThis.__ranShared;
    rmSync(base, { recursive: true, force: true });
  }
});

test("a root-relative skip matches only under that directory, and a root takes no ..", async () => {
  lxTest("one", async () => {});
  skipTestFiles({ "shared-suite.test.mjs": "elsewhere" }, { root: "lingxia-test/showcase" });
  installFakeHost(createWorld());
  const report = await run();
  assert.equal(report.cases[0].status, "passed");
  assert.deepEqual(report.skipUnused, ["lingxia-test/showcase/shared-suite.test.mjs"]);
  reset();
  skipTestFiles({ "shared-suite.test.mjs": "here" }, { root: "lingxia-test/tests" });
  lxTest("two", async () => {});
  installFakeHost(createWorld());
  assert.equal((await run()).cases[0].status, "skipped");
  assert.throws(() => skipTestFiles({ x: "y" }, { root: "../showcase" }), /without \.\./);
});

test("a skip needs a reason", () => {
  assert.throws(() => skipTestFiles({ [here]: " " }), /needs a reason/);
});

test("each case records the renderers its locators read", async () => {
  const world = createWorld();
  world.add({ testId: "gpu-title", renderer: "gpu" });
  world.add({ testId: "web-title" });
  lxTest("gpu page", async (t) => { await expect(t.app.view.testId("gpu-title")).toBeVisible(); });
  lxTest("web page", async (t) => { await expect(t.app.view.testId("web-title")).toBeVisible(); });
  lxTest("no page", async () => {});
  installFakeHost(world);
  const report = await run();
  assert.deepEqual(report.cases.map((c) => c.renderers), [["gpu"], ["webview"], undefined]);
});
