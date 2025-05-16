// Augments the global Page function with enhanced setData:
// - Path syntax support.
// - Debouncing.
// - Diff/Patch generation for optimized data transfer.

(function () {
  globalThis.Page = function (pageConfig) {
    if (!pageConfig || typeof pageConfig !== "object") {
      throw new Error("setData: Invalid page configuration");
    }

    const pageSvc = new PageSvc(pageConfig);

    // Initialize data
    pageSvc.data = JSON.parse(JSON.stringify(pageConfig.data || {}));
    pageSvc._lastData = JSON.parse(JSON.stringify(pageSvc.data));

    // Setup state management
    let updateTimer = null;
    let pendingData = null;
    let pendingCallbacks = [];
    const DEBOUNCE_WAIT = 50;

    // Enhanced setData with optimizations
    pageSvc.setData = async function (updates, callback) {
      if (!updates || typeof updates !== "object") {
        throw new Error("setData: Invalid updates");
      }

      // Queue updates and callback
      pendingData = pendingData ? { ...pendingData, ...updates } : updates;
      if (callback) pendingCallbacks.push(callback);

      // Debounce updates
      clearTimeout(updateTimer);
      updateTimer = setTimeout(async () => {
        try {
          // Capture and reset pending state
          const currentUpdates = pendingData;
          const callbacks = pendingCallbacks;
          pendingData = null;
          pendingCallbacks = [];

          // Apply updates to local data
          for (const [path, value] of Object.entries(currentUpdates)) {
            setValueByPath(this.data, path, value);
          }

          // Generate and send patch
          const patch = diff(this._lastData, this.data);
          if (Object.keys(patch).length > 0) {
            await this._setData(JSON.stringify(patch));
            this._lastData = JSON.parse(JSON.stringify(this.data));
          }

          // Execute callbacks
          callbacks.forEach((cb) => cb());
        } catch (err) {
          pendingCallbacks.forEach((cb) => cb(err));
          throw err;
        }
      }, DEBOUNCE_WAIT);
    };

    // Bind page methods
    Object.entries(pageConfig)
      .filter(([_, value]) => typeof value === "function")
      .forEach(([key, fn]) => (pageSvc[key] = fn.bind(pageSvc)));

    return pageSvc;
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
