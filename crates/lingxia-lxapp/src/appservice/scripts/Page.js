(function () {
  const pageDefinitions = new Map();
  // setData batching keeps real time. The timer globals are installed after
  // this script runs, so the Logic worker calls the hidden capture hook right
  // after the timer module is initialised — before app code, and so before a
  // test clock, can replace them. A faked clock then never holds View updates.
  let realTimers = null;
  function captureTimers() {
    const set = globalThis.setTimeout;
    const clear = globalThis.clearTimeout;
    if (typeof set !== "function" || typeof clear !== "function") {
      throw new Error("setData: timers are not available to capture");
    }
    realTimers = { set, clear };
  }
  Object.defineProperty(globalThis, "__lxCaptureTimers", {
    value: captureTimers,
    enumerable: false,
    writable: false,
    configurable: true,
  });
  function timers() {
    if (!realTimers) {
      throw new Error("setData: timers were not captured when the Logic worker started");
    }
    return realTimers;
  }
  function setTimer(fn, delay) {
    return timers().set(fn, delay);
  }
  function clearTimer(id) {
    return timers().clear(id);
  }

  function createPageInstance(definition, pagePath, pageInstanceId) {
    const pageConfig = definition && definition.config;
    if (!pageConfig || typeof pageConfig !== "object") {
      throw new Error("setData: Invalid page configuration");
    }

    for (const key of ['setData', 'setPath', 'setDataPath', 'flush', 'signal', 'surface', 'opener', '_abortLifetime', '_cancelPendingSetData', '_setData']) {
      if (Object.prototype.hasOwnProperty.call(pageConfig, key)) {
        throw new TypeError(`Page member '${key}' is reserved by the runtime`);
      }
    }
    assertPageData(pageConfig.data || {});
    const pageSvc = new PageSvc(
      pageConfig,
      pagePath,
      definition.bindingMetaJson || '{"handlers":[]}',
      pageInstanceId || "",
    );

    pageSvc.data = cloneJsonValue(pageConfig.data || {});

    for (const [key, value] of Object.entries(pageConfig)) {
      if (key === "data") {
        continue;
      }
      if (typeof value === "function") {
        pageSvc[key] = function (...args) {
          return value.apply(pageSvc, args);
        };
      } else {
        pageSvc[key] = value;
      }
    }

    // Aborted as the page unloads — just before `onUnload` runs, or when the
    // page is retired without one — so work the page started (a fetch, a
    // subscription) stops with it instead of resolving into a page that is gone.
    const lifetime = new AbortController();
    Object.defineProperty(pageSvc, "signal", {
      value: lifetime.signal,
      enumerable: false,
      writable: false,
      configurable: false,
    });

    let updateTimer = null;
    const pendingBaseState = new Map();
    const pendingOps = new Map();
    let pendingCallbacks = [];
    let disposed = false;
    let submitted = Promise.resolve();
    const flushWaiters = new Set();
    const rejectFlushes = (error) => {
      for (const waiter of flushWaiters) waiter.reject(error);
      flushWaiters.clear();
    };
    const DEBOUNCE_WAIT = 16;

    // A listener that throws is the page's bug: report it, never let it cut
    // the runtime's own teardown short. Aborting twice is a no-op.
    pageSvc._abortLifetime = function () {
      try {
        lifetime.abort();
      } catch (error) {
        console.error("A page's signal abort listener threw:", error);
      }
    };

    pageSvc._cancelPendingSetData = function () {
      disposed = true;
      rejectFlushes(new Error("Page unloaded before its state was flushed"));
      clearTimer(updateTimer);
      updateTimer = null;
      pendingBaseState.clear();
      pendingOps.clear();
      pendingCallbacks = [];
      pageSvc._abortLifetime();
    };

    pageSvc.setData = function (updates) {
      if (disposed) {
        return;
      }
      if (!updates || typeof updates !== "object") {
        throw new Error("setData: Invalid updates");
      }

      assertPageData(updates);
      const self = this;

      try {
        for (const [path, value] of Object.entries(updates)) {
          applyUpdate(self.data, pendingBaseState, pendingOps, path, value);
        }
      } catch (err) {
        throw err;
      }

      clearTimer(updateTimer);
      updateTimer = setTimer(submitPending, DEBOUNCE_WAIT);
    };

    function submitPending() {
      updateTimer = null;
      if (disposed) {
        return;
      }
      const ops = Array.from(pendingOps.values()).map(toJsonPatchOp);
      const callbacks = pendingCallbacks;

      pendingBaseState.clear();
      pendingOps.clear();
      pendingCallbacks = [];

      // Every flush includes earlier batches, including batches already in flight.
      // A failed batch has already rejected its waiters; later batches stand alone.
      const previous = submitted.catch(() => {});
      // Native reports one of "acked" | "deferred" | "dropped". It calls back on
      // every terminal path, so only the status tells an acknowledgement apart
      // from a write the View will never see.
      const current = ops.length === 0 ? Promise.resolve("acked") : new Promise((resolve, reject) => {
        try {
          Promise.resolve(pageSvc._setData(JSON.stringify(ops), resolve)).catch(reject);
        } catch (error) {
          reject(error);
        }
      });
      submitted = Promise.all([previous, current]).then(([, status]) => {
        callbacks.forEach((cb) => cb(status));
      });
      submitted.catch((error) => {
        rejectFlushes(error);
        console.error("Error in setData:", error);
      });
    }

    pageSvc.flush = function () {
      if (disposed) return Promise.reject(new Error("Cannot flush an unloaded page"));
      return new Promise((resolve, reject) => {
        const waiter = { reject };
        flushWaiters.add(waiter);
        pendingCallbacks.push((status) => {
          flushWaiters.delete(waiter);
          // "deferred" still reaches the View, through the bridge-ready
          // snapshot rather than this patch. "dropped" never does.
          if (status === "dropped") {
            reject(new Error("Page state was discarded before the View received it"));
            return;
          }
          resolve();
        });
        // Submit now: a steady setData stream would otherwise re-arm the
        // debounce and starve the flush.
        clearTimer(updateTimer);
        submitPending();
      });
    };

    pageSvc.setPath = function (path, value) {
      return pageSvc.setData({ [pathSegmentsToKey(path)]: value });
    };

    pageSvc.setDataPath = function (path, value) {
      if (typeof path !== "string" || !path) {
        throw new Error("setDataPath: Invalid path");
      }
      return pageSvc.setData({ [path]: value });
    };

    return pageSvc;
  }

  globalThis.__registerPage = function (
    pagePath,
    pageConfig,
    bindingMetaJson,
  ) {
    if (!pagePath) {
      throw new Error(
        "__registerPage() called without page path. This indicates a build configuration issue.",
      );
    }
    pageDefinitions.set(pagePath, {
      config: pageConfig,
      bindingMetaJson: bindingMetaJson || '{"handlers":[]}',
    });
  };

  function pageRegistrationTransformError(symbolName) {
    throw new Error(
      `${symbolName}() should be transformed at build time. Rebuild the logic bundle with the LingXia CLI.`,
    );
  }

  globalThis.Page = function () {
    pageRegistrationTransformError("Page");
  };

  globalThis.PageInstance = function () {
    pageRegistrationTransformError("PageInstance");
  };

  globalThis.__LX_CREATE_PAGE__ = function (
    pagePath,
    definitionPath,
    pageInstanceId,
  ) {
    const resolvedDefinitionPath = definitionPath || pagePath;
    const definition = pageDefinitions.get(resolvedDefinitionPath);
    if (!definition) {
      throw new Error(`PageInstance not found: ${resolvedDefinitionPath}`);
    }
    definition.config.route = pagePath;
    return createPageInstance(definition, pagePath, pageInstanceId);
  };
})();

