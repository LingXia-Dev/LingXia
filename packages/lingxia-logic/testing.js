import { readFileSync } from "node:fs";
import { lx as importedLx, setLx } from "./node.js";

export { setLx } from "./node.js";

// The page runtime the Logic worker runs, copied in by `npm run build`.
let runtimeSource;
function pageRuntime() {
  runtimeSource ??= readFileSync(new URL("./page-runtime.js", import.meta.url), "utf8");
  return runtimeSource;
}

function unsupported(member) {
  return Object.assign(new Error(`${member} is not available in mount()`), { code: "E_NOT_SUPPORTED" });
}

let mounted = 0;

export async function mount(definition, options = {}) {
  const { params = {}, lx = importedLx, name = "page" } = options;
  // Page.js runs in this realm, so page data keeps the test's `Object.prototype`,
  // but its globals land on this scope, not on the test's `globalThis`.
  const scope = Object.create(globalThis);
  scope.globalThis = scope;
  scope.lx = lx;
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

  const previousLx = importedLx;
  if (lx !== importedLx) setLx(lx);

  const path = `pages/${name}/index`;
  const instanceId = `mount-${++mounted}`;
  scope.__registerDefinedPage(path, definition);
  const svc = scope.__LX_CREATE_PAGE__(path, path, instanceId, false, JSON.stringify({ name, params }));
  const hook = async (key, ...args) => {
    const fn = svc.config[key];
    if (typeof fn === "function") await fn.apply(svc, args);
  };
  await hook("onLoad", params);
  await hook("onShow");
  await hook("onReady");

  const actions = {};
  for (const [key, fn] of Object.entries(svc.config.actions || {})) {
    actions[key] = async (...args) => fn.apply(undefined, args);
  }
  let unmounted = false;
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
      unmounted = true;
      await hook("onUnload");
      svc._cancelPendingSetData();
      if (lx !== previousLx) setLx(previousLx);
    },
  };
}
