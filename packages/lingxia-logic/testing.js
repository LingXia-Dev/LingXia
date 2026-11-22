import { readFileSync } from "node:fs";
import { LxError } from "@lingxia/types/error";
import { lx as importedLx, setLx } from "./node.js";

export { setLx } from "./node.js";

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

// Timers started while a page is mounted, so unmount can stop them as the page's work ends.
const realTimers = { setTimeout, clearTimeout, setInterval, clearInterval };
function trackTimers() {
  if (globalThis.setTimeout !== realTimers.setTimeout) return;
  const track = (start) => (...args) => {
    const id = start(...args);
    for (const token of live) token.timers.add(id);
    return id;
  };
  globalThis.setTimeout = track(realTimers.setTimeout);
  globalThis.setInterval = track(realTimers.setInterval);
}
function untrackTimers() {
  if (live.size > 0) return;
  Object.assign(globalThis, realTimers);
}

let mounted = 0;

export async function mount(definition, options = {}) {
  const { params = {}, lx = importedLx, name = "page" } = options;
  // Page.js runs in this realm, so page data keeps the test's `Object.prototype`,
  // but its globals land on this scope, not on the test's `globalThis`.
  const scope = Object.create(globalThis);
  scope.globalThis = scope;
  Object.defineProperty(scope, "lx", { get: () => importedLx });
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
  new Function("scope", `with (scope) {\n${pageRuntime()}\n}`)(scope);
  scope.__lxCaptureTimers();

  claimLx(lx);
  const token = { timers: new Set() };
  live.add(token);
  trackTimers();
  let svc;
  let unmounted = false;
  const release = () => {
    if (unmounted) return;
    unmounted = true;
    svc?._cancelPendingSetData();
    for (const id of token.timers) {
      realTimers.clearTimeout(id);
      realTimers.clearInterval(id);
    }
    releaseLx(token);
    untrackTimers();
  };
  const hook = async (key, ...args) => {
    const fn = svc.config[key];
    if (typeof fn === "function") await fn.apply(svc, args);
  };

  try {
    const path = `pages/${name}/index`;
    const instanceId = `mount-${++mounted}`;
    scope.__registerDefinedPage(path, definition);
    svc = scope.__LX_CREATE_PAGE__(path, path, instanceId, false, JSON.stringify({ name, params }));
    await hook("onLoad", params);
    await hook("onShow");
    await hook("onReady");
  } catch (error) {
    release();
    throw error;
  }

  // Called as the page wrote them, so a sync action stays sync and an async generator streams.
  const actions = {};
  for (const [key, fn] of Object.entries(svc.config.actions || {})) {
    actions[key] = (...args) => fn.apply(undefined, args);
  }
  return {
    get data() {
      return svc.data;
    },
    actions,
    page: svc.page,
    show: () => hook("onShow"),
    hide: () => hook("onHide"),
    async unmount() {
      if (unmounted) return;
      // As the runtime does: the signal aborts just before `onUnload`.
      svc._abortLifetime();
      try {
        await hook("onUnload");
      } finally {
        release();
      }
    },
  };
}
