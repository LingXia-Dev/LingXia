import assert from "node:assert/strict";
import { test } from "node:test";
import { definePage, defineStore, derive, LxError, type, type PageLike } from "@lingxia/logic";
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

test("a derived Live recomputes from each source, notifies on a real change, and a page follows it", async () => {
  const override = defineStore(null as string | null, { name: "override" });
  const host = defineStore("en", { name: "host language" });
  let runs = 0;
  const language = derive([override, host], (o, l) => { runs += 1; return o ?? l; });
  assert.equal(language.get(), "en");
  const seen: string[] = [];
  const stop = language.watch(l => seen.push(l));
  host.set("fr");
  override.set("de");
  host.set("es");
  override.set(null);
  assert.deepEqual(seen, ["en", "fr", "de", "es"]);
  stop();
  const before = runs;
  host.set("it");
  assert.equal(runs, before, "an unwatched derive holds no source");
  assert.equal(language.get(), "it");

  const page = definePage({ follow: { language }, actions: {} });
  const mounted = await mount(page, { lx: createLxMock() });
  try {
    assert.equal(mounted.data.language, "it");
    override.set("ja");
    assert.equal(mounted.data.language, "ja");
  } finally {
    await mounted.unmount();
  }
});

const tick = () => new Promise<void>(resolve => setTimeout(() => resolve(), 0));

function deferred<T = void>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

test("a persisted store loads once, keeps initial when empty or failing, and a change before the load wins", async () => {
  const empty = defineStore({ n: 0 }, { persist: { load: async () => undefined, save: async () => {} } });
  assert.deepEqual(await empty.ready, { ok: true });
  assert.deepEqual(empty.get(), { n: 0 });

  const boom = new Error("disk");
  let fail = true;
  const failing = defineStore({ n: 0 }, {
    persist: { load: async () => { if (fail) throw boom; return { n: 7 }; }, save: async () => {} },
  });
  assert.deepEqual(await failing.ready, { ok: false, error: boom });
  assert.deepEqual(failing.get(), { n: 0 });
  fail = false;
  assert.deepEqual(await failing.reload(), { ok: true });
  assert.deepEqual(failing.get(), { n: 7 });

  const slow = deferred<{ n: number }>();
  const saves: unknown[] = [];
  const raced = defineStore({ n: 0 }, { persist: { load: () => slow.promise, save: async v => { saves.push(v); } } });
  raced.set({ n: 1 });
  slow.resolve({ n: 9 });
  assert.deepEqual(await raced.ready, { ok: true });
  assert.deepEqual(raced.get(), { n: 1 });
  assert.deepEqual(saves, [{ n: 1 }], "a loaded value is never written back");
});

test("a persisted store saves one at a time with the newest value, and saved reports each finished save", async () => {
  const first = deferred();
  const saves: number[] = [];
  const reports: unknown[] = [];
  const boom = new Error("full");
  let failNext = false;
  const counter = defineStore(0, {
    persist: {
      load: async () => undefined,
      save: async v => {
        saves.push(v);
        if (saves.length === 1) await first.promise;
        if (failNext) throw boom;
      },
    },
  });
  await counter.ready;
  counter.saved.watch(r => reports.push(r));
  counter.set(1);                       // save(1) starts and waits
  await tick();
  counter.set(2);
  counter.set(3);                       // marks dirty; 2 is never saved
  assert.deepEqual(saves, [1]);
  first.resolve();
  await tick();
  assert.deepEqual(saves, [1, 3]);
  assert.deepEqual(reports, [{ ok: true }, { ok: true }, { ok: true }]);
  failNext = true;
  counter.set(4);
  await tick();
  assert.deepEqual(counter.saved.get(), { ok: false, error: boom });
  assert.equal(counter.get(), 4, "the value holds whatever the save did");
});

test("persist: { key } reads and writes one lx.storage key", async () => {
  const stored = new Map<string, unknown>([["theme", "dark"]]);
  setLx(createLxMock({
    storage: {
      get: async (key: string) => stored.get(key),
      set: async (key: string, value: unknown) => { stored.set(key, value); },
    },
  } as never));
  try {
    const theme = defineStore("light", { persist: { key: "theme" } });
    assert.deepEqual(await theme.ready, { ok: true });
    assert.equal(theme.get(), "dark");
    theme.set("light");
    await tick();
    assert.equal(stored.get("theme"), "light");
    assert.throws(() => (theme.update as (recipe: () => void) => void)(() => {}), /theme: store\.update needs an object/);
  } finally {
    setLx(undefined);
  }
});

test("page.every waits for each run to end, pauses while hidden and runs a missed beat on show", async () => {
  const runs: number[] = [];
  let release: (() => void) | undefined;
  const page = definePage({
    setup: p => ({
      onLoad() {
        p.every(100, async () => {
          runs.push(runs.length);
          if (runs.length === 2) await new Promise<void>(resolve => { release = resolve; });
        });
      },
    }),
  });
  const mounted = await mount(page, { lx: createLxMock() });
  try {
    await mounted.advance(100);
    assert.deepEqual(runs, [0]);
    await mounted.advance(100);
    assert.deepEqual(runs, [0, 1]);
    await mounted.advance(500);
    assert.deepEqual(runs, [0, 1], "a slow run is never overlapped");
    release!();
    await mounted.advance(100);
    assert.deepEqual(runs, [0, 1, 2], "the next wait starts when the run ends");
    await mounted.hide();
    await mounted.advance(1000);
    assert.deepEqual(runs, [0, 1, 2], "paused while hidden");
    await mounted.show();
    await mounted.advance(0);
    assert.deepEqual(runs, [0, 1, 2, 3], "the missed beat runs on show");
    await mounted.advance(100);
    assert.deepEqual(runs, [0, 1, 2, 3, 4]);
  } finally {
    await mounted.unmount();
  }
});

test("page.after runs once, shown or not; whenHidden 'run' keeps every going; a failure is reported and the timer goes on; unload stops both", async () => {
  const logged: unknown[][] = [];
  const consoleError = console.error;
  console.error = (...args: unknown[]) => { logged.push(args); };
  const seen: string[] = [];
  let stopLate!: () => void;
  let api!: PageLike<{}> & { after(ms: number, fn: () => void): () => void };
  const page = definePage({
    setup: p => {
      api = p as never;
      p.after(50, () => { seen.push("after"); });
      stopLate = p.after(80, () => { seen.push("late"); });
      p.every(100, () => { seen.push("tick"); throw new Error("flaky"); }, { whenHidden: "run" });
      return {};
    },
  });
  const mounted = await mount(page, { lx: createLxMock() });
  await mounted.hide();
  stopLate();
  try {
    await mounted.advance(250);
  } finally {
    console.error = consoleError;
  }
  assert.deepEqual(seen, ["after", "tick", "tick"]);
  assert.equal(logged.length, 2);
  assert.equal(/page\.every in 'page' failed/.test(String(logged[0][0])), true);
  await mounted.unmount();
  await mounted.advance(1000);
  assert.deepEqual(seen, ["after", "tick", "tick"], "unload stops every timer");
  api.after(10, () => { seen.push("after unload"); });
  await mounted.advance(100);
  assert.deepEqual(seen, ["after", "tick", "tick"], "a timer does not start after unload");
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
