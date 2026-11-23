import { AsyncLocalStorage } from "node:async_hooks";
import { readFileSync } from "node:fs";
import { LxError } from "@lingxia/types/error";
import { lx as importedLx, setLx } from "./node.js";

export { setLx } from "./node.js";
export { createLxMock } from "@lingxia/types/testing";

// The page runtime the Logic worker runs, copied in by `npm run build`.
let runtimeSource;
function pageRuntime() {
  runtimeSource ??= readFileSync(new URL("./page-runtime.js", import.meta.url), "utf8");
  return runtimeSource;
}

function unsupported(member) {
  return new LxError(`${member} is not available in mount()`, { code: "E_NOT_SUPPORTED" });
}

// `lx` is one module binding per process, so mounted pages share one `lx`: a second page with a
// different one throws instead of swapping it under the first.
const live = new Set();
let ownLx;
let previousLx;

function claimLx(lx) {
  if (live.size > 0) {
    if (lx !== ownLx) {
      throw new Error(
        "mount: another mounted page uses a different lx; unmount it first, or pass both the same lx",
      );
    }
    return;
  }
  ownLx = lx;
  previousLx = importedLx;
  setLx(lx);
}

function releaseLx(token) {
  live.delete(token);
  if (live.size === 0) {
    setLx(previousLx);
    ownLx = previousLx = undefined;
  }
}

// A timer belongs to the page whose setup, hook or action started it, followed through `await`
// by async context; one started from the test body or another page is not this page's.
const realTimers = { setTimeout, clearTimeout, setInterval, clearInterval };
const owner = new AsyncLocalStorage();
const forget = (id) => {
  for (const token of live) token.timers.delete(id);
};
function trackTimers() {
  if (globalThis.setTimeout !== realTimers.setTimeout) return;
  globalThis.setTimeout = (fn, ...rest) => {
    const token = owner.getStore();
    if (!token || typeof fn !== "function") return realTimers.setTimeout(fn, ...rest);
    const id = realTimers.setTimeout((...args) => {
      token.timers.delete(id);
      fn(...args);
    }, ...rest);
    token.timers.set(id, { kind: "timeout", at: startedAt() });
    return id;
  };
  globalThis.setInterval = (...args) => {
    const id = realTimers.setInterval(...args);
    owner.getStore()?.timers.set(id, { kind: "interval", at: startedAt() });
    return id;
  };
  globalThis.clearTimeout = (id) => {
    forget(id);
    realTimers.clearTimeout(id);
  };
  globalThis.clearInterval = (id) => {
    forget(id);
    realTimers.clearInterval(id);
  };
}
// The first stack frame in the author's code: not this file, the page runtime or Node.
const here = import.meta.url;
function startedAt() {
  const frames = (new Error().stack ?? "").split("\n").slice(1);
  const frame = frames.find(line => !line.includes(here) && !line.includes("node:") && !line.includes("<anonymous>"));
  return frame ? frame.trim().replace(/^at /, "") : "an unknown place";
}

function leakError(path, leaked) {
  const count = (kind, word) => {
    const n = leaked.filter(timer => timer.kind === kind).length;
    return n ? [`${n} ${word}${n === 1 ? "" : "s"}`] : [];
  };
  const what = [...count("timeout", "timeout"), ...count("interval", "interval")].join(" and ");
  const where = leaked.map(timer => `\n  ${timer.kind} started at ${timer.at}`).join("");
  return new Error(
    `${path} left ${what} running at unmount; use \`page.every\`/\`page.after\`, or stop it when \`page.signal\` aborts.${where}`,
  );
}

function untrackTimers() {
  if (live.size > 0) return;
  Object.assign(globalThis, realTimers);
}

// An async generator's body runs on each `next`, in the caller's context; run it in the page's.
function owned(token, value) {
  if (!value || typeof value.next !== "function" || typeof value[Symbol.asyncIterator] !== "function") {
    return value;
  }
  const step = (method) => (...args) => owner.run(token, () => value[method](...args));
  return { next: step("next"), return: step("return"), throw: step("throw"), [Symbol.asyncIterator]() { return this; } };
}

let mounted = 0;

