// Augments the global Page function with enhanced setData:
// - Path syntax support.
// - Debouncing.
// - Diff/Patch generation for optimized data transfer.
// Assumes global _Page (native) exists.

(function () {
  try {
    globalThis.Page = function (pageConfig) {
      if (typeof pageConfig !== "object" || pageConfig === null) {
        return new Error("Page configuration must be an object.");
      }
      const initialData =
        typeof pageConfig.data === "object" && pageConfig.data !== null
          ? pageConfig.data
          : {};
      const pageInstance = _Page(pageConfig);

      pageInstance.data = JSON.parse(JSON.stringify(initialData));
      pageInstance._isUpdateScheduled = false;
      pageInstance._debounceTimer = null;
      pageInstance._pendingCallbacks = [];
      pageInstance._batchStartState = null;

      const flushSetData = function () {
        pageInstance._debounceTimer = null;
        if (!pageInstance._isUpdateScheduled) return;

        const startState = pageInstance._batchStartState;
        const endState = pageInstance.data;
        pageInstance._batchStartState = null;
        pageInstance._isUpdateScheduled = false;

        const callbacksToRun = pageInstance._pendingCallbacks;
        pageInstance._pendingCallbacks = [];

        if (startState === null) {
          console.warn(
            "[Lingxia] No diff state available, sending full update",
          );
          const pathUpdates = {}; // Reconstruct roughly from endState if needed, or send empty
          // Or potentially send the full endState as a fallback? For now, send nothing.
          callbacksToRun.forEach((cb) => {
            cb();
          });
          return;
        }

        const patchToSend = diff(startState, endState);

        if (Object.keys(patchToSend).length > 0) {
          try {
            const jsonData = JSON.stringify(patchToSend);

            pageInstance
              ._setData(jsonData)
              .then(() => {
                callbacksToRun.forEach((cb) => {
                  cb();
                });
              })
              .catch((err) => {
                console.error("[Lingxia] Native _setData call failed:", err);
                callbacksToRun.forEach((cb) => {
                  cb();
                });
              });
          } catch (e) {
            console.error(
              "[Lingxia] Failed to stringify PATCH for _setData:",
              e,
              patchToSend,
            );
            callbacksToRun.forEach((cb) => {
              cb();
            });
          }
        } else {
          console.log(
            "[Lingxia] No effective changes detected by diff, running callbacks.",
          );
          callbacksToRun.forEach((cb) => {
            cb();
          });
        }
      };

      pageInstance.setData = function (partialUpdate, callback) {
        if (typeof partialUpdate !== "object" || partialUpdate === null) {
          console.warn("[Lingxia] setData expects an object argument.");
          if (typeof callback === "function") {
            try {
              callback();
            } catch (e) {
              console.error("[Lingxia] Error in setData callback:", e);
            }
          }
          return;
        }

        if (!pageInstance._isUpdateScheduled) {
          try {
            pageInstance._batchStartState = JSON.parse(
              JSON.stringify(pageInstance.data),
            );
          } catch (e) {
            console.error(
              "[Lingxia] Failed to deep copy start state for diffing:",
              e,
            );
            pageInstance._batchStartState = null; // Mark as failed
          }
        }

        // Apply updates synchronously to local data using paths
        for (const path in partialUpdate) {
          if (Object.prototype.hasOwnProperty.call(partialUpdate, path)) {
            const value = partialUpdate[path];
            try {
              setValueByPath(pageInstance.data, path, value);
            } catch (e) {
              console.error(`[Lingxia] Error applying path "${path}":`, e);
            }
          }
        }

        if (typeof callback === "function") {
          pageInstance._pendingCallbacks.push(callback);
        }

        if (!pageInstance._isUpdateScheduled) {
          pageInstance._isUpdateScheduled = true;
          pageInstance._debounceTimer = setTimeout(flushSetData, 0);
        }
      };

      return pageInstance;
    };
  } catch (e) {
    console.error("Error in Page.js IIFE:", e);
  }
})();

// Sets a value on an object based on a path string (e.g., 'a.b[0].c').
// Creates nested objects/arrays if needed. Basic dot/bracket handling.
function setValueByPath(obj, path, value) {
  if (typeof path !== "string" || path === "") {
    console.warn("[Lingxia] Invalid path:", path);
    return;
  }
  if (typeof obj !== "object" || obj === null) {
    console.warn("[Lingxia] Invalid target object");
    return;
  }
  const parts = path.replace(/\[(\d+)\]/g, ".$1").split(".");
  let current = obj;
  for (let i = 0; i < parts.length - 1; i++) {
    const key = parts[i];
    const nextKey = parts[i + 1];
    const isNextKeyArrayIndex = /^\d+$/.test(nextKey);
    if (current[key] === undefined || current[key] === null) {
      current[key] = isNextKeyArrayIndex ? [] : {};
    } else if (typeof current[key] !== "object") {
      console.warn(`[Lingxia] Overwriting path segment "${key}"`);
      current[key] = isNextKeyArrayIndex ? [] : {};
    } else if (isNextKeyArrayIndex && !Array.isArray(current[key])) {
      console.warn(`[Lingxia] Overwriting non-array segment "${key}"`);
      current[key] = [];
    }
    current = current[key];
    if (typeof current !== "object" || current === null) {
      console.error(`[Lingxia] Invalid path segment "${key}"`);
      return;
    }
  }
  const finalKey = parts[parts.length - 1];
  if (value === undefined) {
    if (Array.isArray(current)) {
      const index = parseInt(finalKey, 10);
      if (!isNaN(index) && index >= 0 && index < current.length) {
        // While we could splice, deleting might leave 'empty' slots if that's desired.
        // However, consistent 'delete' behavior is simpler. Let's use delete.
        // If sparse arrays become problematic, we might revisit splice.
        delete current[finalKey];
      } else {
        console.warn(
          `[Lingxia] Invalid array index "${finalKey}" for deletion.`,
        );
      }
    } else if (typeof current === "object" && current !== null) {
      delete current[finalKey];
    } else {
      console.warn(
        `[Lingxia] Cannot delete property "${finalKey}" from non-object/array:`,
        current,
      );
    }
  } else {
    current[finalKey] = value;
  }
}

/**
 * Recursively diffs two objects/values and generates a patch object.
 * Patch format: { 'path.string': newValue, 'deleted.path': undefined }
 * Basic array diffing: sends the whole new array if changed.
 */
function diff(oldValue, newValue, currentPath = "", patch = {}) {
  if (oldValue === newValue) {
    return patch; // No change
  }
  if (
    typeof oldValue !== typeof newValue ||
    typeof newValue !== "object" ||
    newValue === null ||
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
  if (typeof newValue === "object") {
    const oldKeys = new Set(Object.keys(oldValue));
    const newKeys = new Set(Object.keys(newValue));
    for (const key of newKeys) {
      const newPath = currentPath ? `${currentPath}.${key}` : key;
      if (!oldKeys.has(key)) {
        patch[newPath] = newValue[key]; // Added key
      } else {
        diff(oldValue[key], newValue[key], newPath, patch); // Recurse for existing key
      }
    }
    for (const key of oldKeys) {
      if (!newKeys.has(key)) {
        const deletedPath = currentPath ? `${currentPath}.${key}` : key;
        patch[deletedPath] = undefined; // Mark deleted key
      }
    }
  }
  return patch;
}
