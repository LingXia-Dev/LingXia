import assert from "node:assert/strict";
import { test } from "node:test";
import { createLxMock } from "@lingxia/types/testing";
import { LxError } from "@lingxia/logic";
import { mount, setLx } from "@lingxia/logic/testing";
import { lxNow } from "./fixtures/lx-now.mjs";
import attachPage from "./fixtures/attach-page.ts";
import kindsPage, { trace } from "./fixtures/kinds-page.ts";

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

test("actions keep their kind: sync returns, async resolves, an async generator streams and closes", async () => {
  trace.length = 0;
  const mounted = await mount(kindsPage, { lx: createLxMock() });
  try {
    const added: number = mounted.actions.add(2);
    assert.equal(added, 2);
    assert.equal(await mounted.actions.save("a", 3), "ax3");
    const seen: number[] = [];
    for await (const n of mounted.actions.count(5)) {
      seen.push(n);
      if (n === 2) break;
    }
    assert.deepEqual(seen, [1, 2]);
    assert.equal(trace.includes("count closed"), true);
  } finally {
    await mounted.unmount();
  }
});

test("a member mount cannot host throws an LxError E_NOT_SUPPORTED", async () => {
  const mounted = await mount(kindsPage, { lx: createLxMock() });
  try {
    assert.throws(() => mounted.actions.video(), (error: LxError) =>
      error instanceof LxError && error.code === "E_NOT_SUPPORTED" && /page\.video/.test(error.message));
  } finally {
    await mounted.unmount();
  }
});

test("a hook that throws unmounts the page and puts lx back", async () => {
  const before = createLxMock();
  setLx(before);
  try {
    await assert.rejects(mount(kindsPage, { params: { fail: "boom" }, lx: createLxMock() }), /boom/);
    assert.equal(lxNow(), before);
    const mounted = await mount(kindsPage, { lx: createLxMock() });
    await mounted.unmount();
    await mounted.unmount();
    assert.equal(lxNow(), before);
  } finally {
    setLx(undefined);
  }
});

test("pages mounted together share one lx", async () => {
  const lx = createLxMock();
  const first = await mount(kindsPage, { lx });
  try {
    await assert.rejects(mount(kindsPage, { lx: createLxMock() }), /different lx/);
    const second = await mount(kindsPage, { lx });
    await second.unmount();
    assert.equal(lxNow(), lx, "the first page keeps its lx");
  } finally {
    await first.unmount();
  }
});

test("unmount aborts the signal before onUnload and stops the page's timers", async () => {
  trace.length = 0;
  const mounted = await mount(kindsPage, { lx: createLxMock() });
  await new Promise<void>(resolve => setTimeout(() => resolve(), 20));
  await mounted.unmount();
  const ticks = trace.filter(entry => entry === "tick").length;
  assert.equal(ticks > 0, true);
  assert.equal(trace.includes("unload aborted=true"), true);
  await new Promise<void>(resolve => setTimeout(() => resolve(), 20));
  assert.equal(trace.filter(entry => entry === "tick").length, ticks);
});

const sleep = (ms: number) => new Promise<void>(resolve => setTimeout(() => resolve(), ms));

test("a timer the test started while a page is mounted survives unmount", async () => {
  const mounted = await mount(kindsPage, { lx: createLxMock() });
  const done = sleep(30);
  await mounted.unmount();
  await done;
});

test("unmounting one page leaves another page's interval running", async () => {
  trace.length = 0;
  const lx = createLxMock();
  const a = await mount(kindsPage, { lx });
  const b = await mount(kindsPage, { lx });
  try {
    a.actions.poll("a");
    b.actions.poll("b");
    await a.unmount();
    const before = trace.filter(entry => entry === "b").length;
    await sleep(30);
    assert.equal(trace.filter(entry => entry === "b").length > before, true);
  } finally {
    await b.unmount();
  }
});

test("a timer an action starts after an await stops at unmount", async () => {
  trace.length = 0;
  const mounted = await mount(kindsPage, { lx: createLxMock() });
  await mounted.actions.later();
  await mounted.unmount();
  await sleep(40);
  assert.equal(trace.includes("late"), false);
});
