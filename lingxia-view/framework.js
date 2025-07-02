/**
 * LingXia Framework Integration - Universal multi-framework support
 * Supports Vue 3, React, and Vanilla JS with unified API
 */
(function () {
  "use strict";

  function detectFramework() {
    if (
      typeof Vue !== "undefined" &&
      Vue.version &&
      Vue.version.startsWith("3")
    ) {
      return "vue3";
    }
    if (typeof React !== "undefined" && React.useState) {
      return "react";
    }
    return "vanilla";
  }

  /**
   * Framework Adapters - Unified interface for different frameworks
   */
  const FrameworkAdapters = {
    vue3: {
      createReactiveData: () => Vue.ref({}),

      updateData: (dataInstance, newData) => {
        Object.assign(dataInstance.value, newData);
      },

      registerPageFunctions: (functionNames, bridge) => {
        const appElement = document.querySelector("#app");
        if (!appElement || !appElement.__vue_app__) {
          console.warn("[LingXia] Vue app not found");
          return false;
        }

        const app = appElement.__vue_app__;
        if (!app.config || !app.config.globalProperties) {
          console.warn("[LingXia] Vue app config not available");
          return false;
        }

        functionNames.forEach((funcName) => {
          app.config.globalProperties[funcName] = function (...args) {
            return bridge.call(funcName, ...args);
          };
        });

        console.log(
          "[LingXia] Registered page functions to Vue:",
          functionNames,
        );
        return true;
      },
    },

    react: {
      createReactiveData: () => {
        // React uses useState hooks for reactivity
        // Return a proxy that will be used in components
        const reactiveData = { value: {} };

        // Store reference for updates
        reactiveData._updateCallbacks = new Set();

        return reactiveData;
      },

      updateData: (dataInstance, newData) => {
        // Update the data and notify React components
        Object.assign(dataInstance.value, newData);

        // Trigger React re-renders
        dataInstance._updateCallbacks.forEach((callback) => {
          try {
            callback(dataInstance.value);
          } catch (e) {
            console.error("[LingXia] React update callback error:", e);
          }
        });
      },

      registerPageFunctions: (functionNames, bridge) => {
        // For React, functions are accessed directly from window
        return true;
      },
    },

    vanilla: {
      createReactiveData: () => ({ _listeners: [] }),

      updateData: (dataInstance, newData) => {
        Object.assign(dataInstance, newData);
        // Trigger manual listeners
        if (dataInstance._listeners) {
          dataInstance._listeners.forEach((fn) => fn(dataInstance));
        }
      },

      registerPageFunctions: (functionNames, bridge) => {
        // do nothing
        return true;
      },
    },
  };

  // Global data instances registry
  let dataInstances = [];

  /**
   * LingXia Data Hook - Universal multi-framework reactive data
   *
   * Automatically detects framework and provides appropriate reactive data.
   * Sets up unified event-driven initialization.
   *
   * @returns {Object} Framework-appropriate reactive data
   *
   * @example
   * // Vue 3: returns Vue.ref({})
   * const data = useLingXiaData();
   * // Template: <div>{{ data.todos.length }}</div>
   *
   * @example
   * // React: returns reactive object (future implementation)
   * const data = useLingXiaData();
   * // JSX: <div>{data.todos.length}</div>
   *
   * @example
   * // Vanilla JS: returns plain object with onChange
   * const data = useLingXiaData();
   * data.onChange((newData) => console.log('Updated:', newData));
   */
  window.useLingXiaData = function () {
    const framework = detectFramework();
    const adapter = FrameworkAdapters[framework];

    // Setup framework-specific initialization only for Vue and React
    if (framework !== "vanilla") {
      setupUnifiedInitialization();
    }

    const dataInstance = adapter.createReactiveData();

    // Store reference for later updates
    dataInstances.push({ framework, adapter, dataInstance });

    // Add onChange method for vanilla JS
    if (framework === "vanilla") {
      dataInstance.onChange = function (fn) {
        if (!dataInstance._listeners) dataInstance._listeners = [];
        dataInstance._listeners.push(fn);
      };
    }

    return dataInstance;
  };

  /**
   * Unified event-driven initialization system
   * Handles both data subscription and page function registration
   */
  let initializationSetup = false;
  function setupUnifiedInitialization() {
    if (initializationSetup) return;
    initializationSetup = true;

    // Listen for lingxia:init event from Bridge
    window.addEventListener("lingxia:init", function (event) {
      const { functions, bridge } = event.detail;

      if (functions && functions.length > 0) {
        const framework = detectFramework();
        const adapter = FrameworkAdapters[framework];
        adapter.registerPageFunctions(functions, bridge);
      }

      setupUnifiedDataSubscription(bridge);

      console.log(
        "[LingXia] Framework fully initialized via lingxia:init event",
      );
    });
  }

  /**
   * Set up unified data subscription with framework adapters
   */
  function setupUnifiedDataSubscription(bridge) {
    if (!bridge || !bridge.subscribe) return;

    bridge.subscribe((newData) => {
      if (!newData || dataInstances.length === 0) return;

      // Update all data instances using their respective adapters
      dataInstances.forEach(({ adapter, dataInstance }) => {
        adapter.updateData(dataInstance, newData);
      });
    });
  }

  console.log("[LingXia] Framework loaded, useLingXiaData available");
})();
