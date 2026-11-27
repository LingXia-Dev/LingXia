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
// The host reports here too a promise rejection nothing handled, where the engine tracks them,
// as `{ kind: "app", hook: null }`: the engine's event names no page or action.
globalThis.__LX_REPORT_FAILURE__ = failures;

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
  for (const name of ["onShow", "onHide", "onScreenshot"]) reportingHook(appInstance, name);
  hideLinkFields(appInstance);
  const firstShown = onFirstShow(appInstance);
  decideLaunchDestination(appInstance, appConfig, firstShown);
  if (typeof appConfig.onOpen === "function") routeWarmLinksThroughOnOpen(appInstance, appConfig);

  new LxAppSvc(appInstance, handlerNamesJson || "[]");
  __APP_INSTANCE__ = appInstance;
  return appInstance;
}


function appLink(options) {
  if (typeof options === "string") options = JSON.parse(options);
  if (!options || options.scene !== 8003) return null;
  return { url: options.url || "", path: options.path || "", query: options.query || {} };
}

// `onOpen(link)`, run now so it precedes the `onShow` that follows; a throw or rejection is
// reported. Resolves to its target or nothing.
function callOnOpen(appInstance, appConfig, link) {
  return new Promise((resolve) => resolve(appConfig.onOpen.call(appInstance, link)))
    .catch((error) => {
      failures({ error, source: { kind: "app", hook: "onOpen" } }, null);
      return undefined;
    });
}

function navigateTo(target, hook) {
  const opened = lx.nav.go(target);
  if (opened && opened.opened) opened.opened.catch((error) => console.error(`App.${hook} target failed:`, error));
}

// Settles at the first `onShow`, when the first page is on screen.
function onFirstShow(appInstance) {
  let shown;
  const firstShown = new Promise((resolve) => (shown = resolve));
  const onShow = appInstance.onShow;
  appInstance.onShow = function (...args) {
    shown();
    return onShow ? onShow.apply(appInstance, args) : undefined;
  };
  return firstShown;
}

// `onLaunch` resolves to the launch decision the host waits for until `launchDeadline`: a page
// or tab target is the first page; "start" is the start page, chosen when `onOpen` owns the
// link; `null` keeps the page the host was asked for. The app's `onLaunch({ link })` decides
// first, then a cold link's `onOpen`. Anything else (another app, a URL, an answer after the
// deadline, the link after a throwing `onLaunch`) is navigated to once the first page is up.
function decideLaunchDestination(appInstance, appConfig, firstShown) {
  const onLaunch = appInstance.onLaunch;
  const ownsLinks = typeof appConfig.onOpen === "function";
  appInstance.onLaunch = function (options) {
    if (typeof options === "string") options = JSON.parse(options);
    const deadline = options && typeof options.launchDeadline === "number" ? options.launchDeadline : 0;
    const link = appLink(options);
    const fallback = link && ownsLinks ? "start" : null;
    const settle = (target, hook) => {
      if (!target) return fallback;
      const firstPage = (target.page || target.tab) && !target.app && !target.url;
      if (firstPage && Date.now() <= deadline) return target;
      firstShown.then(() => navigateTo(target, hook));
      return fallback;
    };
    const openLink = () => callOnOpen(appInstance, appConfig, link);
    return Promise.resolve()
      .then(() => (onLaunch ? onLaunch.call(appInstance, Object.freeze({ link })) : undefined))
      .then(
        (target) => {
          if (target || !fallback) return settle(target, "onLaunch");
          return openLink().then((opened) => settle(opened, "onOpen"));
        },
        (error) => {
          failures({ error, source: { kind: "app", hook: "onLaunch" } }, null);
          if (fallback) openLink().then((opened) => opened && firstShown.then(() => navigateTo(opened, "onOpen")));
          return fallback;
        },
      );
  };
}

// A warm App Link reaches `onOpen` before `onShow`; a returned target is navigated to.
function routeWarmLinksThroughOnOpen(appInstance, appConfig) {
  const onShow = appInstance.onShow;
  appInstance.onShow = function (options) {
    const link = appLink(options);
    if (link) {
      callOnOpen(appInstance, appConfig, link).then((target) => {
        if (target) navigateTo(target, "onOpen");
      });
    }
    return onShow ? onShow.call(appInstance, options) : undefined;
  };
}

// The link fields feed `onOpen` and `onLaunch({ link })`; `onShow` sees where it lands, not how
// the link was delivered.
function hideLinkFields(appInstance) {
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
