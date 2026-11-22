import assert from "node:assert/strict";
import { test } from "node:test";
import { createLxMock } from "@lingxia/types/testing";
import { mount } from "@lingxia/logic/testing";
import attachPage from "./fixtures/attach-page.ts";

test("an action that calls lx.pick.file runs against a mock lx", async () => {
  const picks: unknown[] = [];
  const lx = createLxMock({
    pick: {
      file: async (options) => {
        picks.push(options);
        return { status: "ok", file: { path: "lx://temp/a.pdf", name: "a.pdf", size: 3, type: "application/pdf" } };
      },
    },
  });
  const mounted = await mount(attachPage, { lx });
  try {
    assert.equal(await mounted.actions.attach(), "ok");
    assert.equal(mounted.data.attached, "a.pdf");
    assert.deepEqual(picks, [{ filters: [{ extensions: ["pdf"] }] }]);
  } finally {
    await mounted.unmount();
  }
});

test("a member the mock was not given rejects E_NOT_SUPPORTED naming itself", async () => {
  const mounted = await mount(attachPage, { lx: createLxMock() });
  try {
    assert.equal(await mounted.actions.attach(), "failed");
    assert.equal(mounted.data.error, "E_NOT_SUPPORTED");
  } finally {
    await mounted.unmount();
  }
  const lx = createLxMock();
  await assert.rejects(lx.pick.file(), /lx\.pick\.file is not mocked/);
  assert.throws(() => lx.device.screen(), (error: { code?: string; message: string }) =>
    error.code === "E_NOT_SUPPORTED" && error.message.startsWith("lx.device.screen"));
  assert.throws(() => lx.app.info.appId, /lx\.app\.info\.appId is not mocked/);
});

test("unmount aborts page.signal", async () => {
  const mounted = await mount(attachPage, { lx: createLxMock() });
  const signal = mounted.page.signal;
  assert.equal(signal.aborted, false);
  await mounted.unmount();
  assert.equal(signal.aborted, true);
});
