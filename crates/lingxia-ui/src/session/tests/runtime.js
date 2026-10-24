// Generated from packages/lingxia-ui/contracts/schema.json; do not edit.
(() => {
  var __defProp = Object.defineProperty;
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true });
  };

  // src/internal.js
  var internal_exports = {};
  __export(internal_exports, {
    List: () => List,
    childrenToArray: () => childrenToArray,
    commit: () => commit,
    configureActions: () => configureActions,
    configureClasses: () => configureClasses,
    createView: () => createView,
    describe: () => describe,
    discard: () => discard,
    dispatch: () => dispatch,
    h: () => h,
    hasLocalUpdates: () => hasLocalUpdates,
    prepare: () => prepare,
    prepareWindows: () => prepareWindows,
    receiveActionResponse: () => receiveActionResponse,
    requestLeave: () => requestLeave,
    reset: () => reset,
    takeActionErrors: () => takeActionErrors,
    takeActionRequests: () => takeActionRequests,
    useChannel: () => useChannel,
    useLeaveGuard: () => useLeaveGuard,
    useLocal: () => useLocal,
    useRequest: () => useRequest,
    useStream: () => useStream,
    useView: () => useView
  });

  // src/runtime/resources.js
  function streamResource(state) {
    let definition, mounted = false, ticket = 0, stream, snapshot = state.value;
    const update = (patch) => {
      snapshot = { ...snapshot, ...patch };
      state.set(snapshot);
    };
    const cancel = () => {
      ticket++;
      const old = stream;
      stream = void 0;
      if (old) {
        old.cancel();
        update({ status: "canceled" });
      }
    };
    return {
      get state() {
        return snapshot;
      },
      cancel,
      start(params) {
        if (!mounted) return;
        cancel();
        const current = ++ticket;
        update({ status: "streaming", error: null, result: null });
        try {
          if (!definition.action) throw Error("UI_STREAM: bridge not ready");
          stream = definition.action(params);
          stream.on("data", (value) => {
            if (!mounted || current !== ticket) return;
            try {
              definition.onData(value);
            } catch (error) {
              cancel();
              update({ status: "failed", error: String(error.message ?? error) });
            }
          });
          stream.result.then((value) => {
            if (!mounted || current !== ticket) return;
            stream = void 0;
            update({ status: "completed", result: value ?? null });
          }, (error) => {
            if (!mounted || current !== ticket) return;
            stream = void 0;
            update({ status: error.code === "BRIDGE_CANCELED" ? "canceled" : "failed", error: String(error.message ?? error) });
          });
        } catch (error) {
          stream = void 0;
          update({ status: "failed", error: String(error.message ?? error) });
        }
      },
      commit(next) {
        if (definition && (definition.action !== next.action || definition.method !== next.method)) {
          cancel();
          update({ status: "idle", error: null, result: null });
        }
        definition = next;
        mounted = true;
      },
      dispose() {
        mounted = false;
        cancel();
      }
    };
  }
  function requestResource(state) {
    let definition, mounted = false, ticket = 0, snapshot = state.value;
    const update = (patch) => {
      snapshot = { ...snapshot, ...patch };
      state.set(snapshot);
    };
    const reload = async () => {
      if (!mounted || !definition.action) return;
      const current = ++ticket;
      update({ pending: true, value: null, error: null });
      try {
        const value = await definition.action(definition.params);
        if (mounted && current === ticket) update({ pending: false, value, error: null });
      } catch (error) {
        if (mounted && current === ticket)
          update({ pending: false, value: null, error: String(error.message ?? error) });
      }
    };
    return {
      get state() {
        return snapshot;
      },
      reload,
      commit(next) {
        const changed = !definition || definition.action !== next.action || definition.method !== next.method || definition.key !== next.key;
        definition = next;
        mounted = true;
        if (changed) {
          ticket++;
          if (next.action) void reload();
          else update({ pending: false, value: null, error: null });
        }
      },
      dispose() {
        mounted = false;
        ticket++;
      }
    };
  }

  // src/generated/wire.js
  var tags = { "column": 0, "row": 1, "text": 2, "button": 3, "spacer": 4, "scroll": 5, "list": 6, "image": 7, "input": 8, "textarea": 9, "checkbox": 10, "switch": 11, "radio": 12, "radio-option": 13, "search": 14, "slider": 15, "grid": 16, "stack": 17, "plot": 18, "vector": 19, "page": 20 };
  var field = { "width": 0, "height": 1, "gap": 2, "padding": 3, "fontSize": 4, "color": 5, "background": 6, "grow": 7, "onClick": 8, "enabled": 9, "radius": 10, "listId": 11, "listCount": 12, "listLeading": 13, "listTrailing": 14, "listRowIndex": 15, "listAnchor": 16, "onReachStart": 17, "onReachEnd": 18, "imageSrc": 19, "imageFit": 20, "inputValue": 21, "placeholder": 22, "secure": 23, "keyboard": 24, "autoFocus": 25, "onInput": 26, "onSubmit": 27, "minWidth": 28, "minHeight": 29, "maxWidth": 30, "maxHeight": 31, "shrink": 32, "align": 33, "justify": 34, "alignSelf": 35, "fontWeight": 36, "lineHeight": 37, "fontFamily": 38, "checked": 39, "onCheckedChange": 40, "accessibilityLabel": 41, "sliderValue": 42, "sliderMin": 43, "sliderMax": 44, "sliderStep": 45, "onSliderChange": 46, "gridColumns": 47, "offsetX": 48, "offsetY": 49, "textLines": 50, "plotValues": 51, "plotKind": 52, "plotColors": 53, "plotSmooth": 54, "plotArea": 55, "onPlotSelect": 56, "plotSelectedIndex": 57, "opacity": 58, "paddingTop": 59, "paddingRight": 60, "paddingBottom": 61, "paddingLeft": 62, "wordBreak": 63, "overflowWrap": 64, "borderWidth": 65, "borderColor": 66, "borderTopWidth": 67, "borderRightWidth": 68, "borderBottomWidth": 69, "borderLeftWidth": 70, "textAlign": 71, "backgroundGradient": 72, "boxShadow": 73, "widthPercent": 74, "heightPercent": 75, "minWidthPercent": 76, "minHeightPercent": 77, "maxWidthPercent": 78, "maxHeightPercent": 79, "vectorSource": 80, "textDecorationLine": 81, "heightAuto": 82, "submitOnEnter": 83, "fontStyle": 84, "plotDomainMin": 85, "plotDomainMax": 86, "flexWrap": 87, "plotPieLayout": 88, "letterSpacing": 89, "radiusTopLeft": 90, "radiusTopRight": 91, "radiusBottomRight": 92, "radiusBottomLeft": 93, "plotMarkerEmphasisScale": 94, "backdropBlur": 95, "backdropSaturation": 96, "placeholderColor": 97, "focusStyle": 98, "flexBasis": 99, "scrollAxis": 100, "animation": 101, "listItemKeys": 102, "flexDirection": 103, "display": 104, "imageAlt": 105, "imageRetryKey": 106, "onImageLoad": 107, "onImageError": 108, "marginTop": 109, "marginRight": 110, "marginBottom": 111, "marginLeft": 112, "position": 113, "insetTop": 114, "insetRight": 115, "insetBottom": 116, "insetLeft": 117, "zIndex": 118, "overflow": 119, "rowGap": 120, "columnGap": 121, "textTransform": 122, "whiteSpace": 123, "textOverflow": 124, "pressedStyle": 125, "hoverStyle": 126, "widthKeyword": 127, "heightKeyword": 128 };
  var fields = { "width": 0, "height": 1, "gap": 2, "padding": 3, "fontSize": 4, "color": 5, "background": 6, "grow": 7, "radius": 10, "minWidth": 28, "minHeight": 29, "maxWidth": 30, "maxHeight": 31, "shrink": 32, "align": 33, "justify": 34, "alignSelf": 35, "fontWeight": 36, "lineHeight": 37, "fontFamily": 38, "opacity": 58, "paddingTop": 59, "paddingRight": 60, "paddingBottom": 61, "paddingLeft": 62, "wordBreak": 63, "overflowWrap": 64, "borderWidth": 65, "borderColor": 66, "borderTopWidth": 67, "borderRightWidth": 68, "borderBottomWidth": 69, "borderLeftWidth": 70, "textAlign": 71, "backgroundGradient": 72, "boxShadow": 73, "textDecorationLine": 81, "fontStyle": 84, "flexWrap": 87, "letterSpacing": 89, "radiusTopLeft": 90, "radiusTopRight": 91, "radiusBottomRight": 92, "radiusBottomLeft": 93, "backdropBlur": 95, "backdropSaturation": 96, "placeholderColor": 97, "flexBasis": 99, "scrollAxis": 100, "animation": 101, "flexDirection": 103, "display": 104, "rowGap": 120, "columnGap": 121 };
  var vectorField = 80;
  var dimensionPercentFields = { "width": 74, "height": 75, "minWidth": 76, "minHeight": 77, "maxWidth": 78, "maxHeight": 79 };
  var styleOptions = { "align": ["start", "center", "end", "stretch", "baseline"], "justify": ["start", "center", "end", "between", "around", "evenly"], "alignSelf": ["start", "center", "end", "stretch", "baseline"], "fontFamily": ["sans", "mono"], "wordBreak": ["normal", "break-all", "keep-all"], "overflowWrap": ["normal", "break-word", "anywhere"], "textAlign": ["start", "end", "left", "center", "right", "justify"], "textDecorationLine": ["none", "line-through", "underline"], "fontStyle": ["normal", "italic"], "flexWrap": ["nowrap", "wrap", "wrap-reverse"], "scrollAxis": ["horizontal", "vertical"], "flexDirection": ["row", "column", "row-reverse", "column-reverse"], "display": ["flex"], "position": ["relative", "absolute"], "overflow": ["visible", "hidden"], "textTransform": ["none", "uppercase", "lowercase", "capitalize"], "whiteSpace": ["normal", "nowrap"], "textOverflow": ["clip", "ellipsis"], "widthKeyword": ["fit-content", "max-content", "min-content"], "heightKeyword": ["fit-content", "max-content", "min-content"] };
  var buttonProps = { "onClick": 8, "enabled": 9, "accessibilityLabel": 41 };
  var listFields = { "listId": 11, "listCount": 12, "leading": 13, "trailing": 14, "rowIndex": 15, "anchor": 16, "itemKeys": 102 };
  var reachProps = { "onReachStart": 17, "onReachEnd": 18 };
  var imageProps = { "src": 19, "fit": 20, "alt": 105, "retryKey": 106, "onLoad": 107, "onError": 108 };
  var inputProps = { "value": 21, "placeholder": 22, "secure": 23, "keyboard": 24, "autoFocus": 25, "onInput": 26, "onSubmit": 27, "submitOnEnter": 83 };
  var checkProps = { "value": 39, "onChange": 40 };
  var radioProps = { "accessibilityLabel": 41 };
  var sliderProps = { "accessibilityLabel": 41, "value": 42, "min": 43, "max": 44, "step": 45, "onChange": 46 };
  var gridProps = { "columns": 47 };
  var textProps = { "lines": 50, "textAlign": 71, "textDecorationLine": 81, "textTransform": 122, "textOverflow": 124 };
  var plotProps = { "values": 51, "kind": 52, "colors": 53, "accessibilityLabel": 41, "smooth": 54, "area": 55, "onSelect": 56, "selectedIndex": 57, "domainMin": 85, "domainMax": 86, "pieLayout": 88, "markerEmphasisScale": 94 };

  // src/authoring/props.js
  var has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
  var common = /* @__PURE__ */ new Set(["key", "testId", "style", "className", "children"]);
  var extra = {
    list: ["count", "row", "height", "anchor", "itemKeys"],
    vector: ["paths", "viewBox", "fit"],
    input: ["bind", "enabled", "accessibilityLabel"],
    textarea: ["bind", "enabled", "accessibilityLabel"],
    search: ["bind", "enabled", "accessibilityLabel"],
    checkbox: ["bind", "enabled", "accessibilityLabel"],
    switch: ["bind", "enabled"],
    slider: ["bind", "enabled"],
    radio: ["options", "value", "bind", "enabled", "onChange"]
  };
  var wireProps = {
    button: buttonProps,
    text: textProps,
    grid: gridProps,
    plot: plotProps,
    image: imageProps,
    input: inputProps,
    textarea: inputProps,
    search: inputProps,
    checkbox: checkProps,
    switch: checkProps,
    slider: sliderProps,
    radio: radioProps
  };
  function acceptsProp(tag, key) {
    if (tag === "search" && ["secure", "keyboard"].includes(key)) return false;
    return common.has(key) || extra[tag]?.includes(key) || wireProps[tag] !== void 0 && has(wireProps[tag], key) || ["scroll", "list"].includes(tag) && has(reachProps, key) || false;
  }

  // src/style/shadow.js
  function normalizeShadows(value) {
    if (value === "none") return value;
    const invalid = () => {
      throw Error("UI_SHADOW: invalid box shadow");
    };
    if (!Array.isArray(value) || value.length > 8) invalid();
    return value.map((shadow) => {
      if (!shadow || typeof shadow !== "object" || Array.isArray(shadow) || Object.keys(shadow).some((key) => !["offsetX", "offsetY", "blur", "spread", "color"].includes(key))) invalid();
      const result = {};
      for (const key of ["offsetX", "offsetY", "blur", "spread"]) {
        const number = shadow[key] === void 0 ? 0 : shadow[key];
        if (typeof number !== "number" || !Number.isFinite(number) || (key === "blur" ? number < 0 || number > 256 : Math.abs(number) > 1024)) invalid();
        result[key] = number;
      }
      result.color = shadow.color === void 0 ? "currentColor" : shadow.color;
      if (result.color !== "currentColor" && (typeof result.color !== "string" || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(result.color))) invalid();
      return result;
    });
  }

  // src/style/gradient.js
  var directions = ["top", "right", "bottom", "left", "top-right", "bottom-right", "bottom-left", "top-left"];
  function normalizeGradient(value) {
    if (value === "none") return value;
    const invalid = () => {
      throw Error("UI_GRADIENT: invalid linear gradient");
    };
    if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some((key) => !["direction", "angle", "colorSpace", "stops"].includes(key)) || value.direction !== void 0 && !directions.includes(value.direction) || value.angle !== void 0 && (typeof value.angle !== "number" || !Number.isFinite(value.angle) || Math.abs(value.angle) > 36e3) || value.direction !== void 0 && value.angle !== void 0 || value.colorSpace !== void 0 && !["srgb", "oklab"].includes(value.colorSpace) || !Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 16) invalid();
    let previous = -Infinity;
    const stops = value.stops.map((stop) => {
      if (!stop || typeof stop !== "object" || Array.isArray(stop) || Object.keys(stop).some((key) => key !== "offset" && key !== "color") || typeof stop.offset !== "number" || !Number.isFinite(stop.offset) || Math.abs(stop.offset) > 16 || stop.offset < previous || typeof stop.color !== "string" || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(stop.color)) invalid();
      previous = stop.offset;
      return { offset: stop.offset, color: stop.color };
    });
    return {
      ...value.direction !== void 0 ? { direction: value.direction } : {},
      ...value.angle !== void 0 ? { angle: value.angle } : {},
      colorSpace: value.colorSpace ?? "srgb",
      stops
    };
  }

  // src/style/classes.js
  var byName = /* @__PURE__ */ new Map();
  var styles = /* @__PURE__ */ new Map();
  var styleTheme;
  var styleWidth;
  function copy(value) {
    if (Array.isArray(value)) return value.map(copy);
    if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, copy(item)]));
    return value;
  }
  function configureClasses(compiled) {
    byName = /* @__PURE__ */ new Map();
    styles.clear();
    compiled.forEach((rule, order) => {
      if (!byName.has(rule.name)) byName.set(rule.name, []);
      byName.get(rule.name).push({ ...copy(rule), order });
    });
  }
  function classStyle(className, env = {}, focused = false) {
    if (className === void 0) return {};
    if (typeof className !== "string") throw Error("UI_CLASS: className must be a string");
    if (className.length > 16384) throw Error("UI_CLASS: className exceeds 16384 characters");
    const theme = env.theme ?? "light", width = env.viewport?.width;
    if (theme !== styleTheme || width !== styleWidth) {
      styles.clear();
      styleTheme = theme;
      styleWidth = width;
    }
    const cacheKey = `${focused ? "1" : "0"}${className}`;
    if (styles.has(cacheKey)) return copy(styles.get(cacheKey));
    const names = new Set(className.split(/\s+/).filter(Boolean));
    const matched = [];
    for (const name of names) {
      if (!byName.has(name)) throw Error(`UI_CLASS: ${name} was not compiled; use complete utility names in source`);
      matched.push(...byName.get(name));
    }
    matched.sort((a, b) => Number(!!a.when?.focus) - Number(!!b.when?.focus) || a.order - b.order);
    const style = {}, gradient = {}, shadow = {}, backdrop = {};
    for (const rule of matched) {
      if (rule.error) throw Error(`UI_CLASS: ${rule.name}: ${rule.error}`);
      const { theme: theme2, minWidth, maxWidth, focus } = rule.when;
      if (focus && !focused) continue;
      if (theme2 && (env.theme ?? "light") !== theme2) continue;
      if (minWidth !== void 0 && !(env.viewport?.width >= minWidth)) continue;
      if (maxWidth !== void 0 && !(env.viewport?.width < maxWidth)) continue;
      Object.assign(style, copy(rule.style));
      Object.assign(gradient, rule.gradient);
      Object.assign(shadow, rule.shadow);
      Object.assign(backdrop, rule.backdrop);
    }
    const filter = backdrop.mode === "fixed" ? backdrop.fixed : backdrop.mode === "variables" ? backdrop : void 0;
    if (filter && (filter.blur != null || filter.saturation != null)) {
      style.backdropBlur = filter.blur ?? 0;
      style.backdropSaturation = filter.saturation ?? 1;
    }
    if (gradient.enabled !== void 0) {
      style.backgroundGradient = "none";
      if (gradient.enabled && gradient.source && (gradient.source !== "via" || gradient.via)) {
        const stops = [{ offset: gradient.fromOffset ?? 0, color: gradient.fromColor ?? "#00000000" }];
        if (gradient.via) stops.push({ offset: gradient.viaOffset ?? 0.5, color: gradient.viaColor ?? "#00000000" });
        stops.push({ offset: gradient.toOffset ?? 1, color: gradient.toColor ?? "#00000000" });
        for (let i = 1; i < stops.length; i++) stops[i].offset = Math.max(stops[i - 1].offset, stops[i].offset);
        style.backgroundGradient = { ...gradient.orientation ?? { direction: "bottom" }, colorSpace: gradient.colorSpace ?? "srgb", stops };
      }
    }
    if (shadow.enabled) {
      style.boxShadow = (shadow.composite === false ? shadow.fixedLayers ?? [] : shadow.layers ?? []).map(({ useColor, ...layer }) => ({
        ...layer,
        color: useColor && shadow.color !== void 0 ? shadow.color : layer.color
      }));
      if (shadow.composite !== false && shadow.ringWidth > 0) style.boxShadow.unshift({ offsetX: 0, offsetY: 0, blur: 0, spread: shadow.ringWidth, color: shadow.ringColor ?? "currentColor" });
      if (style.boxShadow.length > 8) throw Error("UI_STYLE: too many box shadows");
    }
    if (styles.size >= 256) styles.delete(styles.keys().next().value);
    styles.set(cacheKey, copy(style));
    return style;
  }
  function resolveStyle(className, inline, env, inherited = { fontSize: 14 }, focused = false) {
    const style = classStyle(className, env, focused);
    if (inline?.padding !== void 0 && className !== void 0)
      for (const side of ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"]) style[side] = inline.padding;
    if (inline?.borderWidth !== void 0 && className !== void 0)
      for (const side of ["borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth"]) style[side] = inline.borderWidth;
    if (inline?.radius !== void 0 && className !== void 0)
      for (const corner of ["radiusTopLeft", "radiusTopRight", "radiusBottomRight", "radiusBottomLeft"]) style[corner] = inline.radius;
    Object.assign(style, inline);
    const typography = {
      fontSize: style.fontSize ?? inherited.fontSize,
      lineHeight: style.lineHeight ?? inherited.lineHeight
    };
    if (typography.lineHeight && typeof typography.lineHeight === "object") {
      if (inline?.lineHeight !== void 0) throw Error("UI_STYLE_VALUE: lineHeight");
      style.lineHeight = typography.fontSize * typography.lineHeight.em;
    }
    if (style.letterSpacing && typeof style.letterSpacing === "object") {
      if (inline?.letterSpacing !== void 0) throw Error("UI_STYLE_VALUE: letterSpacing");
      style.letterSpacing = typography.fontSize * style.letterSpacing.em;
    }
    return { style, typography };
  }
  function focusStyle(className, inline, env, inherited) {
    if (!className || !className.split(/\s+/).some((name) => byName.get(name)?.some((rule) => rule.when?.focus))) return void 0;
    const base = resolveStyle(className, inline, env, inherited).style;
    const active = resolveStyle(className, inline, env, inherited, true).style;
    const result = {};
    for (const [key, value] of Object.entries(active)) if (JSON.stringify(value) !== JSON.stringify(base[key])) {
      if (!["color", "background", "opacity", "borderColor", "backgroundGradient", "boxShadow", "placeholderColor"].includes(key)) throw Error(`UI_STYLE: focus does not support ${key}`);
      result[key] = value;
    }
    return Object.keys(result).length ? result : void 0;
  }

  // src/runtime/stream.js
  function createStream(id, cancel, report, error) {
    let done = false, failure, finalValue, iterator = false, bytes = 0;
    const queue = [], reads = [];
    const listeners = { data: /* @__PURE__ */ new Set(), end: /* @__PURE__ */ new Set(), error: /* @__PURE__ */ new Set() };
    let resolve, reject;
    const result = new Promise((yes, no) => {
      resolve = yes;
      reject = no;
    });
    result.catch(() => {
    });
    const emit = (kind, value) => {
      for (const fn of [...listeners[kind]]) {
        try {
          fn(value);
        } catch (cause) {
          report(cause);
        }
      }
    };
    const finish = (ok, value) => {
      if (done) return;
      done = true;
      if (ok) {
        finalValue = value;
        resolve(value);
      } else {
        failure = value;
        queue.length = 0;
        bytes = 0;
        reject(value);
      }
      for (const read of reads.splice(0)) {
        if (ok) read.resolve({ done: true, value });
        else read.reject(value);
      }
      emit(ok ? "end" : "error", value);
      for (const bucket of Object.values(listeners)) bucket.clear();
    };
    const handle = Object.freeze({
      id,
      result,
      on(kind, fn) {
        if (!Object.prototype.hasOwnProperty.call(listeners, kind) || typeof fn !== "function")
          throw Error("UI_STREAM: invalid listener");
        if (!done) {
          if (listeners[kind].size >= 256) throw Error("UI_LIMIT: too many stream listeners");
          listeners[kind].add(fn);
        }
        return handle;
      },
      cancel() {
        if (!done) cancel();
      },
      [Symbol.asyncIterator]() {
        if (iterator) throw Error("UI_STREAM: only one iterator is allowed");
        iterator = true;
        return {
          next() {
            if (queue.length) {
              const item = queue.shift();
              bytes -= item.bytes;
              return Promise.resolve({ done: false, value: item.value });
            }
            if (done) return failure ? Promise.reject(failure) : Promise.resolve({ done: true, value: finalValue });
            if (reads.length >= 256) return Promise.reject(error("UI_LIMIT", "Too many pending stream reads"));
            return new Promise((resolve2, reject2) => reads.push({ resolve: resolve2, reject: reject2 }));
          },
          return() {
            queue.length = 0;
            bytes = 0;
            handle.cancel();
            return Promise.resolve({ done: true, value: void 0 });
          }
        };
      }
    });
    return {
      handle,
      resolve: (value) => finish(true, value),
      reject: (cause) => finish(false, cause),
      push(value) {
        if (done) return;
        if (reads.length) reads.shift().resolve({ done: false, value });
        else if (iterator) {
          const size = JSON.stringify(value).length * 2;
          if (queue.length >= 256 || bytes + size > 1048576) {
            cancel(error("UI_LIMIT", "Stream consumer queue exceeded"));
            return;
          }
          queue.push({ value, bytes: size });
          bytes += size;
        }
        emit("data", value);
      }
    };
  }

  // src/runtime/channel.js
  function createChannel(id, send, close, report, error) {
    let closed = false, failure, iterator = false, bytes = 0;
    const queue = [], reads = [];
    const listeners = { data: /* @__PURE__ */ new Set(), close: /* @__PURE__ */ new Set(), error: /* @__PURE__ */ new Set() };
    const invoke = (fn, ...values) => {
      try {
        fn(...values);
      } catch (cause) {
        report(cause);
      }
    };
    const finish = (cause, code, reason, discard2 = false) => {
      if (closed) return;
      closed = true;
      failure = cause;
      if (cause || discard2) {
        queue.length = 0;
        bytes = 0;
      }
      for (const read of reads.splice(0)) {
        if (cause) read.reject(cause);
        else read.resolve({ done: true, value: void 0 });
      }
      if (cause) for (const fn of [...listeners.error]) invoke(fn, cause);
      for (const fn of [...listeners.close]) invoke(fn, code, reason);
      for (const bucket of Object.values(listeners)) bucket.clear();
    };
    const handle = Object.freeze({
      id,
      send(payload) {
        if (closed) throw error("STREAM_CLOSED", "Channel is closed");
        send(payload);
      },
      close(code = "done", reason = "") {
        if (!closed) close(code, reason);
      },
      on(kind, fn) {
        if (!Object.prototype.hasOwnProperty.call(listeners, kind) || typeof fn !== "function")
          throw Error("UI_CHANNEL: invalid listener");
        if (!closed) {
          if (listeners[kind].size >= 256) throw Error("UI_LIMIT: too many channel listeners");
          listeners[kind].add(fn);
          if (kind === "data" && !iterator && queue.length) {
            const buffered = queue.splice(0);
            bytes = 0;
            for (const item of buffered) {
              if (closed) break;
              invoke(fn, item.value);
            }
          }
        }
        return handle;
      },
      [Symbol.asyncIterator]() {
        if (iterator) throw Error("UI_CHANNEL: only one iterator is allowed");
        iterator = true;
        return {
          next() {
            if (queue.length) {
              const item = queue.shift();
              bytes -= item.bytes;
              return Promise.resolve({ done: false, value: item.value });
            }
            if (closed) return failure ? Promise.reject(failure) : Promise.resolve({ done: true, value: void 0 });
            if (reads.length >= 256) return Promise.reject(error("UI_LIMIT", "Too many pending channel reads"));
            return new Promise((resolve, reject) => reads.push({ resolve, reject }));
          },
          return() {
            handle.close();
            return Promise.resolve({ done: true, value: void 0 });
          }
        };
      }
    });
    return {
      handle,
      finish: (code, reason, discard2) => finish(void 0, code, reason, discard2),
      reject: (cause) => finish(cause, cause.code, cause.message),
      push(value) {
        if (closed) return;
        if (reads.length) reads.shift().resolve({ done: false, value });
        else if (iterator || !listeners.data.size) {
          const size = JSON.stringify(value).length * 2;
          if (queue.length >= 256 || bytes + size > 1048576) {
            close("UI_LIMIT", "Channel consumer queue exceeded", error("UI_LIMIT", "Channel consumer queue exceeded"));
            return;
          }
          queue.push({ value, bytes: size });
          bytes += size;
        }
        for (const fn of [...listeners.data]) {
          if (closed) break;
          invoke(fn, value);
        }
      }
    };
  }

  // src/runtime/channel-resource.js
  function channelResource(state, report) {
    let definition, channel, ticket = 0, mounted = false;
    let snapshot = state.value;
    const update = (patch) => {
      snapshot = { ...snapshot, ...patch };
      state.set(snapshot);
    };
    const close = () => {
      ticket++;
      const old = channel;
      channel = void 0;
      update({ connecting: false, connected: false });
      old?.close();
    };
    const connect = async () => {
      if (!mounted || snapshot.connecting || snapshot.connected) return;
      const current = ++ticket;
      update({ connecting: true, connected: false, error: null });
      try {
        const opened = await definition.channels.open(definition.topic, definition.params);
        if (!mounted || current !== ticket) {
          opened.close();
          return;
        }
        channel = opened;
        update({ connecting: false, connected: true });
        opened.on("data", (value) => {
          if (mounted && current === ticket) definition.onData(value);
        });
        opened.on("error", (error) => {
          if (mounted && current === ticket) update({ error: String(error.message ?? error) });
        });
        opened.on("close", () => {
          if (mounted && current === ticket) {
            channel = void 0;
            update({ connecting: false, connected: false });
          }
        });
      } catch (error) {
        if (mounted && current === ticket) {
          update({ connecting: false, connected: false, error: String(error.message ?? error) });
        }
      }
    };
    return {
      get state() {
        return snapshot;
      },
      connect,
      close,
      send(value) {
        if (!mounted || !channel) throw Error("UI_CHANNEL: resource is disconnected");
        channel.send(value);
      },
      commit(next) {
        const changed = !definition || definition.topic !== next.topic || definition.key !== next.key || definition.channels !== next.channels;
        definition = next;
        if (changed) {
          try {
            close();
          } catch (error) {
            report(error);
          }
          mounted = true;
          connect().catch(report);
        }
      },
      dispose() {
        mounted = false;
        try {
          close();
        } catch (error) {
          report(error);
        }
      }
    };
  }

  // src/runtime/leave-guard.js
  function createLeaveGuards(callHost, report) {
    const holders = /* @__PURE__ */ new Set();
    let epoch2 = 0, sent = false, asking = false, tail = Promise.resolve();
    const sync = () => {
      const current = epoch2;
      tail = tail.then(async () => {
        if (current !== epoch2) return;
        const want = holders.size > 0;
        if (want === sent) return;
        try {
          await callHost("navigation.setLeaveGuard", { enabled: want });
          if (current === epoch2) sent = want;
        } catch (error) {
          if (current !== epoch2) return;
          for (const holder of holders) holder.fail(error);
          report(error);
        }
      });
    };
    const request = (reason) => {
      if (!["back", "home"].includes(reason)) throw Error("UI_LEAVE: invalid reason");
      if (asking) return;
      asking = true;
      const current = epoch2, pending = [...holders].reverse();
      (async () => {
        try {
          for (const holder of pending) {
            if (current !== epoch2 || !holders.has(holder) || !await holder.ask(reason)) return;
          }
          if (current === epoch2) await callHost("navigation.leave", { reason });
        } catch (error) {
          if (current === epoch2) report(error);
        } finally {
          if (current === epoch2) asking = false;
        }
      })();
    };
    return {
      request,
      reset() {
        epoch2++;
        for (const holder of holders) holder.cancel();
        holders.clear();
        sent = false;
        asking = false;
        tail = Promise.resolve();
      },
      resource(state) {
        let snapshot = state.value, answer, enabled = false, mounted = false, registeredEpoch = epoch2;
        const update = (patch) => {
          snapshot = { ...snapshot, ...patch };
          state.set(snapshot);
        };
        const decide = (leave) => {
          if (typeof leave !== "boolean") throw Error("UI_LEAVE: expected boolean answer");
          const resolve = answer;
          answer = void 0;
          if (!resolve) return;
          update({ pending: null });
          resolve(leave);
        };
        const holder = {
          cancel() {
            decide(false);
          },
          ask(reason) {
            if (!enabled) return Promise.resolve(false);
            update({ pending: reason, requests: snapshot.requests + 1 });
            return new Promise((resolve) => {
              answer = resolve;
            });
          },
          fail(error) {
            update({ error: String(error.message ?? error) });
          }
        };
        return {
          get state() {
            return snapshot;
          },
          answer: decide,
          request(reason) {
            if (!mounted || registeredEpoch !== epoch2) throw Error("UI_LEAVE: stale guard");
            request(reason);
          },
          commit(next) {
            mounted = true;
            if (enabled === next.enabled && registeredEpoch === epoch2) return;
            registeredEpoch = epoch2;
            enabled = next.enabled;
            if (enabled) {
              update({ error: null });
              holders.add(holder);
            } else {
              decide(false);
              holders.delete(holder);
            }
            sync();
          },
          dispose() {
            mounted = false;
            enabled = false;
            decide(false);
            if (holders.delete(holder)) sync();
          }
        };
      }
    };
  }

  // src/runtime/tree.js
  var arena = [];
  var epoch = 0;
  var nextHandle = 1;
  var brand = Symbol("ui candidate");
  var bindings = /* @__PURE__ */ new WeakMap();
  var componentIds = /* @__PURE__ */ new WeakMap();
  var nextComponent = 1;
  var locals = /* @__PURE__ */ new Map();
  var callbacks = /* @__PURE__ */ new Map();
  var updates = /* @__PURE__ */ new Map();
  var committed = null;
  var lookupLocals = null;
  var localCollectors = [];
  var ownedResources = /* @__PURE__ */ new Map();
  var candidate = null;
  var scope = null;
  var currentView = null;
  var rendering = false;
  var actions = Object.freeze(/* @__PURE__ */ Object.create(null));
  var actionEpoch = 0;
  var nextRequest = 1;
  var requests = [];
  var actionErrors = [];
  var pendingActions = /* @__PURE__ */ new Map();
  var activeChannels = /* @__PURE__ */ new Map();
  var unaryActions = /* @__PURE__ */ new WeakSet();
  var streamActions = /* @__PURE__ */ new WeakSet();
  var actionNames = /* @__PURE__ */ new WeakMap();
  var channels = Object.freeze({ open() {
    throw Error("UI_CHANNEL: bridge not ready");
  } });
  var leaveGuards = createLeaveGuards(callLeaveHost, (error) => {
    if (actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
  });
  var has2 = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
  function bridgeError(code, message, data) {
    const error = new Error(message ?? code);
    error.name = "LxBridgeError";
    error.code = code;
    if (data !== void 0) error.data = data;
    return error;
  }
  function cancelActions() {
    actionEpoch++;
    leaveGuards.reset();
    const pending = [...pendingActions.values()];
    pendingActions.clear();
    requests = [];
    actions = Object.freeze(/* @__PURE__ */ Object.create(null));
    const oldChannels = [...activeChannels.values()];
    activeChannels.clear();
    channels = Object.freeze({ open() {
      throw Error("UI_CHANNEL: bridge not ready");
    } });
    for (const { reject } of pending) reject(bridgeError("BRIDGE_CANCELED", "Page reset"));
    for (const entry of oldChannels) {
      const error = bridgeError("BRIDGE_CANCELED", "Page reset");
      entry.reject(error);
      entry.channel.reject(error);
    }
  }
  function configureActions(metadata) {
    if (rendering) throw Error("UI_RENDER: configure actions during render");
    const names = metadata?.__names, modes = metadata?.__modes;
    if (!Array.isArray(names) || names.length > 256 || !modes || typeof modes !== "object" || names.some((name) => typeof name !== "string" || !name || name.length > 256 || !has2(modes, name) || !["call", "stream"].includes(modes[name])) || new Set(names).size !== names.length) throw Error("UI_ACTION: invalid action catalog");
    cancelActions();
    const epoch2 = actionEpoch, built = /* @__PURE__ */ Object.create(null), actionModes = { ...modes };
    for (const name of names) built[name] = (...args) => {
      if (rendering) throw Error("UI_RENDER: action during render");
      if (epoch2 !== actionEpoch) throw bridgeError("BRIDGE_CANCELED", "Stale page action");
      if (args.length > 1) throw Error("UI_ACTION: expected at most one JSON payload");
      if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256) throw Error("UI_LIMIT: too many pending actions");
      const params = args.length && args[0] !== void 0 ? immutable(args[0]) : void 0;
      const id = `ui-action:${nextRequest++}`;
      const frame = { v: 2, kind: "req", id, method: name, cap: "page" };
      if (params !== void 0) frame.params = params;
      const encoded = JSON.stringify(frame);
      if (encoded.length > 65536) throw Error("UI_LIMIT: action payload too large");
      if (actionModes[name] === "stream") {
        const stream = createStream(id, (cause) => {
          if (!pendingActions.has(id)) return;
          if (requests.length >= 512) throw Error("UI_LIMIT: too many pending cancellations");
          requests.push(JSON.stringify({ v: 2, kind: "cancel", id }));
          pendingActions.delete(id);
          stream.reject(cause ?? bridgeError("BRIDGE_CANCELED", "Stream canceled"));
        }, (cause) => {
          if (epoch2 === actionEpoch && actionErrors.length < 32) actionErrors.push(String(cause).slice(0, 2048));
        }, bridgeError);
        pendingActions.set(id, { ...stream, sequence: -1 });
        requests.push(encoded);
        return stream.handle;
      }
      const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
      promise.catch(() => {
      });
      requests.push(encoded);
      return promise;
    };
    for (const name of names) {
      if (actionModes[name] === "call") unaryActions.add(built[name]);
      else streamActions.add(built[name]);
      actionNames.set(built[name], name);
    }
    actions = Object.freeze(built);
    channels = Object.freeze({
      open(topic, params) {
        if (rendering) throw Error("UI_RENDER: channel during render");
        if (epoch2 !== actionEpoch) throw bridgeError("BRIDGE_CANCELED", "Stale page channel");
        if (typeof topic !== "string" || !has2(built, topic) || topic.includes(".") || actionModes[topic] !== "call")
          throw Error("UI_CHANNEL: expected a page channel handler");
        if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256)
          throw Error("UI_LIMIT: too many pending channels");
        const id = `ui-channel:${nextRequest++}`;
        const frame = { v: 2, kind: "ch.open", id, topic, cap: "page" };
        if (params !== void 0) frame.params = immutable(params);
        const encoded = JSON.stringify(frame);
        if (encoded.length > 65536) throw Error("UI_LIMIT: channel payload too large");
        let outgoing = 0;
        const channel = createChannel(id, (payload) => {
          if (rendering) throw Error("UI_RENDER: channel send during render");
          const entry = activeChannels.get(id);
          if (epoch2 !== actionEpoch || !entry?.open) throw bridgeError("STREAM_CLOSED", "Channel is closed");
          if (requests.length >= 256) throw Error("UI_LIMIT: too many pending channel frames");
          const message = JSON.stringify({ v: 2, kind: "ch.data", id, seq: outgoing, payload: immutable(payload) });
          if (message.length > 65536) throw Error("UI_LIMIT: channel payload too large");
          requests.push(message);
          outgoing++;
        }, (code, reason, cause) => {
          if (rendering) throw Error("UI_RENDER: channel close during render");
          if (!activeChannels.has(id)) return;
          if (typeof code !== "string" || code.length > 256 || typeof reason !== "string" || reason.length > 2048)
            throw Error("UI_CHANNEL: invalid close reason");
          if (requests.length >= 512) throw Error("UI_LIMIT: too many pending cancellations");
          requests.push(JSON.stringify({ v: 2, kind: "ch.close", id, code, reason }));
          activeChannels.delete(id);
          if (cause) channel.reject(cause);
          else channel.finish(code, reason, true);
        }, (cause) => {
          if (epoch2 === actionEpoch && actionErrors.length < 32) actionErrors.push(String(cause).slice(0, 2048));
        }, bridgeError);
        const promise = new Promise((resolve, reject) => activeChannels.set(id, { channel, resolve, reject, open: false, sequence: -1 }));
        promise.catch(() => {
        });
        requests.push(encoded);
        return promise;
      }
    });
  }
  function callLeaveHost(method, params) {
    if (rendering) throw Error("UI_RENDER: leave control during render");
    if (!["navigation.setLeaveGuard", "navigation.leave"].includes(method)) throw Error("UI_LEAVE: invalid control");
    if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256) throw Error("UI_LIMIT: too many pending actions");
    const id = `ui-action:${nextRequest++}`;
    const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
    promise.catch(() => {
    });
    requests.push(JSON.stringify({ v: 2, kind: "req", id, method: `host.${method}`, cap: "host", params }));
    return promise;
  }
  function requestLeave(reason) {
    if (rendering) throw Error("UI_RENDER: leave request during render");
    leaveGuards.request(reason);
  }
  function takeActionRequests() {
    const result = requests;
    requests = [];
    return result;
  }
  function takeActionErrors() {
    const result = actionErrors;
    actionErrors = [];
    return result;
  }
  function receiveActionResponse(frame) {
    if (rendering) throw Error("UI_RENDER: response during render");
    if (frame?.v === 2 && ["ch.ack", "ch.data", "ch.close"].includes(frame.kind)) return receiveChannelFrame(frame);
    if (frame?.v !== 2 || !["res", "event"].includes(frame.kind) || typeof frame.id !== "string" || frame.kind === "res" && typeof frame.ok !== "boolean" || frame.kind === "event" && (!Number.isSafeInteger(frame.seq) || frame.seq < 0))
      throw Error("UI_ACTION: invalid response");
    const pending = pendingActions.get(frame.id);
    if (!pending) return false;
    if (frame.kind === "event") {
      if (!pending.push) throw Error("UI_STREAM: event for unary action");
      if (frame.seq <= pending.sequence || pending.sequence === -1 && frame.seq !== 0)
        throw Error("UI_STREAM: invalid sequence");
      const value2 = immutable(frame.payload ?? null);
      pending.sequence = frame.seq;
      pending.push(value2);
      return true;
    }
    const value = frame.ok ? immutable(frame.result ?? null) : bridgeError(
      typeof frame.error?.code === "string" ? frame.error.code : "BRIDGE_INTERNAL_ERROR",
      typeof frame.error?.message === "string" ? frame.error.message : void 0,
      frame.error?.data === void 0 ? void 0 : immutable(frame.error.data)
    );
    pendingActions.delete(frame.id);
    if (frame.ok) pending.resolve(value);
    else pending.reject(value);
    return true;
  }
  function receiveChannelFrame(frame) {
    if (typeof frame.id !== "string" || frame.kind === "ch.ack" && typeof frame.ok !== "boolean" || frame.kind === "ch.data" && (!Number.isSafeInteger(frame.seq) || frame.seq < 0) || frame.kind === "ch.close" && (frame.code !== void 0 && typeof frame.code !== "string" || frame.reason !== void 0 && typeof frame.reason !== "string"))
      throw Error("UI_CHANNEL: invalid frame");
    const entry = activeChannels.get(frame.id);
    if (!entry) return false;
    if (frame.kind === "ch.ack") {
      if (entry.open) throw Error("UI_CHANNEL: duplicate acknowledgement");
      if (frame.ok) {
        entry.open = true;
        entry.resolve(entry.channel.handle);
      } else {
        const cause = bridgeError(
          frame.error?.code ?? "BRIDGE_INTERNAL_ERROR",
          frame.error?.message,
          frame.error?.data === void 0 ? void 0 : immutable(frame.error.data)
        );
        activeChannels.delete(frame.id);
        entry.reject(cause);
        entry.channel.reject(cause);
      }
      return true;
    }
    if (!entry.open) throw Error("UI_CHANNEL: frame before acknowledgement");
    if (frame.kind === "ch.close") {
      activeChannels.delete(frame.id);
      entry.channel.finish(frame.code, frame.reason);
    } else {
      if (frame.seq <= entry.sequence || entry.sequence === -1 && frame.seq !== 0)
        throw Error("UI_CHANNEL: invalid sequence");
      const value = immutable(frame.payload ?? null);
      entry.sequence = frame.seq;
      entry.channel.push(value);
    }
    return true;
  }
  function immutable(value, depth = 0, omitUndefined = false) {
    if (depth > 64) throw Error("UI_LOCAL: value too deep");
    if (value === null || typeof value === "string" || typeof value === "boolean") return value;
    if (typeof value === "number" && Number.isFinite(value)) return value;
    if (Array.isArray(value)) return Object.freeze(value.map((v) => immutable(v, depth + 1, omitUndefined)));
    if (value && Object.getPrototypeOf(value) === Object.prototype) {
      return Object.freeze(Object.fromEntries(Object.entries(value).filter(([, v]) => !omitUndefined || v !== void 0).map(([k, v]) => [k, immutable(v, depth + 1, omitUndefined)])));
    }
    throw Error("UI_LOCAL: expected JSON value");
  }
  function useView() {
    if (!rendering) throw Error("UI_SCOPE: useView outside render");
    return currentView;
  }
  function createView(data, env) {
    return Object.freeze({ data: immutable(data), env: immutable(env), actions, channels });
  }
  function local(name, initial, resource = "") {
    if (!rendering || scope === null || typeof name !== "string" || !name || name.length > 256) throw Error("UI_SCOPE: invalid local state");
    const key = JSON.stringify(resource ? [scope, resource, name] : [scope, name]);
    if (!resource && candidate.declarations.has(key)) throw Error(`UI_LOCAL: duplicate declaration ${JSON.stringify(name)} in ${scope}`);
    candidate.declarations.add(key);
    let cell = candidate.locals.get(key) ?? lookupLocals?.get(key) ?? locals.get(key);
    if (!cell) cell = { value: immutable(initial, 0, true) };
    candidate.locals.set(key, cell);
    for (const collector of localCollectors) collector.set(key, cell);
    const binding = Object.freeze({
      get value() {
        return updates.has(cell) ? updates.get(cell) : cell.value;
      },
      set(value) {
        if (rendering) throw Error("UI_RENDER: state mutation during render");
        if (locals.get(key) !== cell) return;
        updates.set(cell, immutable(value, 0, true));
      }
    });
    bindings.set(binding, { key, cell });
    return binding;
  }
  function useLocal(name, initial) {
    return local(name, initial);
  }
  function hasLocalUpdates() {
    return updates.size > 0;
  }
  function resourceAction(method, kind, registered) {
    const catalog = currentView.actions, names = Object.keys(catalog);
    const label = kind === "UI_STREAM" ? "streaming" : "unary";
    if (method === void 0 && !names.length) return void 0;
    if (typeof method !== "function" && (typeof method !== "string" || !method || method.includes(".")))
      throw Error(`${kind}: expected a page action name or action reference`);
    const action = typeof method === "function" ? method : catalog[method];
    if (action !== void 0 && (!registered.has(action) || !has2(catalog, actionNames.get(action)) || catalog[actionNames.get(action)] !== action || actionNames.get(action).includes(".")))
      throw Error(`${kind}: expected a ${label} page action`);
    if (action === void 0 && names.length) throw Error(`${kind}: unknown page action`);
    return action;
  }
  function useStream(name, method, options) {
    const state = local(name, { status: "idle", error: null, result: null }, "stream");
    if (typeof options?.onData !== "function")
      throw Error("UI_STREAM: expected a page stream name and onData");
    const action = resourceAction(method, "UI_STREAM", streamActions);
    const { cell } = bindings.get(state);
    if (candidate.resourceNames.has(cell)) throw Error("UI_STREAM: duplicate resource name");
    candidate.resourceNames.add(cell);
    const resource = ownedResources.get(cell) ?? streamResource(state);
    candidate.resources.set(cell, { resource, definition: { method, action, onData: options.onData } });
    const guarded = (fn) => (...args) => {
      if (rendering) throw Error("UI_RENDER: stream mutation during render");
      return fn(...args);
    };
    return Object.freeze({
      get status() {
        return resource.state.status;
      },
      get error() {
        return resource.state.error;
      },
      get result() {
        return resource.state.result;
      },
      start: guarded(resource.start),
      cancel: guarded(resource.cancel)
    });
  }
  function useRequest(name, method, params) {
    const state = local(name, { pending: false, value: null, error: null }, "request");
    const action = resourceAction(method, "UI_REQUEST", unaryActions);
    const payload = params === void 0 ? void 0 : immutable(params);
    const { cell } = bindings.get(state);
    if (candidate.resourceNames.has(cell)) throw Error("UI_REQUEST: duplicate resource name");
    candidate.resourceNames.add(cell);
    const resource = ownedResources.get(cell) ?? requestResource(state);
    candidate.resources.set(cell, { resource, definition: { method, action, params: payload, key: JSON.stringify(payload) } });
    return Object.freeze({
      get pending() {
        return resource.state.pending;
      },
      get value() {
        return resource.state.value;
      },
      get error() {
        return resource.state.error;
      },
      reload() {
        if (rendering) throw Error("UI_RENDER: request reload during render");
        return resource.reload();
      }
    });
  }
  function useLeaveGuard(name, enabled) {
    if (typeof enabled !== "boolean") throw Error("UI_LEAVE: expected boolean enabled");
    const state = local(name, { pending: null, requests: 0, error: null }, "leave");
    const { cell } = bindings.get(state);
    if (candidate.resourceNames.has(cell)) throw Error("UI_LEAVE: duplicate resource name");
    candidate.resourceNames.add(cell);
    const resource = ownedResources.get(cell) ?? leaveGuards.resource(state);
    candidate.resources.set(cell, { resource, definition: { enabled } });
    const guarded = (fn) => (...args) => {
      if (rendering) throw Error("UI_RENDER: leave mutation during render");
      return fn(...args);
    };
    return Object.freeze({
      get pending() {
        return resource.state.pending;
      },
      get requests() {
        return resource.state.requests;
      },
      get error() {
        return resource.state.error;
      },
      answer: guarded(resource.answer),
      request: guarded(resource.request)
    });
  }
  function useChannel(name, topic, options) {
    const state = local(name, { connecting: false, connected: false, error: null }, "channel");
    if (typeof topic !== "string" || !topic || typeof options?.onData !== "function")
      throw Error("UI_CHANNEL: expected topic and onData");
    const { cell } = bindings.get(state);
    if (candidate.resourceNames.has(cell)) throw Error("UI_CHANNEL: duplicate resource name");
    candidate.resourceNames.add(cell);
    const params = options.params === void 0 ? void 0 : immutable(options.params);
    const definition = { topic, params, key: JSON.stringify(params), onData: options.onData, channels: currentView.channels };
    const resource = ownedResources.get(cell) ?? channelResource(state, (error) => {
      if (actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
    });
    candidate.resources.set(cell, { definition, resource });
    const guarded = (callback) => (...args) => {
      if (rendering) throw Error("UI_RENDER: channel resource mutation during render");
      return callback(...args);
    };
    return Object.freeze({
      get connecting() {
        return resource.state.connecting;
      },
      get connected() {
        return resource.state.connected;
      },
      get error() {
        return resource.state.error;
      },
      connect: guarded(resource.connect),
      close: guarded(resource.close),
      send: guarded(resource.send)
    });
  }
  function childrenToArray(children) {
    return flatten([children]);
  }
  function List({ items, getKey, renderItem, getItemHeight, ...props }) {
    return h("list", {
      ...props,
      count: items.length,
      itemKeys: items.map(getKey),
      row: (index) => renderItem(items[index], index),
      ...getItemHeight ? { height: (index) => getItemHeight(items[index], index) } : {}
    });
  }
  function h(tag, props, ...children) {
    if (!rendering) throw Error("UI_SCOPE: JSX outside render");
    if (tag === "view") tag = "row";
    const handle = { [brand]: [epoch, arena.length] };
    if (arena.length >= 16384) throw Error("UI_LIMIT: candidate arena too large");
    const { children: suppliedChildren, ...attributes } = props ?? {};
    arena.push([tag, attributes, children.length ? children : suppliedChildren === void 0 ? [] : [suppliedChildren]]);
    return handle;
  }
  function flatten(children, depth = 0, result = []) {
    if (depth > 128) throw Error("UI_LIMIT: children too deep");
    for (const child of children) {
      if (result.length >= 16384) throw Error("UI_LIMIT: too many children");
      if (Array.isArray(child)) flatten(child, depth + 1, result);
      else if (child !== null && child !== void 0 && typeof child !== "boolean") result.push(child);
    }
    return result;
  }
  function sliderValue({ min, max, step }, value) {
    value = Math.max(min, Math.min(max, value));
    const snapped = Math.max(min, Math.min(max, min + Math.floor((value - min) / step + 0.5) * step));
    return Math.abs(max - value) < Math.abs(snapped - value) ? max : snapped;
  }
  function validInputText(value) {
    return typeof value === "string" && utf8Bytes(value, 65536) <= 65536;
  }
  function utf8Bytes(value, limit) {
    if (value.length > limit) return Infinity;
    if (/^[\x00-\x7f]*$/.test(value)) return value.length;
    let bytes = 0;
    for (const char of value) {
      const c = char.codePointAt(0);
      if (c >= 55296 && c <= 57343) return Infinity;
      bytes += c < 128 ? 1 : c < 2048 ? 2 : c < 65536 ? 3 : 4;
      if (bytes > limit) return Infinity;
    }
    return bytes;
  }
  function resolveNode(node, identity, depth = 0, parentTag = null, inherited, buttonAncestor = false) {
    if (depth > 128) throw Error("UI_LIMIT: children too deep");
    if (node == null || typeof node === "boolean") return null;
    if (typeof node === "string" || typeof node === "number") node = h("text", {}, node);
    if (!node || typeof node !== "object" || !(brand in node)) throw Error("UI_ROOT: expected one node");
    const [owner, offset] = node[brand];
    if (owner !== epoch || !arena[offset]) throw Error("UI_STALE: candidate from another render");
    const [tag, original, children] = arena[offset];
    if (typeof tag === "function") {
      if (!componentIds.has(tag)) componentIds.set(tag, nextComponent++);
      const previous = scope;
      scope = `${identity}/component:${componentIds.get(tag)}`;
      const componentChildren = children.length === 0 ? void 0 : children.length === 1 ? children[0] : children;
      try {
        return resolveNode(tag({ ...original, children: componentChildren }, currentView), scope, depth + 1, parentTag, inherited, buttonAncestor);
      } catch (error) {
        throw new Error(`${error?.message ?? error}
  at component ${tag.name || "<anonymous>"} (${identity})`, { cause: error });
      } finally {
        scope = previous;
      }
    }
    if (!has2(tags, tag) || tag === "radio-option") throw Error(`UI_TAG: ${tag}`);
    const { style, typography } = resolveStyle(original.className, original.style, currentView?.env, inherited);
    const props = { ...original, style };
    const input = tag === "input" || tag === "textarea" || tag === "search";
    const focusedStyle = focusStyle(original.className, original.style, currentView?.env, inherited);
    if (focusedStyle && !input) throw Error("UI_STYLE: focus styles require an editor");
    if (focusedStyle?.boxShadow !== void 0) focusedStyle.boxShadow = normalizeShadows(focusedStyle.boxShadow);
    if (focusedStyle?.backgroundGradient !== void 0) focusedStyle.backgroundGradient = normalizeGradient(focusedStyle.backgroundGradient);
    const check = tag === "checkbox" || tag === "switch";
    const radio = tag === "radio";
    const slider = tag === "slider";
    if (tag === "search" && (props.secure !== void 0 || props.keyboard !== void 0))
      throw Error("UI_INPUT: search owns its keyboard and cannot be secure");
    for (const key of Object.keys(props)) {
      if (!acceptsProp(tag, key)) throw Error(`UI_PROP: ${key}`);
    }
    for (const key of ["key", "testId"]) if (props[key] !== void 0 && typeof props[key] !== "string") throw Error(`UI_PROP: ${key}`);
    if (buttonAncestor && !["column", "row", "text", "spacer", "image", "grid", "stack", "vector"].includes(tag)) throw Error("UI_BUTTON: descendants must be noninteractive content");
    const flat = flatten(children), composed = (tag === "button" || tag === "checkbox") && flat.some((child) => !["string", "number"].includes(typeof child));
    if ((tag === "button" || tag === "checkbox") && (props.accessibilityLabel !== void 0 && (typeof props.accessibilityLabel !== "string" || !props.accessibilityLabel.trim() || props.accessibilityLabel.length > 1024 || props.accessibilityLabel.includes("\0")) || composed && (!props.accessibilityLabel || flat.some((child) => ["string", "number"].includes(typeof child))))) throw Error("UI_BUTTON: composed button requires an explicit label and no inline text");
    const leaf = tag === "text" || tag === "button" && !composed || check && !composed;
    if (leaf && flat.some((x) => !["string", "number"].includes(typeof x))) throw Error(`UI_CHILD: ${tag}`);
    if ((tag === "vector" || tag === "spacer" || tag === "list" || tag === "image" || tag === "plot" || input || radio || slider) && flat.length) throw Error(`UI_CHILD: ${tag}`);
    if (tag === "vector") validateVector(props);
    if (tag === "plot") {
      if (["smooth", "area"].some((name) => props[name] !== void 0 && typeof props[name] !== "boolean") || props.kind !== "line" && (props.smooth || props.area))
        throw Error("UI_PLOT: smooth and area must be booleans on line plots");
      if (typeof props.accessibilityLabel !== "string" || !props.accessibilityLabel.trim() || props.accessibilityLabel.length > 1024 || props.accessibilityLabel.includes("\0"))
        throw Error("UI_PLOT: accessibilityLabel is required");
      if (!["bar", "line", "pie"].includes(props.kind) || !Array.isArray(props.values) || props.values.length < 1 || props.values.length > 128 || props.values.some((v) => typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > 1e9 || props.kind === "pie" && v < 0))
        throw Error("UI_PLOT: invalid kind or values");
      if (props.markerEmphasisScale !== void 0 && (props.kind !== "line" || typeof props.markerEmphasisScale !== "number" || !Number.isFinite(props.markerEmphasisScale) || props.markerEmphasisScale < 1 || props.markerEmphasisScale > 4))
        throw Error("UI_PLOT: invalid marker emphasis scale");
      if (props.pieLayout !== void 0) {
        const layout = props.pieLayout;
        if (props.kind !== "pie" || !layout || typeof layout !== "object" || Array.isArray(layout) || Object.keys(layout).some((key) => !["center", "innerRadius", "outerRadius", "emphasisScale"].includes(key)) || !Array.isArray(layout.center) || layout.center.length !== 2 || [...layout.center, layout.innerRadius, layout.outerRadius].some((value) => typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1) || layout.innerRadius >= layout.outerRadius || layout.emphasisScale !== void 0 && (typeof layout.emphasisScale !== "number" || !Number.isFinite(layout.emphasisScale) || layout.emphasisScale < 0 || layout.emphasisScale > 64))
          throw Error("UI_PLOT: invalid pie layout");
      }
      if (props.domainMin !== void 0 || props.domainMax !== void 0) {
        const lo = props.domainMin, hi = props.domainMax;
        if (props.kind === "pie" || !Number.isFinite(lo) || !Number.isFinite(hi) || lo < -1e9 || hi > 1e9 || lo >= hi || lo > 0 || hi < 0 || props.values.some((value) => value < lo || value > hi))
          throw Error("UI_PLOT: domain must contain zero and all values");
      }
      if (props.onSelect !== void 0 && typeof props.onSelect !== "function")
        throw Error("UI_PLOT: onSelect must be a function");
      if (props.selectedIndex !== void 0 && (!Number.isInteger(props.selectedIndex) || props.selectedIndex < -1 || props.selectedIndex >= props.values.length))
        throw Error("UI_PLOT: invalid selected index");
      if (!Array.isArray(props.colors) || !props.colors.length || props.colors.length > 16 || props.colors.some((c) => typeof c !== "string" || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(c)))
        throw Error("UI_PLOT: expected 1..16 hex colors");
    }
    for (const [name, value] of Object.entries(props.style)) {
      if (name === "animation") {
        if (value !== null) {
          if (!value || typeof value !== "object" || Array.isArray(value)) throw Error("UI_ANIMATION: expected definition");
          const encoded = JSON.stringify(value, (_, item) => {
            if (typeof item === "number" && !Number.isFinite(item) || ["undefined", "function", "symbol", "bigint"].includes(typeof item)) throw Error("UI_ANIMATION: invalid payload value");
            return item;
          });
          if (utf8Bytes(encoded, 32768) > 32768) throw Error("UI_ANIMATION: wire budget exceeded");
          props.style.animation = JSON.parse(encoded);
        }
        continue;
      }
      if (name === "scrollAxis" && tag !== "scroll") throw Error("UI_SCROLL: scrollAxis requires a scroll node");
      if (name === "flexBasis") {
        const basis = typeof value === "string" && /^(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) ? Number(value.slice(0, -1)) : value;
        if (value !== "auto" && (typeof basis !== "number" || !Number.isFinite(basis) || basis < 0 || basis > 16384)) throw Error("UI_STYLE_VALUE: flexBasis");
        continue;
      }
      if (name === "height" && value === "auto") continue;
      if (has2(dimensionPercentFields, name)) {
        const length = typeof value === "string" && /^(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) ? Number(value.slice(0, -1)) : value;
        if (typeof length !== "number" || !Number.isFinite(length) || length < 0 || length > 16384) throw Error(`UI_STYLE_VALUE: ${name}`);
        continue;
      }
      if (name === "boxShadow") {
        props.style.boxShadow = normalizeShadows(value);
        continue;
      }
      if (name === "backgroundGradient") {
        props.style.backgroundGradient = normalizeGradient(value);
        continue;
      }
      if (name === "offset") {
        if (parentTag !== "stack" || !value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some((key) => key !== "x" && key !== "y")) throw Error("UI_STACK: offset requires a stack parent and x/y object");
        const x = value.x ?? 0, y = value.y ?? 0;
        if ([x, y].some((v) => typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > 16384) || value.x === null || value.y === null) throw Error("UI_STACK: invalid offset");
        props.style.offset = { x, y };
        continue;
      }
      if (name === "offsetX" || name === "offsetY") throw Error(`UI_STYLE: ${name}`);
      if (!has2(fields, name)) throw Error(`UI_STYLE: ${name}`);
      if (["radiusTopLeft", "radiusTopRight", "radiusBottomRight", "radiusBottomLeft", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth"].includes(name) && (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 16384)) throw Error(`UI_STYLE_VALUE: ${name}`);
      if ((name === "backdropBlur" || name === "backdropSaturation") && (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > (name === "backdropBlur" ? 64 : 16))) throw Error(`UI_STYLE_VALUE: ${name}`);
      if (name === "placeholderColor" && typeof value !== "string") throw Error("UI_STYLE_VALUE: placeholderColor");
      if (name === "opacity" && (typeof value !== "number" || !Number.isFinite(value) || value < 0 || value > 1)) throw Error("UI_STYLE_VALUE: opacity");
      if (name === "fontWeight" && (typeof value !== "number" || value < 1 || value > 1e3)) throw Error("UI_STYLE_VALUE: fontWeight");
      if (name === "letterSpacing" && (typeof value !== "number" || !Number.isFinite(value) || Math.abs(value) > 1024)) throw Error("UI_STYLE_VALUE: letterSpacing");
      if (name === "lineHeight" && (typeof value !== "number" || value <= 0 || value > 16384)) throw Error("UI_STYLE_VALUE: lineHeight");
      if (has2(styleOptions, name) && !styleOptions[name].includes(value)) throw Error(`UI_STYLE_VALUE: ${name}`);
      if (["flexDirection", "display"].includes(name) && !["row", "column"].includes(tag))
        throw Error("UI_LAYOUT: flex direction/display requires a flex container");
      if (tag === "list" && (name === "align" && value !== "stretch" || name === "justify" && value !== "start")) throw Error("UI_LIST: rows require stretch alignment and start justification");
      if (typeof value !== "string" && (typeof value !== "number" || !Number.isFinite(value))) throw Error(`UI_STYLE_VALUE: ${name}`);
    }
    if (tag === "text" && props.lines !== void 0 && (!Number.isInteger(props.lines) || props.lines < 1 || props.lines > 1024)) throw Error("UI_TEXT: lines must be an integer between 1 and 1024");
    if (tag === "grid" && (!Number.isInteger(props.columns) || props.columns < 1 || props.columns > 64)) throw Error("UI_GRID: columns must be an integer between 1 and 64");
    if (props.enabled !== void 0 && typeof props.enabled !== "boolean") throw Error("UI_PROP: enabled");
    if (props.onClick !== void 0 && typeof props.onClick !== "function") throw Error("UI_PROP: onClick");
    if (check) {
      if (props.bind !== void 0) {
        const local2 = bindings.get(props.bind);
        if (!local2 || candidate.locals.get(local2.key) !== local2.cell || typeof props.bind.value !== "boolean" || props.value !== void 0 || props.onChange !== void 0)
          throw Error("UI_CHECK: bind requires a local boolean cell without value/onChange");
        props.value = props.bind.value;
        props.onChange = (value) => props.bind.set(value);
      }
      if (props.value !== void 0 && typeof props.value !== "boolean") throw Error("UI_CHECK: value must be boolean");
      if (props.onChange !== void 0 && typeof props.onChange !== "function") throw Error("UI_CHECK: onChange");
    }
    if (slider) {
      if (props.bind !== void 0) {
        const local2 = bindings.get(props.bind);
        if (!local2 || candidate.locals.get(local2.key) !== local2.cell || typeof props.bind.value !== "number" || props.value !== void 0 || props.onChange !== void 0) throw Error("UI_SLIDER: bind requires a local number cell without value/onChange");
        props.value = props.bind.value;
        props.onChange = (value2) => props.bind.set(value2);
      }
      for (const [name, fallback] of [["min", 0], ["max", 100], ["step", 1]]) if (props[name] === void 0) props[name] = fallback;
      if (props.value === void 0) props.value = props.min;
      const { min, max, step, value } = props;
      if (![min, max, step, value].every((v) => typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= 1e9) || max <= min || step <= 0 || step > max - min || (max - min) / step > 1e6 || min + step === min || max - step === max) throw Error("UI_SLIDER: invalid range or precision");
      if (typeof props.accessibilityLabel !== "string" || !props.accessibilityLabel.trim() || props.accessibilityLabel.length > 1024 || props.accessibilityLabel.includes("\0")) throw Error("UI_SLIDER: accessibilityLabel is required");
      if (props.onChange !== void 0 && typeof props.onChange !== "function") throw Error("UI_SLIDER: onChange");
      props.value = sliderValue(props, value);
    }
    if (input) {
      if (props.submitOnEnter !== void 0 && (tag !== "textarea" || typeof props.submitOnEnter !== "boolean")) throw Error("UI_INPUT: submitOnEnter requires textarea boolean");
      if (tag === "search") {
        if (props.secure !== void 0 || props.keyboard !== void 0) throw Error("UI_INPUT: search owns its keyboard and cannot be secure");
        props.keyboard = "search";
      }
      if (props.bind !== void 0) {
        const local2 = bindings.get(props.bind);
        if (!local2 || candidate.locals.get(local2.key) !== local2.cell || props.value !== void 0 || props.onInput !== void 0) throw Error("UI_INPUT: bind requires a local string cell without value/onInput");
        props.value = props.bind.value;
        props.onInput = (value) => props.bind.set(value);
      }
      if (!validInputText(props.value === void 0 ? "" : props.value) || !validInputText(props.placeholder === void 0 ? "" : props.placeholder)) throw Error("UI_INPUT: invalid text");
      for (const name of ["secure", "autoFocus"]) if (props[name] !== void 0 && typeof props[name] !== "boolean") throw Error(`UI_INPUT: ${name}`);
      if (props.keyboard !== void 0 && !["text", "number", "email", "phone", "search"].includes(props.keyboard)) throw Error("UI_INPUT: keyboard");
      for (const name of ["onInput", "onSubmit"]) if (props[name] !== void 0 && typeof props[name] !== "function") throw Error(`UI_INPUT: ${name}`);
    }
    const result = { tag, props, typography, focusedStyle, identity, text: leaf ? flat.join("") : "", children: [] };
    if (tag === "plot" && props.onSelect !== void 0) result.plotSelect = [nextHandle++, (index) => {
      if (Number.isInteger(index) && index >= -1 && index < props.values.length) props.onSelect(index);
    }];
    if (props.onClick !== void 0 && props.enabled !== false) result.callback = [nextHandle++, props.onClick];
    if (check && props.onChange !== void 0 && props.enabled !== false) result.change = [nextHandle++, () => props.onChange(!(props.value ?? false))];
    if (tag === "image" && (typeof props.src !== "string" || !props.src.length || props.src.length > 8192 || props.src.includes("\0") || props.fit !== void 0 && !["cover", "contain", "fill"].includes(props.fit))) throw Error("UI_IMAGE: invalid source or fit");
    if (tag === "image" && props.alt !== void 0 && (typeof props.alt !== "string" || utf8Bytes(props.alt, 1024) > 1024 || props.alt.includes("\0") || props.alt.length > 0 && !props.alt.trim())) throw Error("UI_IMAGE: invalid alternative text");
    if (slider && props.onChange !== void 0 && props.enabled !== false) result.numberChange = [nextHandle++, (value) => {
      const next = sliderValue(props, value);
      if (next !== props.value) props.onChange(next);
    }];
    result.inputs = [];
    if (input && props.enabled !== false) for (const name of ["onInput", "onSubmit"]) {
      if (props[name] !== void 0) result.inputs.push([inputProps[name], nextHandle++, props[name]]);
    }
    result.reaches = [];
    if (tag === "image") {
      if (props.retryKey !== void 0 && (typeof props.retryKey !== "string" || utf8Bytes(props.retryKey, 257) > 256 || props.retryKey.includes("\0")))
        throw Error("UI_IMAGE: invalid retry key");
      for (const name of ["onLoad", "onError"]) if (props[name] !== void 0) {
        if (typeof props[name] !== "function") throw Error("UI_IMAGE: invalid callback");
        result.reaches.push([imageProps[name], nextHandle++, props[name]]);
      }
    }
    for (const name of Object.keys(reachProps)) {
      if (props[name] === void 0) continue;
      if (typeof props[name] !== "function") throw Error(`UI_PROP: ${name}`);
      result.reaches.push([reachProps[name], nextHandle++, props[name]]);
    }
    if (radio) {
      if (props.bind !== void 0) {
        const local2 = bindings.get(props.bind);
        if (!local2 || candidate.locals.get(local2.key) !== local2.cell || typeof props.bind.value !== "string" || props.value !== void 0 || props.onChange !== void 0) throw Error("UI_RADIO: bind requires a live string cell without value/onChange");
        props.value = props.bind.value;
        props.onChange = (value) => props.bind.set(value);
      }
      if (props.value !== void 0 && (typeof props.value !== "string" || props.value.length > 256) || props.onChange !== void 0 && typeof props.onChange !== "function") throw Error("UI_RADIO: invalid value/onChange");
      if (props.accessibilityLabel !== void 0 && (typeof props.accessibilityLabel !== "string" || props.accessibilityLabel.length > 1024 || props.accessibilityLabel.includes("\0"))) throw Error("UI_RADIO: invalid accessibility label");
      if (!Array.isArray(props.options) || !props.options.length || props.options.length > 64) throw Error("UI_RADIO: expected 1..64 options");
      const values = /* @__PURE__ */ new Set();
      result.children = props.options.map((option) => {
        if (!option || typeof option !== "object" || Object.keys(option).some((k) => !["value", "label", "enabled", "testId"].includes(k)) || typeof option.value !== "string" || !option.value.length || option.value.length > 256 || values.has(option.value) || typeof option.label !== "string" || option.enabled !== void 0 && typeof option.enabled !== "boolean" || option.testId !== void 0 && typeof option.testId !== "string") throw Error("UI_RADIO: invalid or duplicate option");
        values.add(option.value);
        const value = option.value, selected = props.value === value, enabled = props.enabled !== false && option.enabled !== false;
        const child = { tag: "radio-option", identity: `${identity}/radio/key:${JSON.stringify(value)}`, props: { key: value, testId: option.testId, value: selected, enabled, style: Object.fromEntries(Object.entries(props.style).filter(([name]) => ["fontSize", "fontWeight", "fontFamily", "lineHeight", "color", "background"].includes(name))) }, text: option.label, children: [] };
        if (enabled && props.onChange) child.change = [nextHandle++, () => {
          if (!selected) return props.onChange(value);
        }];
        return child;
      });
    } else if (tag === "list") {
      if (!Number.isSafeInteger(props.count) || props.count < 0 || props.count > 1e5 || typeof props.row !== "function" || props.height !== void 0 && typeof props.height !== "function") throw Error("UI_LIST: invalid row description");
      if (props.itemKeys !== void 0) {
        if (!Array.isArray(props.itemKeys) || props.itemKeys.length !== props.count) throw Error("UI_LIST: itemKeys must match count");
        const seen = /* @__PURE__ */ new Set();
        props.itemKeys = Array.from(props.itemKeys, (key) => {
          if (typeof key !== "string" || utf8Bytes(key, 256) > 256 || utf8Bytes(`key:${JSON.stringify(key)}`, 256) > 256 || seen.has(key)) throw Error("UI_LIST: invalid or duplicate itemKey");
          seen.add(key);
          return key;
        });
        if (utf8Bytes(JSON.stringify(props.itemKeys), 262144) > 262144) throw Error("UI_LIST: itemKeys budget exceeded");
      }
      if (props.anchor !== void 0 && props.anchor !== "end") {
        const a = props.anchor;
        if (!a || typeof a !== "object" || Object.keys(a).some((k) => !["index", "align", "seq"].includes(k)) || !Number.isSafeInteger(a.index) || a.index < 0 || !["start", "end"].includes(a.align) || a.seq !== void 0 && (!Number.isSafeInteger(a.seq) || a.seq < 0)) throw Error("UI_LIST: invalid anchor");
        props.anchor = { index: a.index, align: a.align, ...a.seq === void 0 ? {} : { seq: a.seq } };
      }
      const old = committed?.lists.get(identity);
      const anchor = props.anchor === void 0 ? null : JSON.stringify(props.anchor);
      const positions = props.itemKeys && new Map(props.itemKeys.map((key, i) => [`key:${JSON.stringify(key)}`, i]));
      const states = positions ? new Map([...old?.states ?? []].flatMap(([, state]) => positions.has(state.key) ? [[positions.get(state.key), state]] : [])) : new Map([...old?.states ?? []].filter(([i]) => i < props.count));
      result.list = {
        count: props.count,
        previousCount: old?.count ?? props.count,
        anchor,
        anchorChanged: Boolean(old && old.count === props.count && anchor !== null && anchor !== old.anchor),
        anchorCache: old?.cache ?? null,
        states,
        cache: /* @__PURE__ */ new Map(),
        window: old?.window ? { ...old.window, start: Math.min(old.window.start, props.count), end: Math.min(old.window.end, props.count) } : null
      };
    } else if (!leaf) {
      const keys = /* @__PURE__ */ new Set();
      result.children = flat.map((child, position) => {
        const key = child?.[brand] && arena[child[brand][1]]?.[1]?.key;
        if (key !== void 0 && typeof key !== "string") throw Error("UI_KEY: expected string");
        if (key !== void 0 && keys.has(key)) throw Error("UI_KEY: duplicate sibling");
        if (key !== void 0) keys.add(key);
        const part = key === void 0 ? `position:${position}` : `key:${JSON.stringify(key)}`;
        return resolveNode(child, `${identity}/${tag}/${part}`, depth + 1, tag, typography, buttonAncestor || tag === "button" || tag === "checkbox");
      }).filter((child) => child !== null);
    }
    return result;
  }
  function copyTree(node) {
    let children = node.children;
    for (let i = 0; i < children.length; i++) {
      const child = copyTree(node.children[i]);
      if (child === node.children[i]) continue;
      if (children === node.children) children = children.slice();
      children[i] = child;
    }
    if (node.list) return {
      ...node,
      children,
      list: { ...node.list, states: new Map(node.list.states), cache: new Map(node.list.cache) }
    };
    return children === node.children ? node : { ...node, children };
  }
  function retainCells(cells) {
    for (const [key, cell] of cells) {
      candidate.locals.set(key, cell);
      for (const collector of localCollectors) collector.set(key, cell);
    }
  }
  function encodeTree(tree, windows) {
    const output = [];
    let count = 0, plotWork = 0, animationBytes = 0, itemKeyBytes = 0;
    const write = (node, parent, depth) => {
      if (depth > 128 || ++count > 4096) throw Error("UI_LIMIT: candidate too large");
      const { tag, props, identity } = node, encoded = [];
      for (const [name, value] of Object.entries(props.style ?? {})) {
        if (name === "animation") {
          if (value !== null) {
            const payload = JSON.stringify(value);
            animationBytes += utf8Bytes(payload, 32768);
            if (animationBytes > 262144) throw Error("UI_ANIMATION: page payload budget exceeded");
            encoded.push(fields.animation, payload);
          }
        } else if (name === "height" && value === "auto") encoded.push(82, 1);
        else if (has2(dimensionPercentFields, name) && typeof value === "string") encoded.push(dimensionPercentFields[name], Number(value.slice(0, -1)));
        else if (name === "boxShadow") encoded.push(fields.boxShadow, value === "none" ? value : JSON.stringify(value));
        else if (name === "backgroundGradient") encoded.push(fields.backgroundGradient, value === "none" ? value : JSON.stringify(value));
        else if (name === "offset") encoded.push(field.insetLeft, value.x, field.insetTop, value.y);
        else encoded.push(fields[name], value);
      }
      if (node.focusedStyle) {
        const focused = [];
        for (const [name, value] of Object.entries(node.focusedStyle)) focused.push(fields[name], ["boxShadow", "backgroundGradient"].includes(name) && value !== "none" ? JSON.stringify(value) : value);
        encoded.push(98, JSON.stringify(focused));
      }
      if (props.enabled !== void 0) encoded.push(buttonProps.enabled, +props.enabled);
      if (node.callback) {
        candidate.callbacks.set(node.callback[0], { callback: node.callback[1], text: false });
        encoded.push(buttonProps.onClick, node.callback[0]);
      }
      if ((tag === "button" || tag === "checkbox") && props.accessibilityLabel !== void 0) encoded.push(buttonProps.accessibilityLabel, props.accessibilityLabel);
      if (tag === "vector") encoded.push(vectorField, JSON.stringify({ viewBox: props.viewBox ?? [0, 0, 24, 24], paths: props.paths, ...props.fit === void 0 ? {} : { fit: props.fit } }));
      if (tag === "grid") encoded.push(gridProps.columns, props.columns);
      if (tag === "plot") {
        const count2 = props.values.length;
        const points = (count2 - 1) * (props.smooth ? 8 : 1) + 1;
        plotWork += props.kind === "pie" ? 256 + count2 : props.kind === "line" ? points * (props.area ? 2 : 1) + 2 * count2 + (props.area ? 2 : 0) : 8 * count2;
        if (plotWork > 16384) throw Error("UI_LIMIT: plot geometry budget exceeded");
        encoded.push(plotProps.values, JSON.stringify(props.values), plotProps.kind, props.kind, plotProps.colors, JSON.stringify(props.colors), plotProps.accessibilityLabel, props.accessibilityLabel);
        for (const name of ["smooth", "area"]) if (props[name] !== void 0) encoded.push(plotProps[name], +props[name]);
        for (const name of ["domainMin", "domainMax", "markerEmphasisScale"]) if (props[name] !== void 0) encoded.push(plotProps[name], props[name]);
        if (props.pieLayout !== void 0) encoded.push(plotProps.pieLayout, JSON.stringify(props.pieLayout));
        if (props.selectedIndex !== void 0) encoded.push(plotProps.selectedIndex, props.selectedIndex);
        if (node.plotSelect) {
          candidate.callbacks.set(node.plotSelect[0], { callback: node.plotSelect[1], number: true });
          encoded.push(plotProps.onSelect, node.plotSelect[0]);
        }
      }
      if (tag === "text" && props.lines !== void 0) encoded.push(textProps.lines, props.lines);
      if (tag === "slider") {
        for (const name of ["value", "min", "max", "step", "accessibilityLabel"]) encoded.push(sliderProps[name], props[name]);
        if (node.numberChange) {
          candidate.callbacks.set(node.numberChange[0], { callback: node.numberChange[1], number: true });
          encoded.push(sliderProps.onChange, node.numberChange[0]);
        }
      }
      if (tag === "radio" && props.accessibilityLabel !== void 0) encoded.push(radioProps.accessibilityLabel, props.accessibilityLabel);
      if (tag === "checkbox" || tag === "switch" || tag === "radio-option") {
        encoded.push(checkProps.value, +(props.value ?? false));
        if (node.change) {
          candidate.callbacks.set(node.change[0], { callback: node.change[1], text: false });
          encoded.push(checkProps.onChange, node.change[0]);
        }
      }
      for (const [field2, handle, callback] of node.reaches ?? []) {
        candidate.callbacks.set(handle, { callback, text: false, isolated: field2 === imageProps.onLoad || field2 === imageProps.onError });
        encoded.push(field2, handle);
      }
      if (tag === "input" || tag === "textarea" || tag === "search") {
        if (props.accessibilityLabel !== void 0) {
          if (typeof props.accessibilityLabel !== "string" || !props.accessibilityLabel.trim() || utf8Bytes(props.accessibilityLabel, 1025) > 1024 || props.accessibilityLabel.includes("\0")) throw Error("UI_INPUT: invalid accessibility label");
          encoded.push(buttonProps.accessibilityLabel, props.accessibilityLabel);
        }
        encoded.push(inputProps.value, props.value ?? "");
        for (const name of ["placeholder", "secure", "keyboard", "autoFocus", "submitOnEnter"]) if (props[name] !== void 0) encoded.push(inputProps[name], typeof props[name] === "boolean" ? +props[name] : props[name]);
        for (const [field2, handle, callback] of node.inputs) {
          candidate.callbacks.set(handle, { callback, text: true });
          encoded.push(field2, handle);
        }
      }
      if (tag === "image") {
        encoded.push(imageProps.src, props.src);
        if (props.fit !== void 0) encoded.push(imageProps.fit, props.fit);
        if (props.alt !== void 0) encoded.push(imageProps.alt, props.alt);
        if (props.retryKey !== void 0) encoded.push(imageProps.retryKey, props.retryKey);
      }
      if (node.rowIndex !== void 0) encoded.push(listFields.rowIndex, node.rowIndex);
      let window, seededAnchor = false;
      if (node.list) {
        window = windows[identity] ?? node.list.window ?? { start: 0, end: Math.min(1, props.count), leading: 0, trailing: Math.max(0, props.count - 1) * 64 };
        if (!Number.isSafeInteger(window.start) || !Number.isSafeInteger(window.end) || window.start < 0 || window.end < window.start || window.end > Math.max(props.count, node.list.previousCount) || ![window.leading, window.trailing].every((v) => Number.isFinite(v) && v >= 0 && v <= 16384e5)) throw Error("UI_LIST: invalid window");
        if (window.end > props.count) window = { start: 0, end: Math.min(1, props.count), leading: 0, trailing: Math.max(0, props.count - 1) * 64 };
        const target = props.anchor === "end" ? props.count - 1 : Math.min(props.anchor?.index ?? 0, props.count - 1);
        if (node.list.anchorChanged && props.count > 0 && (target < window.start || target >= window.end)) {
          node.list.cache = new Map([...node.list.anchorCache].filter(([i, row]) => props.itemKeys === void 0 || row.props.key === `key:${JSON.stringify(props.itemKeys[i])}`));
          seededAnchor = true;
        }
        node.list.anchorChanged = false;
        node.list.anchorCache = null;
        node.list.previousCount = props.count;
        node.list.window = { ...window };
        encoded.push(
          listFields.listId,
          identity,
          listFields.listCount,
          props.count,
          listFields.leading,
          window.leading,
          listFields.trailing,
          window.trailing
        );
        if (props.itemKeys !== void 0) {
          const payload = JSON.stringify(props.itemKeys);
          itemKeyBytes += utf8Bytes(payload, 262144);
          if (itemKeyBytes > 262144) throw Error("UI_LIST: itemKeys page budget exceeded");
          encoded.push(listFields.itemKeys, payload);
        }
        if (props.anchor !== void 0) encoded.push(listFields.anchor, JSON.stringify(props.anchor));
        candidate.lists.set(identity, node.list);
      }
      const index = output.length;
      output.push(tags[tag], parent, props.key ?? "", props.testId ?? "", node.text, encoded.length / 2, ...encoded);
      if (!node.list) {
        for (const child of node.children) write(child, index, depth + 1);
        return;
      }
      const visibleKeys = /* @__PURE__ */ new Set();
      for (const i of node.list.cache.keys()) if (i < window.start || i >= window.end) node.list.cache.delete(i);
      for (let i = window.start; i < window.end; i++) {
        const cells = /* @__PURE__ */ new Map();
        localCollectors.push(cells);
        try {
          let row = node.list.cache.get(i), key = node.list.states.get(i)?.key;
          if (!row) {
            const suppliedKey = props.itemKeys?.[i];
            const previous = scope;
            scope = suppliedKey === void 0 ? `${identity}/row:${i}` : `${identity}/row/key:${JSON.stringify(suppliedKey)}`;
            try {
              const value = props.row(i);
              const declared = value?.[brand] && arena[value[brand][1]]?.[1]?.key;
              if (declared !== void 0 && typeof declared !== "string") throw Error("UI_KEY: expected string");
              if (suppliedKey === void 0 && declared !== void 0 && cells.size > 0)
                throw Error("UI_LIST: keyed row factory hooks require itemKeys; alternatively move hooks into a keyed component");
              if (suppliedKey !== void 0 && declared !== void 0 && suppliedKey !== declared) throw Error("UI_LIST: row key does not match itemKeys");
              const rowKey = suppliedKey ?? declared;
              key = rowKey === void 0 ? `index:${i}` : `key:${JSON.stringify(rowKey)}`;
              scope = `${identity}/row/${key}`;
              const style = {};
              if (props.height) {
                const height = props.height(i);
                if (!Number.isFinite(height) || height < 0 || height > 16384) throw Error("UI_LIST: invalid row height");
                style.height = height;
              }
              row = {
                tag: "column",
                props: { key, style },
                identity: scope,
                rowIndex: i,
                text: "",
                children: [resolveNode(value, scope, depth + 2, null, node.typography)].filter((child) => child !== null)
              };
            } finally {
              scope = previous;
            }
          } else {
            row = copyTree(row);
            retainCells(node.list.states.get(i).locals);
          }
          if (visibleKeys.has(key)) throw Error("UI_KEY: duplicate list row");
          visibleKeys.add(key);
          write(row, index, depth + 1);
          node.list.cache.set(i, row);
          node.list.states.set(i, { key, locals: cells });
        } finally {
          localCollectors.pop();
        }
      }
      for (const state of node.list.states.values()) retainCells(state.locals);
      if (seededAnchor) node.list.cache.clear();
    };
    write(tree, -1, 0);
    return output;
  }
  function runCandidate(view, lookup, build) {
    if (rendering) throw Error("UI_RENDER: recursive render");
    arena = [];
    epoch++;
    rendering = true;
    currentView = view;
    lookupLocals = lookup;
    candidate = { locals: /* @__PURE__ */ new Map(), declarations: /* @__PURE__ */ new Set(), callbacks: /* @__PURE__ */ new Map(), lists: /* @__PURE__ */ new Map(), resources: /* @__PURE__ */ new Map(), resourceNames: /* @__PURE__ */ new Set(), view };
    try {
      scope = "root";
      return build();
    } catch (error) {
      discard();
      throw error;
    } finally {
      arena = [];
      scope = null;
      currentView = null;
      lookupLocals = null;
      localCollectors = [];
      rendering = false;
    }
  }
  function prepare(render, view, windows = {}) {
    return runCandidate(view, null, () => {
      candidate.tree = resolveNode(render(view), "root");
      if (!candidate.tree) throw Error("UI_ROOT: page must return one layout node");
      candidate.baseLocals = new Map(candidate.locals);
      return encodeTree(candidate.tree, windows);
    });
  }
  function prepareWindows(windows) {
    const base = candidate ?? committed;
    if (!base) throw Error("UI_LIST: no retained description");
    return runCandidate(base.view, base.locals, () => {
      candidate.resources = new Map(base.resources);
      candidate.tree = copyTree(base.tree);
      candidate.baseLocals = new Map(base.baseLocals);
      retainCells(candidate.baseLocals);
      return encodeTree(candidate.tree, windows);
    });
  }
  function commit() {
    if (!candidate) throw Error("UI_COMMIT: no candidate");
    const previous = committed;
    for (const cell of candidate.locals.values()) if (updates.has(cell)) cell.value = updates.get(cell);
    locals = candidate.locals;
    callbacks = candidate.callbacks;
    committed = candidate;
    candidate = null;
    updates.clear();
    const retained = new Set(locals.values()), next = /* @__PURE__ */ new Map(), definitions = /* @__PURE__ */ new Map();
    for (const [cell, resource] of ownedResources) if (!retained.has(cell)) resource.dispose();
    for (const cell of retained) {
      const entry = committed.resources.get(cell) ?? previous?.resources.get(cell);
      if (!entry) continue;
      definitions.set(cell, entry);
      next.set(cell, entry.resource);
      entry.resource.commit(entry.definition);
    }
    committed.resources = definitions;
    ownedResources = next;
  }
  function discard() {
    candidate = null;
    updates.clear();
  }
  function reset() {
    if (rendering) throw Error("UI_RENDER: reset during render");
    discard();
    committed = null;
    locals.clear();
    callbacks.clear();
    for (const resource of ownedResources.values()) resource.dispose();
    ownedResources.clear();
    cancelActions();
    actionErrors = [];
  }
  function dispatch(handle, value) {
    if (rendering) throw Error("UI_RENDER: dispatch during render");
    const entry = callbacks.get(handle);
    if (!entry) return false;
    if (entry.number ? typeof value !== "number" || !Number.isFinite(value) || Math.abs(value) > 1e9 : entry.text ? !validInputText(value) : value !== void 0) return false;
    const previous = entry.isolated ? new Map(updates) : void 0;
    try {
      const epoch2 = actionEpoch;
      const result = entry.text || entry.number ? entry.callback(value) : entry.callback();
      if (result && typeof result.then === "function") Promise.resolve(result).catch((error) => {
        if (epoch2 === actionEpoch && actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
      });
      return true;
    } catch (error) {
      updates.clear();
      if (!entry.isolated) throw error;
      for (const [cell, value2] of previous) updates.set(cell, value2);
      if (actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
      return true;
    }
  }
  function describe(render, view) {
    const output = prepare(render, view);
    commit();
    return output;
  }
  function validateVector(props) {
    if (props.fit !== void 0 && !["contain", "fill"].includes(props.fit)) throw Error("UI_VECTOR: invalid fit");
    const box = props.viewBox ?? [0, 0, 24, 24];
    if (!Array.isArray(box) || box.length !== 4 || box.some((v) => typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > 16384) || box[2] < 1 / 64 || box[3] < 1 / 64) throw Error("UI_VECTOR: invalid viewBox");
    if (!Array.isArray(props.paths) || !props.paths.length || props.paths.length > 32) throw Error("UI_VECTOR: invalid paths");
    for (const path of props.paths) {
      if (!path || typeof path !== "object" || Array.isArray(path) || Object.keys(path).some((k) => !["d", "fill", "stroke", "strokeWidth", "strokeDasharray", "vectorEffect"].includes(k)) || typeof path.d !== "string" || !path.d.length || path.d.length > 16384) throw Error("UI_VECTOR: invalid path");
      for (const key of ["fill", "stroke"]) if (path[key] !== void 0 && (typeof path[key] !== "string" || !["none", "currentColor"].includes(path[key]) && !/^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(path[key]))) throw Error("UI_VECTOR: invalid color");
      if (path.vectorEffect !== void 0 && !["none", "non-scaling-stroke"].includes(path.vectorEffect)) throw Error("UI_VECTOR: invalid vector effect");
      if (path.strokeDasharray !== void 0 && (!Array.isArray(path.strokeDasharray) || path.strokeDasharray.length > 16 || path.strokeDasharray.some((v) => typeof v !== "number" || !Number.isFinite(v) || v < 1 / 64 || v > 1024))) throw Error("UI_VECTOR: invalid dash pattern");
      if (path.strokeWidth !== void 0 && (typeof path.strokeWidth !== "number" || !Number.isFinite(path.strokeWidth) || path.strokeWidth < 0 || path.strokeWidth > 1024)) throw Error("UI_VECTOR: invalid stroke width");
    }
    if (JSON.stringify({ viewBox: box, paths: props.paths, ...props.fit === void 0 ? {} : { fit: props.fit } }).length > 32768) throw Error("UI_VECTOR: source too large");
  }

  // src/host.js
  function publishHost(render, catalog) {
    Object.assign(globalThis, {
      __lingxiaRender: render,
      __lingxiaDescribe: describe,
      __lingxiaPrepare: prepare,
      __lingxiaPrepareWindows: prepareWindows,
      __lingxiaCommit: commit,
      __lingxiaDiscard: discard,
      __lingxiaReset: reset,
      __lingxiaDispatch: dispatch,
      __lingxiaCreateView: createView,
      __lingxiaConfigureActions: configureActions,
      __lingxiaTakeActionRequests: takeActionRequests,
      __lingxiaReceiveActionResponse: receiveActionResponse,
      __lingxiaTakeActionErrors: takeActionErrors,
      __lingxiaHasLocalUpdates: hasLocalUpdates,
      __lingxiaRequestLeave: requestLeave
    });
    if (catalog !== void 0) globalThis.__lingxiaActionCatalog = catalog;
  }

  // <stdin>
  Object.assign(globalThis, internal_exports, { __lingxiaPublish: publishHost });
})();
