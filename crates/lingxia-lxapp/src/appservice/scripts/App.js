let __APP_INSTANCE__ = null;

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

  if (typeof appConfig.onOpen === "function") routeLinksThroughOnOpen(appInstance, appConfig);

  new LxAppSvc(appInstance, handlerNamesJson || "[]");
  __APP_INSTANCE__ = appInstance;
  return appInstance;
}


// An App Link (`scene: 8003`) reaches `onOpen`, cold after `onLaunch`
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
      .catch((error) => console.error("App.onOpen failed:", error));
  };
  const onLaunch = appConfig.onLaunch;
  appInstance.onLaunch = function (options) {
    const result = onLaunch ? onLaunch.call(appInstance, options) : undefined;
    Promise.resolve(result).then(() => open(options), () => {});
    return result;
  };
  const onShow = appConfig.onShow;
  appInstance.onShow = function (options) {
    open(options);
    return onShow ? onShow.call(appInstance, options) : undefined;
  };
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
