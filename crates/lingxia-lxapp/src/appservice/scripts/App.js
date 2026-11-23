let __APP_INSTANCE__ = null;

// Unhandled failures reach `App.onError`, one call at a time, after the failing stack has unwound.
// The same failure (page instance, source, message) within 1 s is counted, not delivered, and the
// whole app gets at most 20 calls per 10 s; what was held back rides on the next call's `repeated`.
const failures = (() => {
  const MERGE_MS = 1000;
  const CAP = 20;
  const CAP_MS = 10000;
  const recent = new Map();
  let windowStart = 0;
  let windowCalls = 0;
  let heldBack = 0;
  let chain = Promise.resolve();

  function asError(value) {
    if (value instanceof Error) return value;
    return new Error(typeof value === "string" ? value : String(value), { cause: value });
  }

  function sourceKey(source) {
    return [source.kind, source.hook, source.action, source.prop].join(":");
  }

  function deliver(failure) {
    const onError = __APP_INSTANCE__ && __APP_INSTANCE__.onError;
    if (typeof onError !== "function") {
      console.error("Unhandled failure:", failure.error, failure.source);
      return;
    }
    chain = chain.then(() => Promise.resolve()
      .then(() => onError.call(__APP_INSTANCE__, failure))
      .catch((error) => console.error("App.onError threw; its own failure is not reported again:", error)));
  }

  return function report(raw, page) {
    const failure = {
      error: asError(raw.error),
      page: page ? { name: page.name, instance: page.instance } : null,
      source: raw.source,
      repeated: 0,
    };
    const now = Date.now();
    const key = `${failure.page ? failure.page.instance : ""}|${sourceKey(failure.source)}|${failure.error.message}`;
    const seen = recent.get(key);
    if (seen && now - seen.at < MERGE_MS) {
      seen.count += 1;
      return;
    }
    failure.repeated = seen ? seen.count : 0;
    recent.set(key, { at: now, count: 0 });
    if (now - windowStart >= CAP_MS) {
      windowStart = now;
      windowCalls = 0;
    }
    if (windowCalls >= CAP) {
      heldBack += 1 + failure.repeated;
      return;
    }
    windowCalls += 1;
    failure.repeated += heldBack;
    heldBack = 0;
    deliver(Object.freeze(failure));
  };
})();
globalThis.__LX_REPORT_FAILURE__ = failures;

// An engine that tracks unhandled rejections hands them over here; without one, the `action`
// and module-level kinds never fire.
if (typeof globalThis.__LX_TRACK_UNHANDLED__ === "function") {
  globalThis.__LX_TRACK_UNHANDLED__((reason, page, action) => failures(
    { error: reason, source: action ? { kind: "action", action } : { kind: "app", hook: null } },
    page,
  ));
}

// A thrown or rejected App hook is reported, then fails as before.
function reportingHook(appInstance, name) {
  const hook = appInstance[name];
  if (typeof hook !== "function") return;
  appInstance[name] = function (...args) {
    const failed = (error) => failures({ error, source: { kind: "app", hook: name } }, null);
    let result;
    try {
      result = hook.apply(this, args);
    } catch (error) {
      failed(error);
      throw error;
    }
    if (result && typeof result.then === "function") {
      return result.then(undefined, (error) => {
        failed(error);
        throw error;
      });
    }
    return result;
  };
}

function registerApp(appConfig, handlerNamesJson) {
  if (!appConfig || typeof appConfig !== "object") {
    throw new Error("App registration requires a configuration object");
  }

  if ("globalData" in appConfig) {
    throw new TypeError("App: globalData is gone; keep app state in a module the pages import");
  }
  const appInstance = {};
  for (const [key, value] of Object.entries(appConfig)) {
    appInstance[key] = value;
  }

  if ("onError" in appConfig && typeof appConfig.onError !== "function") {
    throw new TypeError("App: onError is a function: onError(failure) { … }");
  }
  for (const name of ["onLaunch", "onShow", "onHide", "onScreenshot"]) reportingHook(appInstance, name);
  hideLinkFields(appInstance);
  if (typeof appConfig.onOpen === "function") routeLinksThroughOnOpen(appInstance, appConfig);

  new LxAppSvc(appInstance, handlerNamesJson || "[]");
  __APP_INSTANCE__ = appInstance;
  return appInstance;
}


// An App Link (host scene 8003) reaches `onOpen`, cold after `onLaunch`
// settles and warm before `onShow`; a returned target is navigated to.
function routeLinksThroughOnOpen(appInstance, appConfig) {
  const open = (options) => {
    if (typeof options === "string") options = JSON.parse(options);
    if (!options || options.scene !== 8003) return;
    const link = { url: options.url || "", path: options.path || "", query: options.query || {} };
    Promise.resolve()
      .then(() => appConfig.onOpen.call(appInstance, link))
      .then((target) => {
        if (target) {
          const opened = lx.nav.go(target);
          if (opened && opened.opened) opened.opened.catch((error) => console.error("App.onOpen target failed:", error));
          return opened && typeof opened.then === "function" ? opened : undefined;
        }
      })
      .catch((error) => failures({ error, source: { kind: "app", hook: "onOpen" } }, null));
  };
  const onLaunch = appInstance.onLaunch;
  appInstance.onLaunch = function (options) {
    const result = onLaunch ? onLaunch.call(appInstance, options) : undefined;
    Promise.resolve(result).then(() => open(options), () => {});
    return result;
  };
  const onShow = appInstance.onShow;
  appInstance.onShow = function (options) {
    open(options);
    return onShow ? onShow.call(appInstance, options) : undefined;
  };
}

// The link fields feed `onOpen` only; `onLaunch` takes nothing and `onShow`
// sees where it lands, not how the link was delivered.
function hideLinkFields(appInstance) {
  const onLaunch = appInstance.onLaunch;
  if (typeof onLaunch === "function") {
    appInstance.onLaunch = function () {
      return onLaunch.call(appInstance);
    };
  }
  const onShow = appInstance.onShow;
  if (typeof onShow === "function") {
    appInstance.onShow = function (options) {
      if (typeof options === "string") options = JSON.parse(options);
      if (options && typeof options === "object") {
        const { scene: _scene, url: _url, ...shown } = options;
        options = shown;
      }
      return onShow.call(appInstance, options);
    };
  }
}

globalThis.__registerApp = registerApp;
globalThis.App = function () {
  throw new Error(
    "App() should be transformed at build time. Rebuild the logic bundle with the LingXia CLI.",
  );
};
// For `lxdev test` only; app code shares state through modules.
Object.defineProperty(globalThis, "__lxAppInstance", {
  value: () => __APP_INSTANCE__,
  configurable: true,
});
