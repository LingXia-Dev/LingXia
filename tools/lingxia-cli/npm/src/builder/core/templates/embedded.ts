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

window.useLingXia = function () {
  const [data, setData] = React.useState({});

  React.useEffect(() => {
    if (window.LingXiaBridge && window.LingXiaBridge.subscribe) {
      window.LingXiaBridge.subscribe((newData) => {
        if (newData) {
          setData(prevData => ({ ...prevData, ...newData }));
        }
      });
    }
  }, []);

  // Create functions object from page functions
  const functions = React.useMemo(() => {
    if (!window.__PAGE_FUNCTIONS) return {};

    return window.__PAGE_FUNCTIONS.reduce((acc, funcName) => {
      acc[funcName] = window[funcName];
      return acc;
    }, {});
  }, []);

  // Return both data and functions
  return {
    data,
    ...functions
  };
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
  <script type="module" src="./main.js"></script>
</body>
</html>`;

export const VUE_MAIN_JS = `import { createApp, ref } from "vue";
/* {{APP_IMPORT}} */

window.useLingXia = function () {
  const dataInstance = ref({});

  if (window.LingXiaBridge && window.LingXiaBridge.subscribe) {
    window.LingXiaBridge.subscribe((newData) => {
      if (newData && dataInstance.value) {
        Object.assign(dataInstance.value, newData);
      }
    });
  }

  // Create functions object from page functions (same as React)
  const functions = {};
  if (window.__PAGE_FUNCTIONS) {
    window.__PAGE_FUNCTIONS.forEach((funcName) => {
      functions[funcName] = window[funcName];
    });
  }

  // Return both data and functions
  return {
    data: dataInstance,
    ...functions
  };
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
