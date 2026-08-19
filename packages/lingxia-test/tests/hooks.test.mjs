import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { createWorld, installFakeHost } from "./helpers/fake-host.mjs";
import { spec, reset } from "../dist/index.js";
import { registerOtherFileSpec, registerOtherFileHook } from "./helpers/other-file.mjs";

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

test("a hook from another file does not leak into this file's specs", async () => {
  const world = createWorld();
  installFakeHost(world);
  const ran = [];

  registerOtherFileHook(ran);
  spec("in this file", async () => {});

  const protocol = await globalThis.__LINGXIA_TEST__.run();
  assert.equal(protocol.failed, 0);
  assert.deepEqual(ran, [], "the other file's beforeEach ran for an unrelated spec");
});
