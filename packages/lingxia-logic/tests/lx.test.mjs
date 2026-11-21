import assert from "node:assert/strict";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

test("the runtime entry exports the host's own lx object", async () => {
  const host = { nav: {} };
  globalThis.lx = host;
  try {
    const runtime = await import(new URL("../index.js", import.meta.url));
    assert.equal(runtime.lx, host, "the same object, not a copy or a wrapper");
    assert.equal(typeof runtime.definePage, "function");
    assert.equal(typeof runtime.isLxError, "function");
  } finally {
    delete globalThis.lx;
  }
});

test("in Node, setLx reaches a module that imported lx before the test ran", async () => {
  const { lx: before } = await import("@lingxia/logic");
  assert.equal(before, undefined, "no host in Node until a test provides one");
  const page = await import(pathToFileURL(new URL("./fixtures/uses-lx.mjs", import.meta.url).pathname).href);
  const { setLx } = await import("@lingxia/logic/testing");
  const mock = { app: { id: "mock" } };
  setLx(mock);
  try {
    assert.equal(page.appId(), "mock");
  } finally {
    setLx(undefined);
  }
});
