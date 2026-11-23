import assert from "node:assert/strict";
import { test } from "node:test";
import { definePage, defineStore, LxError, type, type PageLike } from "@lingxia/logic";
import { createLxMock, mount, setLx, type LxMockOverrides } from "@lingxia/logic/testing";
import { lxNow } from "./fixtures/lx-now.mjs";
import attachPage from "./fixtures/attach-page.ts";
import kindsPage, { trace } from "./fixtures/kinds-page.ts";

test("an action that calls lx.pick.file runs against a mock lx", async () => {
  const picks: unknown[] = [];
  const overrides: LxMockOverrides = {
    pick: {
      file: async (options) => {
        picks.push(options);
        return { status: "ok", file: { path: "lx://temp/a.pdf", name: "a.pdf", size: 3, type: "application/pdf" } };
      },
    },
  };
  const lx = createLxMock(overrides);
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

test("a timer the page left running fails unmount, naming where it started, and is stopped", async () => {
  trace.length = 0;
  const mounted = await mount(kindsPage, { lx: createLxMock() });
  await mounted.actions.later();
  await assert.rejects(mounted.unmount(), (error: Error) =>
    /pages\/page\/index left 1 timeout running/.test(error.message) &&
    /timeout started at .*kinds-page\.ts:\d+/.test(error.message) &&
    /stop it when `page\.signal` aborts/.test(error.message));
  await sleep(40);
  assert.equal(trace.includes("late"), false);
});

test("leakedTimers: 'allow' resolves unmount and still stops the interval", async () => {
  trace.length = 0;
  const mounted = await mount(kindsPage, { lx: createLxMock(), leakedTimers: "allow" });
  mounted.actions.leak();
  await mounted.unmount();
  const leaks = trace.filter(entry => entry === "leak").length;
  await sleep(30);
  assert.equal(trace.filter(entry => entry === "leak").length, leaks);
});

// A helper in `logic/` takes the page structurally, without `any`.
function clearList(page: PageLike<{ list: number[] }>) {
  page.set({ list: [] });
}

test("page.set assigns whole keys, leaves the others, and refuses inside page.update", async () => {
  const setPage = definePage({
    data: { n: 0, label: "a", list: [1, 2] as number[] },
    actions: page => ({
      assign() { page.set({ n: 2, label: "b" }); },
      clear() { clearList(page); },
      nested() { page.update(() => { page.set({ n: 3 }); }); },
      notAnObject() { (page.set as (patch: unknown) => void)([1]); },
    }),
  });
  const mounted = await mount(setPage, { lx: createLxMock() });
  try {
    mounted.actions.assign();
    assert.deepEqual(mounted.data, { n: 2, label: "b", list: [1, 2] });
    mounted.actions.clear();
    assert.deepEqual(mounted.data.list, []);
    assert.throws(() => mounted.actions.nested(), /page\.set cannot be called inside page\.update/);
    assert.throws(() => mounted.actions.notAnObject(), /page\.set takes an object/);
    assert.equal(mounted.data.n, 2);
  } finally {
    await mounted.unmount();
  }
});

test("onShow gets a tab switch's params on that show only", async () => {
  const shown: unknown[] = [];
  const tabPage = definePage({
    params: type<{ item?: string }>(),
    data: {},
    setup: () => ({
      onShow(options) {
        const item: string | undefined = options.params?.item;
        shown.push(item === undefined ? options : item);
      },
    }),
  });
  const mounted = await mount(tabPage, { lx: createLxMock() });
  try {
    await mounted.show({ params: { item: "7" } });
    await mounted.show();
    assert.deepEqual(shown, [{}, "7", {}]);
    assert.equal(Object.isFrozen(shown[0]), true);
  } finally {
    await mounted.unmount();
  }
});

test("a followed store is in the first data, follows each change, refuses page writes and stops at unmount", async () => {
  const session = defineStore({ userId: "u1", plan: "free" as "free" | "pro" }, { name: "session" });
  const seen: string[] = [];
  const page = definePage({
    data: { n: 0 },
    follow: { session, plan: session.pick(s => s.plan) },
    setup: p => {
      seen.push(p.data.plan);
      return {
        actions: {
          upgrade() { session.update(d => { d.plan = "pro"; }); },
          writePlan() { p.update(d => { (d as Record<string, unknown>).plan = "free"; }); },
        },
      };
    },
  });
  const mounted = await mount(page, { lx: createLxMock() });
  try {
    assert.deepEqual(seen, ["free"]);
    assert.deepEqual(mounted.data, { n: 0, session: { userId: "u1", plan: "free" }, plan: "free" });
    mounted.actions.upgrade();
    assert.equal(mounted.data.plan, "pro");
    assert.equal(mounted.data.session.plan, "pro");
    assert.throws(() => mounted.actions.writePlan(), /'plan' follows a Live; change its source/);
  } finally {
    await mounted.unmount();
  }
  session.set({ userId: "u2", plan: "free" });
  assert.equal(mounted.data.plan, "pro");
  await assert.rejects(
    mount(definePage({ data: { plan: "x" }, follow: { plan: session.pick(s => s.plan) } } as never), { lx: createLxMock() }),
    /'plan' is in both data and follow/,
  );
});

test("a store is one value for every page: an action's update reaches another page and a pick", async () => {
  const cart = defineStore({ items: [] as string[] }, { name: "cart" });
  const picks: number[] = [];
  const count = cart.pick(c => c.items.length);
  const stop = count.watch(n => picks.push(n));
  const shop = definePage({
    actions: () => ({
      add(item: string) { cart.update(d => { d.items.push(item); }); },
      clear() { cart.set({ items: [] }); },
    }),
  });
  const writer = definePage({
    data: { n: 0 },
    actions: page => ({
      inRecipe() { page.update(() => { cart.set({ items: ["x"] }); }); },
    }),
  });
  const lx = createLxMock();
  const a = await mount(shop, { lx });
  const b = await mount(writer, { lx });
  try {
    a.actions.add("tea");
    a.actions.add("tea");
    assert.deepEqual(cart.get(), { items: ["tea", "tea"] });
    assert.equal(Object.isFrozen(cart.get().items), true);
    assert.throws(() => b.actions.inRecipe(), /cart: store\.set cannot be called inside a page\.update or store\.update recipe/);
    a.actions.clear();
    assert.deepEqual(picks, [0, 1, 2, 0]);
    stop();
    a.actions.add("milk");
    assert.deepEqual(picks, [0, 1, 2, 0]);
    assert.equal(count.get(), 1);
  } finally {
    await a.unmount();
    await b.unmount();
  }
});
