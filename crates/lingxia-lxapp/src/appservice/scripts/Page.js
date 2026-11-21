(function () {
  const pageDefinitions = new Map();
  // setData batching keeps real time. The timer globals are installed after
  // this script runs, so the Logic worker calls the hidden capture hook right
  // after the timer module is initialised — before app code, and so before a
  // test clock, can replace them. A faked clock then never holds View updates.
  let realTimers = null;
  function captureTimers() {
    const set = globalThis.setTimeout;
    const clear = globalThis.clearTimeout;
    if (typeof set !== "function" || typeof clear !== "function") {
      throw new Error("setData: timers are not available to capture");
    }
    realTimers = { set, clear };
  }
  // A View `<link to>`: `{ back: n }` or a target of `lx.nav.go`, settled
  // once the destination is on screen. A link to where its page already is does nothing.
  Object.defineProperty(globalThis, "__lxNavForView", {
    value: async (target, svc) => {
      if (target.back !== undefined) return lx.nav.back(target.back);
      const here = svc && svc.page;
      if (here && target.app === undefined && (target.tab !== undefined
        ? target.tab === here.name
        : target.page === here.name && isDeepEqual(target.params || {}, here.params || {}))) return;
      const { replace, ...to } = target;
      const went = replace === true ? lx.nav.go(to, { replace: true }) : lx.nav.go(to);
      await (went && went.opened ? went.opened : went);
    },
    configurable: true,
  });
  Object.defineProperty(globalThis, "__lxCaptureTimers", {
    value: captureTimers,
    enumerable: false,
    writable: false,
    configurable: true,
  });
  function timers() {
    if (!realTimers) {
      throw new Error("setData: timers were not captured when the Logic worker started");
    }
    return realTimers;
  }
  function setTimer(fn, delay) {
    return timers().set(fn, delay);
  }
  function clearTimer(id) {
    return timers().clear(id);
  }

  function createPageInstance(definition, pagePath, pageInstanceId, gpu, lifetime = new AbortController()) {
    const pageConfig = definition && definition.config;
    if (!pageConfig || typeof pageConfig !== "object") {
      throw new Error("Invalid page configuration");
    }
    assertPageData(pageConfig.data || {});
    // Only hooks and the `actions` object reach the service, so a page's own
    // names never shadow the runtime's members.
    const svcConfig = { actions: {} };
    for (const [key, value] of Object.entries(pageConfig)) {
      if (key === "data" || key === "route") continue;
      if (key === "actions") {
        Object.assign(svcConfig.actions, value);
      } else if (PAGE_HOOKS.includes(key) && typeof value === "function") {
        svcConfig[key] = value;
      } else {
        throw new TypeError(`Page config: '${key}' is not a lifecycle hook; put View-callable functions under actions`);
      }
    }
    const pageSvc = new PageSvc(
      svcConfig,
      pagePath,
      definition.bindingMetaJson || "{}",
      pageInstanceId || "",
    );

    pageSvc.data = cloneJsonValue(pageConfig.data || {});
    pageSvc.route = pageConfig.route;
    // For `lxdev test`: a spec calls an action the way the View does, by name.
    Object.defineProperty(pageSvc, "actions", {
      value: Object.freeze({ ...svcConfig.actions }),
      enumerable: false,
    });

    // Aborted as the page unloads — just before `onUnload` runs, or when the
    // page is retired without one — so work the page started (a fetch, a
    // subscription) stops with it instead of resolving into a page that is gone.
    Object.defineProperty(pageSvc, "signal", {
      value: lifetime.signal,
      enumerable: false,
      writable: false,
      configurable: false,
    });

    let updateTimer = null;
    const pendingBaseState = new Map();
    const pendingOps = new Map();
    let pendingCallbacks = [];
    let disposed = false;
    let submitted = Promise.resolve();
    const flushWaiters = new Set();
    const rejectFlushes = (error) => {
      for (const waiter of flushWaiters) waiter.reject(error);
      flushWaiters.clear();
    };
    // Writes of one Logic task go together. A GPU View paces its own frames and keeps only
    // the newest state, so they leave when the task ends; a WebView applies each batch to its
    // DOM, so writes within 16 ms of the first unsent one wait and leave together.
    const BATCH_DELAY = gpu ? 0 : 16;

    // A listener that throws is the page's bug: report it, never let it cut
    // the runtime's own teardown short. Aborting twice is a no-op.
    pageSvc._abortLifetime = function () {
      try {
        lifetime.abort();
      } catch (error) {
        console.error("A page's signal abort listener threw:", error);
      }
    };

    pageSvc._cancelPendingSetData = function () {
      disposed = true;
      rejectFlushes(new Error("Page unloaded before its state was flushed"));
      clearTimer(updateTimer);
      updateTimer = null;
      pendingBaseState.clear();
      pendingOps.clear();
      pendingCallbacks = [];
      pageSvc._abortLifetime();
    };

    pageSvc.setData = function (updates) {
      if (disposed) {
        return;
      }
      if (!updates || typeof updates !== "object") {
        throw new Error("setData: Invalid updates");
      }

      assertPageData(updates);
      const self = this;

      try {
        for (const [path, value] of Object.entries(updates)) {
          applyUpdate(self.data, pendingBaseState, pendingOps, path, value);
        }
      } catch (err) {
        throw err;
      }

      if (updateTimer === null) {
        updateTimer = setTimer(submitPending, BATCH_DELAY);
      }
    };

    function submitPending() {
      updateTimer = null;
      if (disposed) {
        return;
      }
      const ops = Array.from(pendingOps.values()).map(toJsonPatchOp);
      const callbacks = pendingCallbacks;

      pendingBaseState.clear();
      pendingOps.clear();
      pendingCallbacks = [];

      // Every flush includes earlier batches, including batches already in flight.
      // A failed batch has already rejected its waiters; later batches stand alone.
      const previous = submitted.catch(() => {});
      // Native reports one of "acked" | "deferred" | "dropped". It calls back on
      // every terminal path, so only the status tells an acknowledgement apart
      // from a write the View will never see.
      const current = ops.length === 0 ? Promise.resolve("acked") : new Promise((resolve, reject) => {
        try {
          // A GPU View walks the ops in place; a WebView takes them as frame text.
          Promise.resolve(pageSvc._setData(gpu ? ops : JSON.stringify(ops), resolve)).catch(reject);
        } catch (error) {
          reject(error);
        }
      });
      submitted = Promise.all([previous, current]).then(([, status]) => {
        callbacks.forEach((cb) => cb(status));
      });
      submitted.catch((error) => {
        rejectFlushes(error);
        console.error("Error in setData:", error);
      });
    }

    pageSvc.flush = function () {
      if (disposed) return Promise.reject(new Error("Cannot flush an unloaded page"));
      return new Promise((resolve, reject) => {
        const waiter = { reject };
        flushWaiters.add(waiter);
        pendingCallbacks.push((status) => {
          flushWaiters.delete(waiter);
          // "deferred" still reaches the View, through the bridge-ready
          // snapshot rather than this patch. "dropped" never does.
          if (status === "dropped") {
            reject(new Error("Page state was discarded before the View received it"));
            return;
          }
          resolve();
        });
        // Submit now rather than when the batch would leave.
        clearTimer(updateTimer);
        submitPending();
      });
    };

    // What the opener's `result` resolves with. It settles once, after
    // `onUnload`: `{ status: 'ok', value }` when a result was set, otherwise
    // `{ status: 'canceled', reason }`.
    let result = null;
    let settleOpener = null;

    pageSvc.setResult = function (value) {
      assertPageData(value);
      result = { value: cloneJsonValue(value) };
    };

    pageSvc.close = function (value) {
      if (arguments.length > 0) pageSvc.setResult(value);
      if (pageSvc.surface) return pageSvc.surface.close();
      pageSvc._leave();
      return Promise.resolve();
    };

    pageSvc._bindResult = function () {
      if (settleOpener) settleOpener({ status: "canceled", reason: "opener" });
      result = null;
      return new Promise((resolve) => {
        settleOpener = resolve;
      });
    };

    // A replacing page takes over the opener and the result it is owed.
    pageSvc._handOver = function (target) {
      const settle = settleOpener;
      settleOpener = null;
      const opener = pageSvc.opener;
      delete pageSvc.opener;
      target._adopt(opener, settle);
    };

    pageSvc._adopt = function (opener, settle) {
      if (settleOpener) settleOpener({ status: "canceled", reason: "opener" });
      result = null;
      settleOpener = settle;
      if (opener) pageSvc.opener = opener;
      else delete pageSvc.opener;
    };

    pageSvc._settleResult = function (reason) {
      const settle = settleOpener;
      settleOpener = null;
      if (!settle) return;
      settle(result ? { status: "ok", value: result.value } : { status: "canceled", reason });
    };

    // `page.update`: the writes its recipe made join the batch.
    pageSvc._applyChanges = function (changes) {
      if (disposed) return;
      for (const [segments, value] of changes) {
        applyUpdateSegments(pageSvc.data, pendingBaseState, pendingOps, segments, value);
      }
      if (changes.length > 0 && updateTimer === null) {
        updateTimer = setTimer(submitPending, BATCH_DELAY);
      }
    };

    pageSvc.setPath = function (path, value) {
      return pageSvc.setData({ [pathSegmentsToKey(path)]: value });
    };

    pageSvc.setDataPath = function (path, value) {
      if (typeof path !== "string" || !path) {
        throw new Error("setDataPath: Invalid path");
      }
      return pageSvc.setData({ [path]: value });
    };

    return pageSvc;
  }

  const PAGE_BRAND = Symbol.for("lingxia.page");
  const PAGE_HOOKS = ["onLoad", "onShow", "onReady", "onHide", "onUnload", "onPullDownRefresh"];
  const DEFINITION_KEYS = new Set(["params", "result", "messages", "data", "setup", "actions", "opens"]);

  // `definePage({ data, setup(page) { return { onLoad, actions } } })`, or
  // `definePage({ data, actions: page => ({ … }) })`. The hooks and actions
  // become the config a `PageSvc` runs, so dispatch is the same as for any
  // page; `page` is what they close over.
  function instantiateDefinedPage(definition, pagePath, pageInstanceId, gpu, identityJson) {
    const def = definition.config;
    for (const key of Object.keys(def)) {
      if (!DEFINITION_KEYS.has(key)) {
        throw new TypeError(`definePage: unknown key '${key}'; a page declares params, result, messages, data, and setup or actions`);
      }
    }
    if (def.setup !== undefined && typeof def.setup !== "function") {
      throw new TypeError("definePage: setup must be a function");
    }
    if (def.setup !== undefined && def.actions !== undefined) {
      throw new TypeError("definePage: give setup or actions, not both; with setup, return actions from it");
    }
    assertPageData(def.data || {});
    const identity = identityJson ? JSON.parse(identityJson) : {};
    const lifetime = new AbortController();
    let svc = null;
    let initial = cloneJsonValue(def.data || {});
    let params = identity.params || {};
    let updating = false;
    let warnedUnloaded = false;

    function live() {
      if (!svc) throw new Error("page: not available until setup has returned");
      return svc;
    }

    function native() {
      return globalThis.__LX_PAGE_NATIVE__;
    }

    const navigationBar = Object.freeze({
      update(patch) {
        return native().navigationBarUpdate(pageInstanceId, patch);
      },
    });

    // Size class only. A watcher starts with the current value and goes with the page.
    const env = Object.freeze({
      get sizeClass() {
        return native().env().sizeClass;
      },
      get canDock() {
        return native().env().canDock;
      },
      watch(callback, options) {
        if (typeof callback !== "function") {
          throw new TypeError("page.env.watch takes a callback");
        }
        const signal = subscriptionSignal(options);
        if (lifetime.signal.aborted || (signal && signal.aborted)) return () => {};
        let stopped = false;
        const off = native().envWatch((context) => {
          const parsed = typeof context === "string" ? JSON.parse(context) : context;
          if (!stopped) callback({ sizeClass: parsed.sizeClass, canDock: parsed.canDock });
        });
        const stop = () => {
          if (stopped) return;
          stopped = true;
          lifetime.signal.removeEventListener("abort", stop);
          if (signal) signal.removeEventListener("abort", stop);
          off();
        };
        lifetime.signal.addEventListener("abort", stop, { once: true });
        if (signal) signal.addEventListener("abort", stop, { once: true });
        const current = native().env();
        callback({ sizeClass: current.sizeClass, canDock: current.canDock });
        return stop;
      },
    });

    const page = Object.freeze({
      get data() {
        return svc ? svc.data : initial;
      },
      update(recipe) {
        if (typeof recipe !== "function") {
          throw new TypeError("page.update takes a recipe: page.update(d => { d.n += 1; })");
        }
        if (updating) throw new Error("page.update cannot be called inside page.update");
        if (lifetime.signal.aborted) {
          if (!warnedUnloaded) {
            warnedUnloaded = true;
            console.warn(`page.update after '${page.name}' unloaded is ignored`);
          }
          return;
        }
        const draft = createDraft(svc ? svc.data : initial);
        updating = true;
        let returned;
        try {
          returned = recipe(draft.root);
        } finally {
          updating = false;
          draft.close();
        }
        // What a recipe returns is ignored (`d => d.list.push(x)`, `d => d.list.pop()`),
        // except a promise (an await that came too late) and a new plain object (a
        // replacement that would be lost). Data is an object, so an array never replaces it.
        if (returned !== null && typeof returned === "object") {
          if (typeof returned.then === "function") {
            throw new TypeError("page.update: the recipe must be synchronous; await first, then update");
          }
          if (!Array.isArray(returned) && !draft.owns(returned)) {
            throw new TypeError("page.update: change the draft in place; a returned object is not applied");
          }
        }
        const changes = draft.changes();
        if (svc) {
          svc._applyChanges(changes);
        } else {
          for (const [segments, value] of changes) setValueBySegments(initial, segments, value);
        }
      },
      flush() {
        return live().flush();
      },
      get params() {
        return params;
      },
      get name() {
        return identity.name || pagePath.replace(/\.[^./]+$/, "");
      },
      signal: lifetime.signal,
      close(value) {
        return arguments.length > 0 ? live().close(value) : live().close();
      },
      setResult(value) {
        live().setResult(value);
      },
      get opener() {
        return svc ? svc.opener : undefined;
      },
      get surface() {
        return svc ? svc.surface : undefined;
      },
      navigationBar,
      refresh() {
        native().startRefresh(pageInstanceId);
      },
      video(name) {
        return native().video(pageInstanceId, name);
      },
      env,
    });

    const returned = def.setup ? def.setup(page)
      : { actions: typeof def.actions === "function" ? def.actions(page) : def.actions };
    if (!def.setup && returned.actions && typeof returned.actions.then === "function") {
      throw new TypeError("definePage: actions(page) returns the actions synchronously; start async work in an action");
    }
    if (returned && typeof returned.then === "function") {
      throw new TypeError("definePage: setup must be synchronous; start async work in onLoad");
    }
    if (returned !== undefined && (returned === null || typeof returned !== "object")) {
      throw new TypeError("definePage: setup returns { onLoad?, …, actions? }");
    }
    const config = { data: initial };
    for (const [key, value] of Object.entries(returned || {})) {
      if (key === "actions") continue;
      if (!PAGE_HOOKS.includes(key)) {
        throw new TypeError(typeof value === "function"
          ? `definePage: '${key}' is not a lifecycle hook; put View-callable functions under actions`
          : `definePage: setup returned an unknown key '${key}'`);
      }
      if (typeof value !== "function") throw new TypeError(`definePage: ${key} must be a function`);
      config[key] = value;
    }
    // The indicator goes away when the hook settles; a page without one
    // still gets it put away.
    const userRefresh = config.onPullDownRefresh;
    config.onPullDownRefresh = function () {
      const stop = () => {
        try {
          native().stopRefresh(pageInstanceId);
        } catch (error) {
          console.warn("Could not stop the refresh indicator:", error);
        }
      };
      let result;
      try {
        result = userRefresh ? userRefresh() : undefined;
      } catch (error) {
        stop();
        throw error;
      }
      Promise.resolve(result).then(stop, stop);
      return result;
    };
    const userOnLoad = config.onLoad;
    config.onLoad = function (options) {
      params = options || {};
      return userOnLoad ? userOnLoad(params) : undefined;
    };
    const actions = (returned && returned.actions) || {};
    if (typeof actions !== "object") {
      throw new TypeError("definePage: actions must be an object of functions");
    }
    config.actions = {};
    for (const [name, fn] of Object.entries(actions)) {
      if (typeof fn !== "function") throw new TypeError(`definePage: action '${name}' must be a function`);
      config.actions[name] = fn;
    }
    config.route = pagePath.replace(/\.[^./]+$/, "");
    svc = createPageInstance(
      { config },
      pagePath,
      pageInstanceId,
      gpu,
      lifetime,
    );
    // What a test's getPage(id) reaches the page-scoped members through.
    Object.defineProperty(svc, "page", { value: page, enumerable: false });
    return svc;
  }

  // Registers a raw config; runtime tests drive PageSvc through it.
  globalThis.__registerPage = function (
    pagePath,
    pageConfig,
    bindingMetaJson,
  ) {
    if (!pagePath) {
      throw new Error(
        "__registerPage() called without page path. This indicates a build configuration issue.",
      );
    }
    pageDefinitions.set(pagePath, {
      config: pageConfig,
      bindingMetaJson: bindingMetaJson || '{"handlers":[]}',
    });
  };

  globalThis.__registerDefinedPage = function (pagePath, definition) {
    if (!definition || definition[PAGE_BRAND] !== true) {
      throw new TypeError(
        `Page '${pagePath}' must default-export definePage({ ... }) from '@lingxia/logic'`,
      );
    }
    pageDefinitions.set(pagePath, { config: definition });
  };

  function removedPageRegistration(symbolName) {
    throw new Error(
      `${symbolName}() was removed: a page's Logic default-exports definePage({ data, setup }) from '@lingxia/logic'.`,
    );
  }

  globalThis.Page = function () {
    removedPageRegistration("Page");
  };

  globalThis.PageInstance = function () {
    removedPageRegistration("PageInstance");
  };

  globalThis.__LX_CREATE_PAGE__ = function (
    pagePath,
    definitionPath,
    pageInstanceId,
    gpu,
    identityJson,
  ) {
    const resolvedDefinitionPath = definitionPath || pagePath;
    const definition = pageDefinitions.get(resolvedDefinitionPath);
    if (!definition) {
      throw new Error(`PageInstance not found: ${resolvedDefinitionPath}`);
    }
    // The route is the configured page path, not the renderer's file.
    if (definition.config[PAGE_BRAND] === true) {
      return instantiateDefinedPage(definition, pagePath, pageInstanceId, gpu === true, identityJson);
    }
    definition.config.route = pagePath.replace(/\.[^./]+$/, "");
    return createPageInstance(definition, pagePath, pageInstanceId, gpu === true);
  };
})();

