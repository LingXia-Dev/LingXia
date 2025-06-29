/**
 * LingXia Bridge integration for Vue 3, React, and native JavaScript
 */
(function () {
  "use strict";

  function initFramework() {
    if (!window.LingXiaBridge || !window.lxSmartMethods) {
      console.error("LingXia Bridge not found");
      return;
    }

    window.useLingXia = function (options = {}) {
      const framework = detectFramework();
      const { data, connected, cleanup } = createFrameworkBinding(
        framework,
        options,
      );

      // Create framework helper object
      const frameworkHelper = {
        type: framework,
        getPageFunctions: () => window.LingXiaBridge.getPageFunctions(),
        isPageFunction: (methodName) =>
          window.LingXiaBridge.isPageFunction(methodName),
      };

      // Create a proxy that combines data access with smart method calls
      const result = new Proxy(
        {
          data,
          connected,
          lx: window.lx,
          getPageFunctions: () => window.LingXiaBridge.getPageFunctions(),
          isPageFunction: (methodName) =>
            window.LingXiaBridge.isPageFunction(methodName),
          _cleanup: cleanup,
          _framework: frameworkHelper,
        },
        {
          get(target, prop) {
            // Check if it's a built-in property first
            if (prop in target) {
              return target[prop];
            }

            // Check if it's a smart method
            if (window.lxSmartMethods && window.lxSmartMethods[prop]) {
              return window.lxSmartMethods[prop];
            }

            return undefined;
          },

          has(target, prop) {
            return (
              prop in target ||
              (window.lxSmartMethods && prop in window.lxSmartMethods)
            );
          },

          ownKeys(target) {
            const baseKeys = Object.keys(target);
            const smartKeys = window.lxSmartMethods
              ? Object.keys(window.lxSmartMethods)
              : [];
            return [...new Set([...baseKeys, ...smartKeys])];
          },
        },
      );

      return result;
    };
  }

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

    return "native";
  }

  function createFrameworkBinding(framework, options) {
    switch (framework) {
      case "vue3":
        return createVue3Binding(options);
      case "react":
        return createReactBinding(options);
      case "native":
      default:
        return createNativeBinding(options);
    }
  }

  function createVue3Binding(options) {
    if (!Vue || !Vue.reactive || !Vue.ref) {
      console.warn(
        "Vue 3 reactive API not found, falling back to native binding",
      );
      return createNativeBinding(options);
    }

    const { reactive, ref, onUnmounted } = Vue;

    const data = reactive({});
    const connected = ref(!!window.LingXiaBridge);

    const unsubscribe = window.LingXiaBridge.subscribe(
      (newData, callbackId, isInitialData) => {
        const updateData = options.namespace
          ? newData[options.namespace] || {}
          : newData;

        if (isInitialData) {
          Object.keys(data).forEach((key) => delete data[key]);
          Object.assign(data, updateData);
        } else {
          Object.assign(data, updateData);
        }

        connected.value = true;
      },
    );

    const cleanup = () => {
      unsubscribe?.();
    };

    if (onUnmounted) {
      onUnmounted(cleanup);
    }

    return { data, connected, cleanup };
  }

  function createReactBinding(options) {
    if (!React || !React.useState || !React.useEffect) {
      console.warn("React hooks not found, falling back to native binding");
      return createNativeBinding(options);
    }

    const { useState, useEffect, useRef } = React;

    const [data, setData] = useState({});
    const [connected, setConnected] = useState(!!window.LingXiaBridge);
    const unsubscribeRef = useRef(null);

    if (useEffect) {
      useEffect(() => {
        unsubscribeRef.current = window.LingXiaBridge.subscribe(
          (newData, callbackId, isInitialData) => {
            const updateData = options.namespace
              ? newData[options.namespace] || {}
              : newData;

            if (isInitialData) {
              setData({ ...updateData });
            } else {
              setData((prevData) => ({ ...prevData, ...updateData }));
            }

            setConnected(true);
          },
        );

        return () => unsubscribeRef.current?.();
      }, []);
    }

    const cleanup = () => {
      unsubscribeRef.current?.();
    };

    return { data, connected, cleanup };
  }

  function createNativeBinding(options) {
    let data = {};
    let connected = !!window.LingXiaBridge;
    const listeners = new Set();

    const reactiveData = new Proxy(data, {
      set(target, prop, value) {
        target[prop] = value;
        listeners.forEach((listener) => {
          try {
            listener(target);
          } catch (e) {
            console.warn("Data listener error:", e);
          }
        });
        return true;
      },

      get(target, prop) {
        return target[prop];
      },
    });

    const unsubscribe = window.LingXiaBridge.subscribe(
      (newData, callbackId, isInitialData) => {
        const updateData = options.namespace
          ? newData[options.namespace] || {}
          : newData;

        if (isInitialData) {
          Object.keys(data).forEach((key) => delete data[key]);
          Object.assign(data, updateData);
        } else {
          Object.assign(data, updateData);
        }

        connected = true;
        listeners.forEach((listener) => {
          try {
            listener(data);
          } catch (e) {
            console.warn("Data listener error:", e);
          }
        });
      },
    );

    const cleanup = () => {
      unsubscribe?.();
      listeners.clear();
    };

    reactiveData._addListener = (listener) => {
      if (typeof listener === "function") {
        listeners.add(listener);
      }
    };

    reactiveData._removeListener = (listener) => {
      listeners.delete(listener);
    };

    return {
      data: reactiveData,
      connected: { value: connected },
      cleanup,
    };
  }

  // Vue 3 Composable API
  if (typeof Vue !== "undefined") {
    window.useLingXiaComposable = function (options = {}) {
      const result = window.useLingXia(options);

      if (Vue.version && Vue.version.startsWith("3")) {
        const { toRefs } = Vue;
        if (toRefs && typeof result.data === "object") {
          return {
            ...toRefs(result.data),
            connected: result.connected,
            lx: result.lx,
            ...result,
          };
        }
      }

      return result;
    };
  }

  // Initialize when bridge is ready
  if (window.LingXiaBridge && window.lxSmartMethods) {
    initFramework();
  } else {
    window.addEventListener("LingXiaBridgeReady", initFramework, {
      once: true,
    });
  }
})();

