let __APP_INSTANCE__ = null;

function registerApp(appConfig, handlerNamesJson) {
  if (!appConfig || typeof appConfig !== "object") {
    throw new Error("App registration requires a configuration object");
  }

  const appInstance = {};
  for (const [key, value] of Object.entries(appConfig)) {
    appInstance[key] = value;
  }
  appInstance.globalData = appConfig.globalData || {};

  new LxAppSvc(appInstance, handlerNamesJson || "[]");
  __APP_INSTANCE__ = appInstance;
  return appInstance;
}

function getApp() {
  if (!__APP_INSTANCE__) {
    console.warn("getApp() called before App() - no app instance available");
    return null;
  }
  return __APP_INSTANCE__;
}

globalThis.__registerApp = registerApp;
globalThis.App = function () {
  throw new Error(
    "App() should be transformed at build time. Rebuild the logic bundle with the LingXia CLI.",
  );
};
globalThis.getApp = getApp;
