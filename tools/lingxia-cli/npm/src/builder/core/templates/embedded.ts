/**
 * Embedded builder templates for React and Vue frameworks.
 * These are internal templates used during the build process, not user-facing.
 */

export const REACT_INDEX_HTML = `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no" />
    <title>LingXia React Page</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="./main.jsx"></script>
  </body>
</html>`;

export const REACT_MAIN_JSX = `import React from 'react'
import ReactDOM from 'react-dom/client'
/* {{APP_IMPORT}} */

// Singleton data store + listeners
let __lxData = {};
let __lxSubscribed = false;
const __lxListeners = new Set();

// Deep merge for nested objects
function __deepMerge(target, source) {
  if (!source || typeof source !== 'object') return target;
  const result = { ...target };
  for (const key in source) {
    const sv = source[key], tv = target[key];
    if (sv && typeof sv === 'object' && !Array.isArray(sv)) {
      result[key] = __deepMerge(tv || {}, sv);
    } else {
      result[key] = sv;
    }
  }
  return result;
}

// Register bridge subscription with retry
(function registerBridge() {
  if (__lxSubscribed) return;
  if (!window.LingXiaBridge?.subscribe) {
    setTimeout(registerBridge, 10);
    return;
  }
  __lxSubscribed = true;
  window.LingXiaBridge.subscribe((d) => {
    if (d) {
      __lxData = __deepMerge(__lxData, d);
      __lxListeners.forEach(fn => fn(__lxData));
    }
  });
})();

window.useLingXia = function () {
  const [data, setData] = React.useState(__lxData);

  React.useEffect(() => {
    const listener = (newData) => setData(newData);
    __lxListeners.add(listener);
    setData(__lxData); // Sync initial data
    return () => __lxListeners.delete(listener); // Cleanup
  }, []);

  const fns = React.useMemo(() => {
    const obj = {};
    window.__PAGE_FUNCTIONS?.forEach(n => { obj[n] = window[n]; });
    return obj;
  }, []);

  return { data, ...fns };
};

// Page functions injection
/* {{PAGE_FUNCTIONS}} */

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)`;

export const VUE_INDEX_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
  <title>LingXia Vue Page</title>
</head>
<body>
  <div id="app"></div>
  <script>
    // Initialize globals before module loads
    window.__lxData = {};
    window.__lxSubscribed = false;
    window.__PAGE_FUNCTIONS = [];
  </script>
  <script type="module" src="./main.js"></script>
</body>
</html>`;

export const VUE_MAIN_JS = `import { createApp, reactive } from "vue";
/* {{APP_IMPORT}} */

// Vue reactive data store
const __lxReactiveData = reactive(window.__lxData || {});
window.__lxData = __lxReactiveData;

// Deep merge for Vue reactivity
function __deepMerge(target, source) {
  if (!source || typeof source !== 'object') return;
  for (const key in source) {
    const sv = source[key], tv = target[key];
    if (sv && typeof sv === 'object' && !Array.isArray(sv)) {
      if (!tv || typeof tv !== 'object') target[key] = {};
      __deepMerge(target[key], sv);
    } else {
      target[key] = sv;
    }
  }
}

// Register bridge subscription with retry
(function registerBridge() {
  if (window.__lxSubscribed) return;
  if (!window.LingXiaBridge?.subscribe) {
    setTimeout(registerBridge, 10);
    return;
  }
  window.__lxSubscribed = true;
  window.LingXiaBridge.subscribe((d) => d && __deepMerge(__lxReactiveData, d));
})();

// useLingXia hook
window.useLingXia = function () {
  const fns = {};
  window.__PAGE_FUNCTIONS?.forEach((n) => { fns[n] = window[n]; });
  return { data: __lxReactiveData, ...fns };
};

// Page functions injection
/* {{PAGE_FUNCTIONS}} */

// Create and configure Vue app
const app = createApp(App);

// Register page functions to Vue global properties (before mount)
if (window.__PAGE_FUNCTIONS) {
  window.__PAGE_FUNCTIONS.forEach((funcName) => {
    app.config.globalProperties[funcName] = window[funcName];
  });
}

app.mount("#app");
`;

export interface FrameworkTemplates {
  indexHtml: string;
  mainEntry: string;
  mainEntryFilename: string;
}

export const FRAMEWORK_TEMPLATES: Record<string, FrameworkTemplates> = {
  react: {
    indexHtml: REACT_INDEX_HTML,
    mainEntry: REACT_MAIN_JSX,
    mainEntryFilename: 'main.jsx',
  },
  vue: {
    indexHtml: VUE_INDEX_HTML,
    mainEntry: VUE_MAIN_JS,
    mainEntryFilename: 'main.js',
  },
};

export function getFrameworkTemplates(framework: string): FrameworkTemplates | undefined {
  return FRAMEWORK_TEMPLATES[framework];
}

export function hasFrameworkTemplates(framework: string): boolean {
  return framework in FRAMEWORK_TEMPLATES;
}