function applyUpdate(root, pendingBaseState, pendingOps, path, nextValue) {
  applyUpdateSegments(root, pendingBaseState, pendingOps, parseDataPath(path), nextValue);
}

function applyUpdateSegments(root, pendingBaseState, pendingOps, segments, nextValue) {
  captureBaseState(root, pendingBaseState, segments);

  const previous = getValueAtPath(root, segments);
  const pointer = segmentsToJsonPointer(segments);
  const pendingBaseEntry = pendingBaseState.get(pointer);
  const existedBefore =
    pendingBaseEntry && typeof pendingBaseEntry.exists === "boolean"
      ? pendingBaseEntry.exists
      : previous.exists;

  if (nextValue === undefined) {
    if (!previous.exists && !existedBefore) {
      return;
    }
  } else if (previous.exists && isDeepEqual(previous.value, nextValue)) {
    return;
  }

  setValueBySegments(root, segments, nextValue);
  enqueuePendingPatch(root, pendingOps, segments, existedBefore);
}

// `page.update`'s draft reads through to `base`; a write copies only the object it lands in, so
// an update costs what it touches, not the size of the page's data.
function createDraft(base) {
  const states = new WeakMap();
  let closed = false;

  function make(value, parent) {
    const state = { base: value, copy: null, children: new Map(), modified: false, parent, proxy: null };
    const current = () => state.copy || state.base;
    const write = () => {
      if (closed) throw new TypeError("page.update: the draft is only usable inside the recipe");
      if (!state.copy) state.copy = Array.isArray(value) ? value.slice() : Object.assign({}, value);
      for (let s = state; s && !s.modified; s = s.parent) s.modified = true;
      return state.copy;
    };
    const proxy = new Proxy(Array.isArray(value) ? [] : {}, {
      get(_, key) {
        const child = current()[key];
        if (typeof key === "symbol" || child === null || typeof child !== "object" || states.has(child)) return child;
        if (!Object.prototype.hasOwnProperty.call(current(), key)) return child;
        const cached = state.children.get(key);
        if (cached && cached.state.base === child) return cached.proxy;
        const made = make(child, state);
        state.children.set(key, made);
        return made.proxy;
      },
      set(_, key, next) {
        write()[key] = next;
        state.children.delete(key);
        return true;
      },
      deleteProperty(_, key) {
        delete write()[key];
        state.children.delete(key);
        return true;
      },
      defineProperty(_, key, descriptor) {
        if (!("value" in descriptor)) throw new TypeError("Page data cannot contain accessors");
        write()[key] = descriptor.value;
        state.children.delete(key);
        return true;
      },
      has: (_, key) => key in current(),
      ownKeys: () => Reflect.ownKeys(current()),
      getOwnPropertyDescriptor(_, key) {
        const descriptor = Reflect.getOwnPropertyDescriptor(current(), key);
        // An array's `length` is the one property the stand-in target also has.
        if (descriptor && key !== "length") descriptor.configurable = true;
        return descriptor;
      },
    });
    state.proxy = proxy;
    states.set(proxy, state);
    return { state, proxy };
  }

  // A value written into the draft, as page data: drafts inside it read out, nothing shared.
  function plain(value) {
    if (value === null || typeof value !== "object") return value;
    if (Array.isArray(value)) return Array.prototype.map.call(value, plain);
    const out = {};
    for (const key of Object.keys(value)) out[key] = plain(value[key]);
    return out;
  }

  function collect(state, segments, out) {
    if (!state.modified) return;
    const { base, copy } = state;
    if (!copy) {
      for (const [key, child] of state.children) {
        if (child.state.modified) collect(child.state, [...segments, key], out);
      }
      return;
    }
    // A resized array goes whole, read through the draft so its edited items come along.
    if (Array.isArray(base) && copy.length !== base.length) {
      out.push([segments, state.proxy]);
      return;
    }
    const has = (key) => Object.prototype.hasOwnProperty.call(base, key);
    for (const key of Object.keys(base)) {
      if (!Object.prototype.hasOwnProperty.call(copy, key)) out.push([[...segments, key], undefined]);
    }
    for (const key of Object.keys(copy)) {
      const next = copy[key];
      const cached = state.children.get(key);
      const child = cached && cached.state.base === next ? cached : null;
      if (child && has(key) && next === base[key]) {
        collect(child.state, [...segments, key], out);
      } else if (!has(key) || next !== base[key] || (child && child.state.modified)) {
        out.push([[...segments, key], child ? child.proxy : next]);
      }
    }
  }

  const root = make(base, null);
  return {
    root: root.proxy,
    owns: (value) => states.has(value),
    close() {
      closed = true;
    },
    changes() {
      const out = [];
      collect(root.state, [], out);
      for (const change of out) {
        assertPageData(change[1]);
        change[1] = plain(change[1]);
      }
      return out;
    },
  };
}

