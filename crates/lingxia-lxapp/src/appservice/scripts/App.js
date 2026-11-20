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

  new LxAppSvc(appInstance, handlerNamesJson || "[]");
  __APP_INSTANCE__ = appInstance;
  return appInstance;
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
