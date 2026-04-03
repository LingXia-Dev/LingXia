(function () {
  const pageDefinitions = new Map();

  function createPageInstance(definition, pagePath) {
    const pageConfig = definition && definition.config;
    if (!pageConfig || typeof pageConfig !== "object") {
      throw new Error("setData: Invalid page configuration");
    }

    const pageSvc = new PageSvc(
      pageConfig,
      pagePath,
      definition.bindingMetaJson || '{"handlers":[]}',
    );

    pageSvc.data = cloneJsonValue(pageConfig.data || {});

    for (const [key, value] of Object.entries(pageConfig)) {
      if (key === "data") {
        continue;
      }
      if (typeof value === "function") {
        pageSvc[key] = value.bind(pageSvc);
      } else {
        pageSvc[key] = value;
      }
    }

    let updateTimer = null;
    const pendingBaseState = new Map();
    const pendingOps = new Map();
    let pendingCallbacks = [];
    const DEBOUNCE_WAIT = 16;

    pageSvc.setData = function (updates, callback) {
      if (!updates || typeof updates !== "object") {
        throw new Error("setData: Invalid updates");
      }

      const self = this;

      try {
        for (const [path, value] of Object.entries(updates)) {
          applyUpdate(self.data, pendingBaseState, pendingOps, path, value);
        }
      } catch (err) {
        console.error("Error in setData:", err);
        return;
      }

      if (typeof callback === "function") {
        pendingCallbacks.push(callback);
      }

      clearTimeout(updateTimer);
      updateTimer = setTimeout(() => {
        const ops = Array.from(pendingOps.values()).map(toJsonPatchOp);
        const callbacks = pendingCallbacks;

        pendingBaseState.clear();
        pendingOps.clear();
        pendingCallbacks = [];

        try {
          if (ops.length === 0) {
            callbacks.forEach((cb) => cb());
            return;
          }

          const combinedCallback =
            callbacks.length === 0
              ? undefined
              : callbacks.length === 1
                ? callbacks[0]
                : () => callbacks.forEach((cb) => cb());

          const maybePromise = combinedCallback
            ? self._setData(JSON.stringify(ops), combinedCallback)
            : self._setData(JSON.stringify(ops));

          if (maybePromise && typeof maybePromise.then === "function") {
            maybePromise.catch((err) => {
              console.error("Error in setData:", err);
            });
          }
        } catch (err) {
          console.error("Error in setData:", err);
        }
      }, DEBOUNCE_WAIT);
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

  globalThis.Page = function () {
    throw new Error(
      "Page() should be transformed at build time. Rebuild the logic bundle with the LingXia CLI.",
    );
  };

  globalThis.__LX_CREATE_PAGE__ = function (pagePath, definitionPath) {
    const resolvedDefinitionPath = definitionPath || pagePath;
    const definition = pageDefinitions.get(resolvedDefinitionPath);
    if (!definition) {
      throw new Error(`Page not found: ${resolvedDefinitionPath}`);
    }
    definition.config.route = pagePath;
    return createPageInstance(definition, pagePath);
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