function pathSegmentsToKey(path) {
  if (!Array.isArray(path) || path.length === 0) {
    throw new Error("setPath: Invalid path");
  }
  let key = "";
  for (const segment of path) {
    if (typeof segment === "number") {
      if (!Number.isInteger(segment) || segment < 0) {
        throw new Error("setPath: Invalid path");
      }
      key += `[${segment}]`;
      continue;
    }
    if (typeof segment !== "string" || segment === "") {
      throw new Error("setPath: Invalid path");
    }
    key = key === "" ? segment : `${key}.${segment}`;
  }
  return key;
}

function parseDataPath(path) {
  if (!path) {
    throw new Error("setData: Invalid path");
  }

  return path.replace(/\[(\d+)\]/g, ".$1").split(".");
}

function getValueAtPath(root, segments) {
  let current = root;

  for (let i = 0; i < segments.length; i++) {
    const key = segments[i];
    if (!current || typeof current !== "object") {
      return { exists: false, value: undefined };
    }

    if (Array.isArray(current)) {
      const index = parseInt(key, 10);
      if (Number.isNaN(index) || index < 0 || index >= current.length) {
        return { exists: false, value: undefined };
      }
      current = current[index];
      continue;
    }

    if (!Object.prototype.hasOwnProperty.call(current, key)) {
      return { exists: false, value: undefined };
    }
    current = current[key];
  }

  return { exists: true, value: current };
}