function applyUpdate(root, pendingBaseState, pendingOps, path, nextValue) {
  const segments = parseDataPath(path);
  captureBaseState(root, pendingBaseState, segments);

  const previous = getValueAtPath(root, segments);
  const pointer = segmentsToJsonPointer(segments);
  const pendingBaseEntry = pendingBaseState.get(pointer);
  const existedBefore =
    pendingBaseEntry && typeof pendingBaseEntry.exists === "boolean"
      ? pendingBaseEntry.exists
      : previous.exists;

  if (nextValue === undefined) {
    if (!previous.exists && !existedBefore) {
      return;
    }
  } else if (previous.exists && isDeepEqual(previous.value, nextValue)) {
    return;
  }

  setValueBySegments(root, segments, nextValue);
  enqueuePendingPatch(root, pendingOps, segments, existedBefore);
}

function pathSegmentsToKey(path) {
  if (!Array.isArray(path) || path.length === 0) {
    throw new Error("setPath: Invalid path");
  }
  let key = "";
  for (const segment of path) {
    if (typeof segment === "number") {
      if (!Number.isInteger(segment) || segment < 0) {
        throw new Error("setPath: Invalid path");
      }
      key += `[${segment}]`;
      continue;
    }
    if (typeof segment !== "string" || segment === "") {
      throw new Error("setPath: Invalid path");
    }
    key = key === "" ? segment : `${key}.${segment}`;
  }
  return key;
}

