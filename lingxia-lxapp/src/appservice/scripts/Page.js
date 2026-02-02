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
    let pendingCallbacks = [];
    const DEBOUNCE_WAIT = 16; // 16ms ≈ 60fps, better for UI updates

    // Enhanced setData with debouncing and diff optimization
    pageSvc.setData = function (updates, callback) {
      if (!updates || typeof updates !== "object") {
        throw new Error("setData: Invalid updates");
      }

      const self = this;

      // Keep `this.data` up-to-date immediately, but still debounce
      // the expensive diff/patch generation and bridge transfer for performance.
      try {
        for (const [path, value] of Object.entries(updates)) {
          setValueByPath(self.data, path, value);
        }
      } catch (err) {
        console.error("Error in setData:", err);
        return;
      }

      pendingData = pendingData ? { ...pendingData, ...updates } : updates;
      if (typeof callback === "function") {
        pendingCallbacks.push(callback);
      }

      clearTimeout(updateTimer);
      updateTimer = setTimeout(() => {
        const currentUpdates = pendingData;
        const callbacks = pendingCallbacks;

        // Reset state for next batch
        pendingData = null;
        pendingCallbacks = [];

        try {
          if (!currentUpdates) {
            return;
          }

          // Generate and send patch if needed
          const ops = diffToJsonPatchOps(self._lastData, self.data);
          if (ops.length > 0) {
            const combinedCallback =
              callbacks.length === 0
                ? undefined
                : callbacks.length === 1
                  ? callbacks[0]
                  : () => callbacks.forEach((cb) => cb());

            const maybePromise = combinedCallback
              ? self._setData(JSON.stringify({ ops }), combinedCallback)
              : self._setData(JSON.stringify({ ops }));

            if (maybePromise && typeof maybePromise.then === "function") {
              maybePromise.catch((err) => {
                console.error("Error in setData:", err);
              });
            }

            self._lastData = JSON.parse(JSON.stringify(self.data));
          } else {
            callbacks.forEach((cb) => cb());
          }
        } catch (err) {
          console.error("Error in setData:", err);
        }
      }, DEBOUNCE_WAIT);
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
      pageConfig.route = pagePath;
      return createPageInstance(pageConfig, pagePath);
    } else {
      throw new Error(`Page not found: ${pagePath}`);
    }
  };

  globalThis.__PAGE_REGISTRY__ = __PAGE_REGISTRY__;
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
        current.splice(index, 1);
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

function jsonPointerEscape(seg) {
  return String(seg).replace(/~/g, "~0").replace(/\//g, "~1");
}

function joinJsonPointer(base, seg) {
  const escaped = jsonPointerEscape(seg);
  if (!base) return `/${escaped}`;
  return `${base}/${escaped}`;
}

function isPlainObject(v) {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}

// Generates JSON Patch ops (RFC 6902 subset: add/replace/remove).
// Arrays are treated as atomic values (replaced as a whole) for simplicity and correctness.
function diffToJsonPatchOps(oldValue, newValue, basePath = "") {
  const ops = [];
  diffToJsonPatchOpsInto(oldValue, newValue, basePath, ops);
  return ops;
}

function diffToJsonPatchOpsInto(oldValue, newValue, path, ops) {
  if (oldValue === newValue) return;

  const oldIsArr = Array.isArray(oldValue);
  const newIsArr = Array.isArray(newValue);
  const oldIsObj = isPlainObject(oldValue);
  const newIsObj = isPlainObject(newValue);

  // Different kinds => replace at this path (including root '').
  if (oldIsArr !== newIsArr || oldIsObj !== newIsObj) {
    ops.push({ op: "replace", path, value: newValue });
    return;
  }

  // Arrays are atomic: replace when changed.
  if (newIsArr) {
    if (JSON.stringify(oldValue) !== JSON.stringify(newValue)) {
      ops.push({ op: "replace", path, value: newValue });
    }
    return;
  }

  // Primitives (including null): replace when changed.
  if (!newIsObj) {
    ops.push({ op: "replace", path, value: newValue });
    return;
  }

  // Both plain objects: recurse.
  const oldKeys = Object.keys(oldValue || {});
  const newKeys = Object.keys(newValue || {});
  const oldSet = new Set(oldKeys);
  const newSet = new Set(newKeys);

  for (const key of oldKeys) {
    if (!newSet.has(key)) {
      ops.push({ op: "remove", path: joinJsonPointer(path, key) });
    }
  }

  for (const key of newKeys) {
    const childPath = joinJsonPointer(path, key);
    if (!oldSet.has(key)) {
      ops.push({ op: "add", path: childPath, value: newValue[key] });
    } else {
      diffToJsonPatchOpsInto(oldValue[key], newValue[key], childPath, ops);
    }
  }
}
