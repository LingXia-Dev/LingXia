import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset } from "../dist/index.js";
import { registerOtherFileSpec, registerOtherFileHook, registerOtherFileSpecWithHook } from "./helpers/other-file.mjs";
import { installHooks } from "./helpers/install-hooks.mjs";

afterEach(() => {
  reset();
  delete globalThis.__LINGXIA_AUTOMATION_HOST__;
  delete globalThis.lx;
});

test("beforeEach runs only for specs declared in the same file", async () => {
  const world = createWorld();
  installFakeHost(world);
  const ran = [];

  spec.beforeEach(async () => {
    ran.push("this-file-hook");
  });
  spec("in this file", async () => {});
  registerOtherFileSpec("in the other file");

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0, JSON.stringify(protocol.cases));
  // One hook, one spec in its file — not one per spec in the run.
  assert.deepEqual(ran, ["this-file-hook"]);
});

test("a hook registered by a helper runs for the calling file's specs", async () => {
  const world = createWorld();
  installFakeHost(world);
  const ran = [];

  registerOtherFileHook(ran);
  spec("in this file", async () => {});

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0);
  assert.deepEqual(ran, ["other-file-hook"]);
});

test("a spec file's hook does not leak into another file's specs", async () => {
  const world = createWorld();
  installFakeHost(world);
  const ran = [];

  // other-file.mjs declares a spec, so it owns the hook it registers even
  // though this file called it.
  registerOtherFileSpecWithHook("in the other file", ran);
  spec("in this file", async () => {});

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0);
  assert.deepEqual(ran, ["other-file-hook"], "the other file's beforeEach ran for this file's spec");
});

test("reset, beforeEach and afterEach from a shared helper run in order", async () => {
  const world = createWorld();
  installFakeHost(world);
  const ran = [];

  installHooks(ran);
  spec("uses the shared hooks", async () => {
    ran.push("body");
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0, JSON.stringify(protocol.cases));
  assert.deepEqual(ran, ["reset", "beforeEach", "body", "afterEach"]);
});

test("--retries accepts a spec.reset registered by a helper", async () => {
  const world = createWorld();
  installFakeHost(world, { control: { retries: "1" } });
  const ran = [];
  let calls = 0;

  installHooks(ran);
  spec("intermittent", { forensics: false }, () => {
    calls += 1;
    if (calls === 1) throw new Error("first attempt fails");
  });

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.passed, 1, JSON.stringify(protocol.cases));
  assert.equal(protocol.cases[0].flaky, true);
  assert.equal(ran.filter((entry) => entry === "reset").length, 2);
});

test("a hook with no spec file on its stack is reported, not silently dropped", async () => {
  const world = createWorld();
  const { events } = installFakeHost(world);
  const ran = [];

  // This file declares no spec in this run, so neither it nor the helper
  // owns a spec the hooks could run for.
  installHooks(ran);
  registerOtherFileSpec("in the other file");

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0);
  assert.deepEqual(ran, []);
  const diagnostics = events.filter((event) => event.type === "diagnostic" && event.phase === "collect");
  assert.equal(diagnostics.length, 3);
  assert.match(diagnostics[0].message, /spec\.beforeEach\(\) registered from .*install-hooks\.mjs never runs/);
});