function parseDataPath(path) {
  if (!path) {
    throw new Error("setData: Invalid path");
  }

  return path.replace(/\[(\d+)\]/g, ".$1").split(".");
}

function getValueAtPath(root, segments) {
  let current = root;

  for (let i = 0; i < segments.length; i++) {
    const key = segments[i];
    if (!current || typeof current !== "object") {
      return { exists: false, value: undefined };
    }

    if (Array.isArray(current)) {
      const index = parseInt(key, 10);
      if (Number.isNaN(index) || index < 0 || index >= current.length) {
        return { exists: false, value: undefined };
      }
      current = current[index];
      continue;
    }

    if (!Object.prototype.hasOwnProperty.call(current, key)) {
      return { exists: false, value: undefined };
    }
    current = current[key];
  }

  return { exists: true, value: current };
}

function setValueBySegments(root, segments, value) {
  let current = root;

  for (let i = 0; i < segments.length - 1; i++) {
    const key = segments[i];
    const nextKey = segments[i + 1];
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

  const finalKey = segments[segments.length - 1];
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
      return;
    }

    if (current && typeof current === "object") {
      delete current[finalKey];
      return;
    }

    throw new Error(`setData: Cannot delete property "${finalKey}"`);
  }

  current[finalKey] = value;
}

function captureBaseState(root, pendingBaseState, segments) {
  for (let depth = 1; depth <= segments.length; depth++) {
    const partialSegments = segments.slice(0, depth);
    const pointer = segmentsToJsonPointer(partialSegments);
    if (pendingBaseState.has(pointer)) {
      continue;
    }

    const snapshot = getValueAtPath(root, partialSegments);
    pendingBaseState.set(pointer, {
      exists: snapshot.exists,
    });
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

function segmentsToJsonPointer(segments) {
  let pointer = "";
  for (const seg of segments) {
    pointer = joinJsonPointer(pointer, seg);
  }
  return pointer;
}

function enqueuePendingPatch(root, pendingOps, segments, existedBefore) {
  const existingEntry = pendingOps.get(segmentsToJsonPointer(segments));
  const hadExisted =
    existingEntry && typeof existingEntry.hadExisted === "boolean"
      ? existingEntry.hadExisted
      : existedBefore;

  for (let i = segments.length - 1; i > 0; i--) {
    const ancestorPointer = segmentsToJsonPointer(segments.slice(0, i));
    const ancestorEntry = pendingOps.get(ancestorPointer);
    if (!ancestorEntry) {
      continue;
    }

    const refreshedAncestor = buildPendingPatchEntry(
      root,
      ancestorEntry.segments,
      ancestorEntry.hadExisted,
    );
    if (refreshedAncestor) {
      pendingOps.set(ancestorPointer, refreshedAncestor);
    } else {
      pendingOps.delete(ancestorPointer);
    }
    return;
  }

  for (const [pointer, entry] of pendingOps.entries()) {
    if (
      pointer !== segmentsToJsonPointer(segments) &&
      isDescendantPointer(pointer, segments)
    ) {
      pendingOps.delete(pointer);
    }
  }

  const nextEntry = buildPendingPatchEntry(root, segments, hadExisted);
  if (!nextEntry) {
    pendingOps.delete(segmentsToJsonPointer(segments));
    return;
  }
  pendingOps.set(nextEntry.path, nextEntry);
}

function buildPendingPatchEntry(root, segments, hadExisted) {
  const current = getValueAtPath(root, segments);
  if (!current.exists) {
    if (!hadExisted) {
      return null;
    }
    return {
      path: segmentsToJsonPointer(segments),
      segments: [...segments],
      hadExisted,
      op: "remove",
    };
  }

  return {
    path: segmentsToJsonPointer(segments),
    segments: [...segments],
    hadExisted,
    op: hadExisted ? "replace" : "add",
    value: cloneJsonValue(current.value),
  };
}

function isDescendantPointer(candidatePointer, ancestorSegments) {
  const ancestorPointer = segmentsToJsonPointer(ancestorSegments);
  return candidatePointer.startsWith(`${ancestorPointer}/`);
}

function toJsonPatchOp(entry) {
  if (entry.op === "remove") {
    return {
      op: entry.op,
      path: entry.path,
    };
  }

  return {
    op: entry.op,
    path: entry.path,
    value: entry.value,
  };
}

function cloneJsonValue(value) {
  if (value === undefined) {
    return undefined;
  }

  if (typeof structuredClone === "function") {
    return structuredClone(value);
  }

  return JSON.parse(JSON.stringify(value));
}

function isPlainObject(v) {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}

function isDeepEqual(a, b) {
  if (a === b) {
    return true;
  }

  if (Array.isArray(a) || Array.isArray(b)) {
    if (!Array.isArray(a) || !Array.isArray(b) || a.length !== b.length) {
      return false;
    }
    for (let i = 0; i < a.length; i++) {
      if (!isDeepEqual(a[i], b[i])) {
        return false;
      }
    }
    return true;
  }

  if (!isPlainObject(a) || !isPlainObject(b)) {
    return false;
  }

  const keysA = Object.keys(a);
  const keysB = Object.keys(b);
  if (keysA.length !== keysB.length) {
    return false;
  }

  for (const key of keysA) {
    if (!Object.prototype.hasOwnProperty.call(b, key)) {
      return false;
    }
    if (!isDeepEqual(a[key], b[key])) {
      return false;
    }
  }

  return true;
}

// Undefined remains the explicit remove operation; other values must survive JSON transport.
function assertPageData(value, seen = new Set()) {
  if (value === null || value === undefined || typeof value === 'string' || typeof value === 'boolean') return;
  if (typeof value === 'number' && Number.isFinite(value)) return;
  if (typeof value !== 'object' || seen.has(value)) throw new TypeError('Page data must be acyclic JSON values');
  const proto = Object.getPrototypeOf(value);
  if (!Array.isArray(value) && proto !== Object.prototype && proto !== null) {
    throw new TypeError('Page data cannot contain class instances');
  }
  if (Object.getOwnPropertySymbols(value).length) throw new TypeError('Page data cannot contain symbol keys');
  seen.add(value);
  for (const item of Object.values(value)) assertPageData(item, seen);
  seen.delete(value);
}
