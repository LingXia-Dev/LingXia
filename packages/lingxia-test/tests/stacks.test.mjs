import assert from "node:assert/strict";
import { test } from "node:test";
import { parseFrames, resolveOrigin } from "../dist/ids.js";

const V8 = [
  "Error",
  "    at callerLocation (/repo/packages/lingxia-test/dist/ids.js:44:17)",
  "    at register (/repo/packages/lingxia-test/dist/runtime.js:70:19)",
  "    at file:///repo/app/tests/pages/home.test.js:12:1",
].join("\n");

// JavaScriptCore (macOS/iOS) and ArkJS write `name@file:line:col`.
const JSC = [
  "callerLocation@lxdev-test://tests/entries/macos.test.ts:2538:54",
  "register@lxdev-test://tests/entries/macos.test.ts:2536:35",
  "@lxdev-test://tests/entries/macos.test.ts:2548:3",
  "eval@[native code]",
].join("\n");

test("parses V8 and JavaScriptCore frames alike", () => {
  const v8 = parseFrames(V8);
  assert.deepEqual(v8[0], {
    file: "/repo/packages/lingxia-test/dist/ids.js",
    line: 44,
    column: 17,
  });
  assert.equal(v8.at(-1).file, "/repo/app/tests/pages/home.test.js");

  const jsc = parseFrames(JSC);
  assert.equal(jsc.length, 3, "the [native code] frame carries no position");
  assert.deepEqual(jsc[2], {
    file: "lxdev-test://tests/entries/macos.test.ts",
    line: 2548,
    column: 3,
  });
});

test("resolveOrigin skips this package's own frames", () => {
  const origin = resolveOrigin(parseFrames(V8));
  assert.equal(origin.file, "/repo/app/tests/pages/home.test.js");
  assert.equal(origin.line, 12);
});

test("an unparsable stack degrades to unknown rather than throwing", () => {
  assert.deepEqual(parseFrames(undefined), []);
  assert.deepEqual(resolveOrigin([]), { file: "unknown", line: 0, column: 0 });
});

test("the bundle source map moves a frame back to the authored file", async () => {
  // sources[0] maps generated line 1 col 0 -> tests/pages/home.test.ts line 4.
  globalThis.__LINGXIA_TEST_SOURCE_MAP__ = {
    version: 3,
    sources: ["tests/pages/home.test.ts"],
    mappings: "AAGA",
  };
  try {
    const origin = resolveOrigin([
      { file: "lxdev-test://tests/entries/macos.test.ts", line: 1, column: 1 },
    ]);
    assert.equal(origin.file, "tests/pages/home.test.ts");
    assert.equal(origin.line, 4);
  } finally {
    delete globalThis.__LINGXIA_TEST_SOURCE_MAP__;
  }
});
