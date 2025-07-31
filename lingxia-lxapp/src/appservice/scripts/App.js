/**
 * LingXia LxApp App Service
 * Provides App() function and getApp() global function
 */

// Global app instance storage
let __APP_INSTANCE__ = null;

/**
 * App function - creates and registers the app instance
 * @param {Object} appConfig - App configuration object
 * @returns {Object} App instance
 */
function App(appConfig) {
  if (!appConfig || typeof appConfig !== "object") {
    throw new Error("App() requires a configuration object");
  }

  // Create app instance - just store the config as-is
  const appInstance = {
    ...appConfig,
    globalData: appConfig.globalData || {},
  };

  // Create LxAppSvc for Rust side but store original instance for getApp()
  const miniAppSvc = new LxAppSvc(appInstance);
  __APP_INSTANCE__ = appInstance;

  console.log("📱 App instance created");
  return appInstance;
}

/**
 * getApp function - returns the current app instance
 * @returns {Object|null} Current app instance or null if not created
 */
function getApp() {
  if (!__APP_INSTANCE__) {
    console.warn("getApp() called before App() - no app instance available");
    return null;
  }
  return __APP_INSTANCE__;
}

// Make functions globally available
globalThis.App = App;
globalThis.getApp = getApp;
