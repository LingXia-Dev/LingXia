// Enhanced Page function with unified logic.js support:
// - Path syntax support for setData
// - Debouncing for performance optimization
// - Diff/Patch generation for optimized data transfer
// - Page registration system for unified loading

(function () {
  // Page configuration registry for unified logic.js
  const __PAGE_REGISTRY__ = {};

  // Core Page instance creation function
  function createPageInstance(pageConfig, pagePath) {
    if (!pageConfig || typeof pageConfig !== "object") {
      throw new Error("setData: Invalid page configuration");
    }

    const pageSvc = new PageSvc(pageConfig, pagePath);

    // Initialize data
    pageSvc.data = JSON.parse(JSON.stringify(pageConfig.data || {}));
    pageSvc._lastData = JSON.parse(JSON.stringify(pageSvc.data));

    // Copy all page properties to pageSvc instance so they can access each other via this
    for (const [key, value] of Object.entries(pageConfig)) {
      if (key !== "data") {
        // Skip private properties and data (already handled)
        if (typeof value === "function") {
          pageSvc[key] = value.bind(pageSvc);
        } else {
          pageSvc[key] = value; // Copy non-function properties like STORAGE_KEYS
        }
      }
    }

    // Setup state management
    let updateTimer = null;
    let pendingData = null;
    let pendingResolvers = [];
    const DEBOUNCE_WAIT = 16; // 16ms ≈ 60fps, better for UI updates

    // Enhanced setData with debouncing and diff optimization
    pageSvc.setData = async function (updates, callback) {
      if (!updates || typeof updates !== "object") {
        throw new Error("setData: Invalid updates");
      }

      return new Promise((resolve, reject) => {
        pendingData = pendingData ? { ...pendingData, ...updates } : updates;
        pendingResolvers.push({ resolve, reject, callback });

        const self = this;

        clearTimeout(updateTimer);
        updateTimer = setTimeout(async () => {
          try {
            const currentUpdates = pendingData;
            const resolvers = pendingResolvers;

            // Reset state
            pendingData = null;
            pendingResolvers = [];

            // Apply updates to local data
            for (const [path, value] of Object.entries(currentUpdates)) {
              setValueByPath(self.data, path, value);
            }

            // Generate and send patch if needed
            const patch = diff(self._lastData, self.data);
            if (Object.keys(patch).length > 0) {
              const callbacks = resolvers
                .filter((r) => r.callback)
                .map((r) => r.callback);

              if (callbacks.length > 0) {
                const combinedCallback =
                  callbacks.length === 1
                    ? callbacks[0]
                    : () => callbacks.forEach((cb) => cb());
                await self._setData(JSON.stringify(patch), combinedCallback);
              } else {
                await self._setData(JSON.stringify(patch));
              }

              self._lastData = JSON.parse(JSON.stringify(self.data));
            } else {
              // Execute callbacks even if no patch
              resolvers.forEach((r) => r.callback && r.callback());
            }

            // Resolve all pending promises
            resolvers.forEach(({ resolve }) => resolve());
          } catch (err) {
            console.error("Error in setData:", err);
            resolvers.forEach(({ reject }) => reject(err));
          }
        }, DEBOUNCE_WAIT);
      });
    };

    return pageSvc;
  }

  // Enhanced Page function with registration support
  globalThis.Page = function (pageConfig, pagePath) {
    if (pagePath) {
      // Register page configuration for unified logic.js
      __PAGE_REGISTRY__[pagePath] = pageConfig;
    } else {
      // This should not happen in production builds
      throw new Error(
        "Page() called without path parameter. This indicates a build configuration issue.",
      );
    }
  };

  // Page instance creation function (called by Rust)
  globalThis.__CREATE_PAGE__ = function (pagePath) {
    const pageConfig = __PAGE_REGISTRY__[pagePath];
    if (pageConfig) {
      return createPageInstance(pageConfig, pagePath);
    } else {
      throw new Error(`Page not found: ${pagePath}`);
    }
  };
})();

// Sets a value in an object using a path string (e.g., 'a.b[0].c')
function setValueByPath(obj, path, value) {
  if (!path) throw new Error("setData: Invalid path");

  const parts = path.replace(/\[(\d+)\]/g, ".$1").split(".");
  let current = obj;

  for (let i = 0; i < parts.length - 1; i++) {
    const key = parts[i];
    const nextKey = parts[i + 1];
    const isNextKeyArrayIndex = /^\d+$/.test(nextKey);

    if (current[key] === undefined || current[key] === null) {
      current[key] = isNextKeyArrayIndex ? [] : {};
    } else if (typeof current[key] !== "object") {
      throw new Error(
        `setData: Cannot set path "${key}", parent is not an object`,
      );
    } else if (isNextKeyArrayIndex && !Array.isArray(current[key])) {
      throw new Error(
        `setData: Cannot set array index on non-array at "${key}"`,
      );
    }

    current = current[key];
    if (!current || typeof current !== "object") {
      throw new Error(`setData: Invalid path segment "${key}"`);
    }
  }

  const finalKey = parts[parts.length - 1];
  if (value === undefined) {
    if (Array.isArray(current)) {
      const index = parseInt(finalKey, 10);
      if (index >= 0 && index < current.length) {
        delete current[finalKey];
      } else {
        throw new Error(
          `setData: Invalid array index "${finalKey}" for deletion`,
        );
      }
    } else if (current && typeof current === "object") {
      delete current[finalKey];
    } else {
      throw new Error(`setData: Cannot delete property "${finalKey}"`);
    }
  } else {
    current[finalKey] = value;
  }
}

// Generates minimal diff between old and new values
function diff(oldValue, newValue, currentPath = "", patch = {}) {
  if (oldValue === newValue) return patch;

  if (
    !newValue ||
    typeof newValue !== "object" ||
    !oldValue ||
    typeof oldValue !== "object" ||
    Array.isArray(newValue) !== Array.isArray(oldValue)
  ) {
    patch[currentPath] = newValue;
    return patch;
  }

  if (Array.isArray(newValue)) {
    if (
      oldValue.length !== newValue.length ||
      JSON.stringify(oldValue) !== JSON.stringify(newValue)
    ) {
      patch[currentPath] = newValue;
    }
    return patch;
  }

  const oldKeys = new Set(Object.keys(oldValue));
  const newKeys = new Set(Object.keys(newValue));

  for (const key of newKeys) {
    const newPath = currentPath ? `${currentPath}.${key}` : key;
    if (!oldKeys.has(key)) {
      patch[newPath] = newValue[key];
    } else {
      diff(oldValue[key], newValue[key], newPath, patch);
    }
  }

  for (const key of oldKeys) {
    if (!newKeys.has(key)) {
      const deletedPath = currentPath ? `${currentPath}.${key}` : key;
      patch[deletedPath] = undefined;
    }
  }

  return patch;
}