// `page.after`/`page.every` run on this clock in `mount`; `advance` moves it and lets each run settle.
function manualClock() {
  let now = 0;
  let next = 0;
  const due = new Map();
  const settle = () => new Promise((resolve) => realTimers.setTimeout(resolve, 0));
  return {
    setTimeout(fn, ms) {
      const id = ++next;
      due.set(id, { at: now + ms, fn });
      return id;
    },
    clearTimeout(id) {
      due.delete(id);
    },
    async advance(ms) {
      if (typeof ms !== "number" || !(ms >= 0)) throw new TypeError("advance takes a number of ms");
      // Work already under way (a run that just ended) schedules before the clock moves.
      await settle();
      const end = now + ms;
      for (;;) {
        let first;
        for (const [id, timer] of due) {
          if (timer.at <= end && (!first || timer.at < first[1].at)) first = [id, timer];
        }
        if (!first) break;
        due.delete(first[0]);
        now = first[1].at;
        first[1].fn();
        await settle();
      }
      now = end;
      await settle();
    },
  };
}

export async function mount(definition, options = {}) {
  const { params = {}, lx = importedLx, name = "page", leakedTimers = "fail" } = options;
  // Page.js runs in this realm, so page data keeps the test's `Object.prototype`,
  // but its globals land on this scope, not on the test's `globalThis`.
  const scope = Object.create(globalThis);
  scope.globalThis = scope;
  Object.defineProperty(scope, "lx", { get: () => importedLx });
  // Stores live on the real global; they must see a page.update recipe in progress.
  const recipeDepth = Symbol.for("lingxia.recipeDepth");
  Object.defineProperty(scope, recipeDepth, { get: () => globalThis[recipeDepth], set: (n) => { globalThis[recipeDepth] = n; } });
  // The runtime's own setData batching keeps the real timers and clears its timer at unload.
  Object.assign(scope, realTimers);
  scope.PageSvc = class {
    constructor(config) {
      this.config = config;
    }
    _setData(_ops, ack) {
      ack();
      return Promise.resolve();
    }
  };
  scope.__LX_PAGE_NATIVE__ = {
    navigationBarUpdate: async () => {},
    startRefresh() {},
    stopRefresh() {},
    orientationLock() {},
    video() {
      throw unsupported("page.video");
    },
    env: () => ({ sizeClass: "compact", canDock: false }),
    envWatch: () => () => {},
  };
  const clock = manualClock();
  scope.__LX_PAGE_CLOCK__ = clock;
  new Function("scope", `with (scope) {\n${pageRuntime()}\n}`)(scope);
  scope.__lxCaptureTimers();

  claimLx(lx);
  const token = { timers: new Map() };
  live.add(token);
  trackTimers();
  let svc;
  let unmounted = false;
  // Returns the timers the page left running, now stopped.
  const release = () => {
    if (unmounted) return [];
    unmounted = true;
    const leaked = [...token.timers.values()];
    try {
      svc?._cancelPendingSetData();
      for (const id of token.timers.keys()) {
        realTimers.clearTimeout(id);
        realTimers.clearInterval(id);
      }
    } finally {
      token.timers.clear();
      releaseLx(token);
      untrackTimers();
    }
    return leaked;
  };
  const hook = async (key, ...args) => {
    const fn = svc.config[key];
    if (typeof fn === "function") await owner.run(token, () => fn.apply(svc, args));
  };

  const path = `pages/${name}/index`;
  try {
    const instanceId = `mount-${++mounted}`;
    scope.__registerDefinedPage(path, definition);
    svc = owner.run(token, () =>
      scope.__LX_CREATE_PAGE__(path, path, instanceId, false, JSON.stringify({ name, params })),
    );
    await hook("onLoad", params);
    await hook("onShow", {});
    await hook("onReady");
  } catch (error) {
    release();
    throw error;
  }

  // Called as the page wrote them, so a sync action stays sync and an async generator streams.
  const actions = {};
  for (const [key, fn] of Object.entries(svc.config.actions || {})) {
    actions[key] = (...args) => owned(token, owner.run(token, () => fn.apply(undefined, args)));
  }
  return {
    get data() {
      return svc.data;
    },
    actions,
    page: svc.page,
    show: (options = {}) => hook("onShow", options),
    advance: (ms) => clock.advance(ms),
    hide: () => hook("onHide"),
    async unmount() {
      if (unmounted) return;
      let leaked;
      try {
        // As the runtime does: the signal aborts just before `onUnload`.
        svc._abortLifetime();
        await hook("onUnload");
      } finally {
        leaked = release();
      }
      // The runtime never stops a page's timers, so one left running here leaks in the app.
      if (leaked.length > 0 && leakedTimers !== "allow") throw leakError(path, leaked);
    },
  };
}