// The container decides how a key applies; a numeric-looking key in an object is a plain key.
function setValueBySegments(root, segments, value) {
  let current = root;

  for (let i = 0; i < segments.length - 1; i++) {
    const key = segments[i];
    if (current[key] === undefined || current[key] === null) {
      current[key] = /^\d+$/.test(segments[i + 1]) ? [] : {};
    } else if (typeof current[key] !== "object") {
      throw new Error(`Cannot set "${segments.slice(0, i + 2).join(".")}": "${key}" is not an object`);
    }
    current = current[key];
  }

  const finalKey = segments[segments.length - 1];
  if (value === undefined) {
    if (Array.isArray(current)) {
      const index = parseInt(finalKey, 10);
      if (!(index >= 0 && index < current.length)) {
        throw new Error(`Cannot remove item "${finalKey}": the array has ${current.length} items`);
      }
      current.splice(index, 1);
      return;
    }
    delete current[finalKey];
    return;
  }

  current[finalKey] = value;
}

function captureBaseState(root, pendingBaseState, segments) {
  for (let depth = 1; depth <= segments.length; depth++) {
    const partialSegments = segments.slice(0, depth);
    const pointer = segmentsToJsonPointer(partialSegments);
    if (pendingBaseState.has(pointer)) {
      continue;
    }

    const snapshot = getValueAtPath(root, partialSegments);
    pendingBaseState.set(pointer, {
      exists: snapshot.exists,
    });
  }
}

function jsonPointerEscape(seg) {
  return String(seg).replace(/~/g, "~0").replace(/\//g, "~1");
}

function joinJsonPointer(base, seg) {
  const escaped = jsonPointerEscape(seg);
  if (!base) return `/${escaped}`;
  return `${base}/${escaped}`;
}

function segmentsToJsonPointer(segments) {
  let pointer = "";
  for (const seg of segments) {
    pointer = joinJsonPointer(pointer, seg);
  }
  return pointer;
}

function enqueuePendingPatch(root, pendingOps, segments, existedBefore) {
  const existingEntry = pendingOps.get(segmentsToJsonPointer(segments));
  const hadExisted =
    existingEntry && typeof existingEntry.hadExisted === "boolean"
      ? existingEntry.hadExisted
      : existedBefore;

  for (let i = segments.length - 1; i > 0; i--) {
    const ancestorPointer = segmentsToJsonPointer(segments.slice(0, i));
    const ancestorEntry = pendingOps.get(ancestorPointer);
    if (!ancestorEntry) {
      continue;
    }

    const refreshedAncestor = buildPendingPatchEntry(
      root,
      ancestorEntry.segments,
      ancestorEntry.hadExisted,
    );
    if (refreshedAncestor) {
      pendingOps.set(ancestorPointer, refreshedAncestor);
    } else {
      pendingOps.delete(ancestorPointer);
    }
    return;
  }

  for (const [pointer, entry] of pendingOps.entries()) {
    if (
      pointer !== segmentsToJsonPointer(segments) &&
      isDescendantPointer(pointer, segments)
    ) {
      pendingOps.delete(pointer);
    }
  }

  const nextEntry = buildPendingPatchEntry(root, segments, hadExisted);
  if (!nextEntry) {
    pendingOps.delete(segmentsToJsonPointer(segments));
    return;
  }
  pendingOps.set(nextEntry.path, nextEntry);
}

function buildPendingPatchEntry(root, segments, hadExisted) {
  const current = getValueAtPath(root, segments);
  if (!current.exists) {
    if (!hadExisted) {
      return null;
    }
    return {
      path: segmentsToJsonPointer(segments),
      segments: [...segments],
      hadExisted,
      op: "remove",
    };
  }

  return {
    path: segmentsToJsonPointer(segments),
    segments: [...segments],
    hadExisted,
    op: hadExisted ? "replace" : "add",
    value: cloneJsonValue(current.value),
  };
}

function isDescendantPointer(candidatePointer, ancestorSegments) {
  const ancestorPointer = segmentsToJsonPointer(ancestorSegments);
  return candidatePointer.startsWith(`${ancestorPointer}/`);
}

function toJsonPatchOp(entry) {
  if (entry.op === "remove") {
    return {
      op: entry.op,
      path: entry.path,
    };
  }

  return {
    op: entry.op,
    path: entry.path,
    value: entry.value,
  };
}

function cloneJsonValue(value) {
  if (value === undefined) {
    return undefined;
  }

  if (typeof structuredClone === "function") {
    return structuredClone(value);
  }

  return JSON.parse(JSON.stringify(value));
}

function isPlainObject(v) {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}

function isDeepEqual(a, b) {
  if (a === b) {
    return true;
  }

  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) {
      return false;
    }
    for (let i = 0; i < a.length; i++) {
      if (!isDeepEqual(a[i], b[i])) {
        return false;
      }
    }
    return true;
  }

  if (!isPlainObject(a) || !isPlainObject(b)) {
    return false;
  }

  const keysA = Object.keys(a);
  const keysB = Object.keys(b);
  if (keysA.length !== keysB.length) {
    return false;
  }

  for (const key of keysA) {
    if (!Object.prototype.hasOwnProperty.call(b, key)) {
      return false;
    }
    if (!isDeepEqual(a[key], b[key])) {
      return false;
    }
  }

  return true;
}

// Undefined remains the explicit remove operation; other values must survive JSON transport.
function assertPageData(value, seen = new Set()) {
  if (value === null || value === undefined || typeof value === 'string' || typeof value === 'boolean') return;
  if (typeof value === 'number' && Number.isFinite(value)) return;
  if (typeof value !== 'object' || seen.has(value)) throw new TypeError('Page data must be acyclic JSON values');
  const proto = Object.getPrototypeOf(value);
  if (!Array.isArray(value) && proto !== Object.prototype && proto !== null) {
    throw new TypeError('Page data cannot contain class instances');
  }
  if (Object.getOwnPropertySymbols(value).length) throw new TypeError('Page data cannot contain symbol keys');
  seen.add(value);
  for (const item of Object.values(value)) assertPageData(item, seen);
  seen.delete(value);
}

// The `{ signal }` every subscription takes; anything else is refused.
function subscriptionSignal(options) {
  if (options === undefined) return undefined;
  const signal = options !== null && typeof options === "object" ? options.signal : null;
  if (signal === undefined) return undefined;
  if (!signal || typeof signal.addEventListener !== "function") {
    throw new TypeError("a subscription takes { signal } as its options");
  }
  return signal;
}
