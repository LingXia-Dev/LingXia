// Generated from contracts/view/schema.json; do not edit.
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
    SizeClassScope: () => SizeClassScope,
    childrenToArray: () => childrenToArray,
    cloneElement: () => cloneElement,
    commit: () => commit,
    configureActions: () => configureActions,
    configureClasses: () => configureClasses,
    configureFonts: () => configureFonts,
    configureSourceMap: () => configureSourceMap,
    createView: () => createView,
    describe: () => describe,
    discard: () => discard,
    dispatch: () => dispatch,
    elementType: () => elementType,
    enableHotSwap: () => enableHotSwap,
    h: () => h,
    hasLocalUpdates: () => hasLocalUpdates,
    listTag: () => listTag,
    measureFrames: () => measureFrames,
    prepare: () => prepare,
    prepareWindows: () => prepareWindows,
    readsHostContext: () => readsHostContext,
    receiveActionResponse: () => receiveActionResponse,
    registerCatalogues: () => registerCatalogues,
    registerLanguageCatalogue: () => registerLanguageCatalogue,
    requestLeave: () => requestLeave,
    reset: () => reset,
    reveal: () => reveal,
    seedLowered: () => seedLowered,
    setLoudFailures: () => setLoudFailures,
    takeActionErrors: () => takeActionErrors,
    takeActionRequests: () => takeActionRequests,
    takeHandlerFailure: () => takeHandlerFailure,
    takeLowered: () => takeLowered,
    takeMovedAnchors: () => takeMovedAnchors,
    takeThrown: () => takeThrown,
    useAction: () => useAction,
    useBackHandler: () => useBackHandler,
    useBox: () => useBox,
    useChannel: () => useChannel,
    useEnv: () => useEnv,
    useId: () => useId,
    useLeaveGuard: () => useLeaveGuard,
    useLocal: () => useLocal,
    useNow: () => useNow,
    useRequest: () => useRequest,
    useStream: () => useStream,
    useVideo: () => useVideo,
    useView: () => useView,
    useWebView: () => useWebView
  });

  // src/runtime/state.js
  var render = {
    active: false,
    candidate: null,
    scope: null,
    view: null,
    lookupLocals: null,
    collectors: [],
    site: null,
    thrown: "",
    // Memo: committed subtrees to recall, open component frames, and counters subtrees compare.
    recall: null,
    frames: [],
    lists: 0,
    cellReads: 0,
    contextReads: 0,
    // The providers visible to the component or row factory now running.
    contexts: void 0
  };
  var page = { committed: null, locals: /* @__PURE__ */ new Map(), callbacks: /* @__PURE__ */ new Map(), updates: /* @__PURE__ */ new Map(), resources: /* @__PURE__ */ new Map() };
  function outsideRender(what) {
    if (render.active) throw Error(`UI_RENDER: ${what} during render`);
  }

  // src/runtime/profile.js
  var spans = null;
  function clock() {
    const read = globalThis.__lingxiaClock;
    return read && read.phases === true ? read : null;
  }
  function now() {
    const read = clock();
    if (!read) return 0;
    spans ?? (spans = {});
    return read();
  }
  function spent(name, since) {
    if (spans && since) spans[name] = (spans[name] ?? 0) + globalThis.__lingxiaClock() - since;
  }
  function counted(name) {
    if (spans) spans[name] = (spans[name] ?? 0) + 1;
  }
  function takeSpans() {
    if (!spans) return "";
    for (const name in spans) spans[name] = Math.round(spans[name]);
    const out = JSON.stringify(spans);
    spans = {};
    return out;
  }

  // src/runtime/cx.js
  function cx(...values) {
    const out = [];
    const add = (value) => {
      if (!value) return;
      if (typeof value === "string" || typeof value === "number") out.push(String(value));
      else if (Array.isArray(value)) value.forEach(add);
      else if (typeof value === "object") {
        for (const key of Object.keys(value)) if (value[key]) out.push(key);
      }
    };
    values.forEach(add);
    return out.join(" ");
  }

  // src/runtime/nodes.js
  var arena = [];
  var epoch = 0;
  var brand = Symbol("ui candidate");
  function resetArena() {
    arena = [];
    epoch++;
  }
  function clearArena() {
    arena = [];
  }
  var currentArena = () => arena;
  function readNode(node) {
    const ref = node[brand], offset = ref[1];
    if (ref[0] !== epoch || !arena[offset]) throw Error("UI_STALE: candidate from another render");
    return arena[offset];
  }
  var isNode = (value) => Boolean(value && typeof value === "object" && brand in value);
  var nodeKey = (value) => isNode(value) ? readNode(value)[1].key : void 0;
  function push(record) {
    if (!render.active) throw Error("UI_SCOPE: JSX outside render");
    if (arena.length >= 16384) throw Error("UI_LIMIT: candidate arena too large");
    counted("h");
    const handle2 = { [brand]: [epoch, arena.length] };
    arena.push(record);
    return handle2;
  }
  function h(tag, props, ...children) {
    const { children: supplied, __loc: location, ...attributes } = props ?? {};
    return push([tag, attributes, children.length ? children : supplied === void 0 ? [] : [supplied], location]);
  }
  var elementType = (child) => isNode(child) ? readNode(child)[0] : void 0;
  var tagName = (tag) => typeof tag === "string" ? `<${tag}>` : `<${tag?.name || "component"}>`;
  function cloneElement(element, props, options2) {
    if (!isNode(element)) throw Error("UI_PROP: cloneElement takes one element; use childrenToArray first");
    const [tag, own2, children, location] = readNode(element);
    if (props === null || typeof props !== "object" || Array.isArray(props)) throw Error("UI_PROP: cloneElement takes a props object");
    if ("key" in props || "children" in props) throw Error("UI_PROP: cloneElement does not change key or children");
    const override = options2?.override ?? [];
    if (!Array.isArray(override)) throw Error("UI_PROP: cloneElement override takes an array of handler names");
    const merged = { ...own2 };
    for (const name of Object.keys(props)) {
      if (/^on[A-Z]/.test(name) && own2[name] !== void 0 && !override.includes(name)) {
        throw Error(`UI_PROP: cloneElement would replace ${name} on ${tagName(tag)}; pass { override: ["${name}"] } to replace it`);
      }
      merged[name] = name === "className" ? cx(own2.className, props.className) : props[name];
    }
    return push([tag, merged, children, location]);
  }
  function Fragment() {
    throw Error("UI_FRAGMENT: a fragment is not a component");
  }
  var isFragment = (value) => isNode(value) && readNode(value)[0] === Fragment;
  var kept = (child) => child !== null && child !== void 0 && typeof child !== "boolean" && child !== "";
  function entries(children, prefix = "", scope, depth = 0, result = []) {
    if (depth > 128) throw Error("UI_LIMIT: children too deep");
    for (let index = 0; index < children.length; index++) {
      if (!(index in children)) continue;
      const child = children[index];
      if (result.length >= 16384) throw Error("UI_LIMIT: too many children");
      const slot = prefix ? `${prefix}.${index}` : String(index);
      if (Array.isArray(child)) {
        entries(child, slot, scope, depth + 1, result);
        continue;
      }
      if (isFragment(child)) {
        const record = readNode(child), props = record[1];
        if (props.key !== void 0 && typeof props.key !== "string") throw Error("UI_KEY: expected string");
        const named = props.key === void 0 ? scope : grouped(scope, slot, props.key);
        entries(record[2], props.key === void 0 ? slot : "", named, depth + 1, result);
        continue;
      }
      if (!kept(child)) continue;
      const key = nodeKey(child);
      if (!scope) result.push({ child, slot, key });
      else result.push({ child, key: JSON.stringify([scope, key ?? `#${slot}`]) });
    }
    return result;
  }
  var grouped = (scope, slot, key) => scope === void 0 ? key : JSON.stringify([scope, key ?? `#${slot}`]);
  function flatten(children) {
    return entries(children).map((entry) => entry.child);
  }
  function childrenToArray(children) {
    return flatten([children]);
  }

  // src/runtime/press.js
  var held = null;
  function pressChange(identity2, callback) {
    if (held?.identity === identity2) return;
    held = { identity: identity2, callback };
    return callback(true);
  }
  function releasePress() {
    if (!held) return false;
    const { callback } = held;
    held = null;
    const result = callback(false);
    if (result && typeof result.then === "function") Promise.resolve(result).catch(() => {
    });
    return true;
  }
  function refreshPresses(live) {
    if (held && live.has(held.identity)) held.callback = live.get(held.identity);
  }
  var waiting = /* @__PURE__ */ new Map();
  function changeEnd(identity2, value, end) {
    if (waiting.has(identity2)) waiting.set(identity2, { value });
    else end(value);
  }
  function pressEnd(identity2, pressed, end) {
    if (pressed) {
      waiting.set(identity2, null);
      return;
    }
    const last = waiting.get(identity2);
    waiting.delete(identity2);
    if (last) end(last.value);
  }
  var resetPresses = () => {
    held = null;
    waiting = /* @__PURE__ */ new Map();
  };

  // src/generated/contract.js
  var elements = { "view": { "wire": "column", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "headingLevel": false, "aria-valuenow": false, "aria-valuemin": false, "aria-valuemax": false, "aria-valuetext": false, "aria-modal": false, "bleed": false, "onPage": false, "onContextMenu": false, "anchor": false, "onPointer": false, "pointerAxis": false, "onLayout": false, "box": false, "aria-label": false, "popover": false, "open": false, "onToggle": false, "drag": false, "dragSnapPoints": false, "bind": false, "snapIndex": false, "onSnapChange": false, "onDragStart": false, "onDragEnd": false, "aria-actions": false, "onPointerEnter": false, "onPointerLeave": false, "closeAfter": false, "onCloseRequest": false } }, "page": { "wire": "page", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "title": false, "aria-label": false } }, "text": { "wire": "text", "children": "runs", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "backgroundClip", "lineClamp", "textOverflow"], "props": { "headingLevel": false, "onContextMenu": false, "onLayout": false, "box": false, "aria-label": false } }, "button": { "wire": "button", "children": "content", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "aria-level": false, "aria-controls": false, "aria-valuetext": false, "aria-checked": false, "onContextMenu": false, "onLayout": false, "box": false, "onPointer": false, "pointerAxis": false, "onClick": false, "onFocus": false, "onBlur": false, "disabled": false, "aria-label": false, "popoverTarget": false, "interestFor": false, "aria-current": false, "keepOpen": false, "keepsFocus": false, "drag": false, "dragSnapPoints": false, "bind": false, "snapIndex": false, "onSnapChange": false, "onDragStart": false, "onDragEnd": false, "aria-actions": false, "onPointerEnter": false, "onPointerLeave": false } }, "scroll": { "wire": "scroll", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "overflowX", "overflowY", "overscrollBehavior", "scrollSnapStop", "scrollSnapType"], "props": { "bleed": false, "onLayout": false, "box": false, "onReachStart": false, "onReachEnd": false, "onScrollEdge": false, "onVisibleChange": false, "bind": false, "snapIndex": false, "onSnapChange": false } }, "list": { "wire": "list", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "overscrollBehavior", "scrollSnapStop", "scrollSnapType"], "props": { "bleed": false, "count": true, "row": true, "height": false, "anchor": false, "itemKeys": false, "onReachStart": false, "onReachEnd": false, "onScrollEdge": false, "onVisibleChange": false, "aria-label": false }, "component": "List" }, "image": { "wire": "image", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "objectFit", "objectPosition"], "props": { "bleed": false, "src": true, "placeholder": false, "alt": false, "retryKey": false, "onLoad": false, "onError": false } }, "input": { "wire": "input", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "placeholderColor", "textOverflow"], "props": { "aria-valuenow": false, "aria-valuemin": false, "aria-valuemax": false, "aria-valuetext": false, "aria-activedescendant": false, "aria-controls": false, "aria-autocomplete": false, "onKeyDown": false, "readOnly": false, "onFocus": false, "onBlur": false, "onSelectionChange": false, "onPaste": false, "aria-label": false, "placeholder": false, "type": false, "inputMode": false, "autoFocus": false, "disabled": false, "onSubmit": false, "maxLength": false, "autoComplete": false, "enterKeyHint": false, "autoCapitalize": false, "spellCheck": false, "min": false, "max": false, "step": false }, "binding": { "type": "string", "event": "onInput" } }, "textarea": { "wire": "textarea", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "placeholderColor"], "props": { "aria-activedescendant": false, "aria-controls": false, "onKeyDown": false, "readOnly": false, "onFocus": false, "onBlur": false, "onSelectionChange": false, "onPaste": false, "rows": false, "maxRows": false, "aria-label": false, "placeholder": false, "inputMode": false, "autoFocus": false, "disabled": false, "onSubmit": false, "maxLength": false, "autoComplete": false, "enterKeyHint": false, "autoCapitalize": false, "spellCheck": false }, "binding": { "type": "string", "event": "onInput" } }, "checkbox": { "wire": "checkbox", "children": "content", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "indeterminate": false, "defaultChecked": false, "aria-label": false, "disabled": false }, "binding": { "type": "boolean", "event": "onChange" } }, "switch": { "wire": "switch", "children": "content", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "trackPart", "rangePart", "secondaryPart", "thumbPart"], "props": { "defaultChecked": false, "aria-label": false, "disabled": false }, "binding": { "type": "boolean", "event": "onChange" } }, "radio": { "wire": "radio", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "options": true, "aria-label": false, "disabled": false }, "binding": { "type": "string", "event": "onChange" } }, "slider": { "wire": "slider", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "trackPart", "rangePart", "secondaryPart", "thumbPart"], "props": { "min": false, "max": false, "step": false, "defaultValue": false, "secondaryValue": false, "aria-label": true, "aria-valuetext": false, "disabled": false, "onPressChange": false }, "binding": { "type": "number", "event": "onChange", "end": "onChangeEnd", "pair": { "values": "values", "labels": "thumbLabels" } } }, "dialog": { "wire": "column", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "aria-modal": false, "open": false, "onClose": false, "closedby": false, "aria-label": true, "drag": false, "dragSnapPoints": false, "bind": false, "snapIndex": false, "onSnapChange": false, "onDragStart": false, "onDragEnd": false } }, "plot": { "wire": "plot", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign"], "props": { "onLayout": false, "box": false, "kind": true, "values": true, "colors": true, "aria-label": true, "smooth": false, "area": false, "selectedIndex": false, "domainMin": false, "domainMax": false, "onSelect": false, "pieLayout": false, "markerEmphasisScale": false } }, "vector": { "wire": "vector", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "objectFit"], "props": { "bleed": false, "onPointer": false, "pointerAxis": false, "onLayout": false, "box": false, "paths": true, "gradients": false, "viewBox": false } }, "span": { "wire": "span", "children": "runs", "style": ["color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "letterSpacing", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "fontVariantNumeric", "backgroundColor"], "props": {} }, "link": { "wire": "link", "children": "runs", "style": ["color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "letterSpacing", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "fontVariantNumeric", "backgroundColor"], "props": { "onContextMenu": false, "onClick": false, "onFocus": false, "onBlur": false, "to": false, "aria-label": false, "disabled": false, "popoverTarget": false, "interestFor": false, "aria-current": false } }, "select": { "wire": "select", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "backgroundClip", "lineClamp", "textOverflow", "placeholderColor"], "props": { "options": true, "defaultValue": false, "placeholder": false, "aria-label": false, "disabled": false }, "binding": { "type": "string", "event": "onChange" } }, "video": { "wire": "video", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "objectFit", "objectPosition"], "props": { "bleed": false, "src": false, "sources": false, "player": false, "poster": false, "autoPlay": false, "paused": false, "muted": false, "loop": false, "playbackRate": false, "volume": false, "aria-label": false, "onLoadedMetadata": false, "onPlay": false, "onPlaying": false, "onPause": false, "onWaiting": false, "onSeeked": false, "onTimeUpdate": false, "onEnded": false, "onError": false, "onRateChange": false, "onVolumeChange": false, "onQualityChange": false, "onFullscreenChange": false } }, "webview": { "wire": "webview", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign"], "props": { "bleed": false, "src": false, "web": false, "aria-label": true, "handlesDrags": false, "onFullscreenChange": false, "onLoad": false, "onMessage": false, "onBlocked": false, "onError": false } } };
  var blocks = { "link-box": { "wire": "link-box", "children": "content", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "positionArea", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "ring", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "maskImage", "filter", "display", "touchTarget", "scrollSnapAlign", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textDecorationStyle", "textDecorationColor", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "onContextMenu": false, "onClick": false, "onFocus": false, "onBlur": false, "to": false, "aria-label": false, "disabled": false, "popoverTarget": false, "interestFor": false, "aria-current": false }, "of": "link" } };
  var kindOf = (name) => Object.hasOwn(elements, name) ? elements[name] : blocks[name];
  var style = { "width": { "lower": "dimension", "wire": "width" }, "height": { "lower": "dimension", "wire": "height" }, "minWidth": { "lower": "dimension", "wire": "minWidth" }, "minHeight": { "lower": "dimension", "wire": "minHeight" }, "maxWidth": { "lower": "dimension", "wire": "maxWidth" }, "aspectRatio": { "lower": "aspect-ratio", "wire": "aspectRatio" }, "maxHeight": { "lower": "dimension", "wire": "maxHeight" }, "margin": { "lower": "margin", "wire": ["marginTop", "marginRight", "marginBottom", "marginLeft"] }, "marginTop": { "lower": "margin", "wire": "marginTop" }, "marginRight": { "lower": "margin", "wire": "marginRight" }, "marginBottom": { "lower": "margin", "wire": "marginBottom" }, "marginLeft": { "lower": "margin", "wire": "marginLeft" }, "marginInlineStart": { "lower": "margin", "wire": "marginInlineStart" }, "marginInlineEnd": { "lower": "margin", "wire": "marginInlineEnd" }, "position": { "lower": "option", "wire": "position" }, "inset": { "lower": "inset", "wire": ["insetTop", "insetRight", "insetBottom", "insetLeft"] }, "top": { "lower": "inset", "wire": "insetTop" }, "right": { "lower": "inset", "wire": "insetRight" }, "bottom": { "lower": "inset", "wire": "insetBottom" }, "left": { "lower": "inset", "wire": "insetLeft" }, "insetInlineStart": { "lower": "inset", "wire": "insetInlineStart" }, "insetInlineEnd": { "lower": "inset", "wire": "insetInlineEnd" }, "zIndex": { "lower": "z-index", "wire": "zIndex" }, "positionArea": { "lower": "option", "wire": "anchorArea" }, "flexGrow": { "lower": "number", "wire": "grow" }, "flexShrink": { "lower": "number", "wire": "shrink" }, "flexBasis": { "lower": "basis", "wire": "flexBasis" }, "alignSelf": { "lower": "alignment", "wire": "alignSelf" }, "gridColumn": { "lower": "grid-line", "wire": "gridColumn" }, "gridColumnStart": { "lower": "grid-line", "wire": "gridColumn" }, "gridColumnEnd": { "lower": "grid-line", "wire": "gridColumn" }, "gridRow": { "lower": "grid-line", "wire": "gridRow" }, "gridRowStart": { "lower": "grid-line", "wire": "gridRow" }, "gridRowEnd": { "lower": "grid-line", "wire": "gridRow" }, "justifySelf": { "lower": "alignment", "wire": "justifySelf" }, "order": { "lower": "order", "wire": "order" }, "opacity": { "lower": "opacity", "wire": "opacity", "paint": true }, "overflow": { "lower": "option", "wire": "overflow" }, "backgroundColor": { "lower": "color", "wire": "background", "paint": true }, "backgroundImage": { "lower": "gradient", "wire": "backgroundGradient", "paint": true }, "borderRadius": { "lower": "length", "wire": ["radius", "radiusTopLeft", "radiusTopRight", "radiusBottomRight", "radiusBottomLeft"] }, "borderTopLeftRadius": { "lower": "length", "wire": "radiusTopLeft" }, "borderTopRightRadius": { "lower": "length", "wire": "radiusTopRight" }, "borderBottomRightRadius": { "lower": "length", "wire": "radiusBottomRight" }, "borderBottomLeftRadius": { "lower": "length", "wire": "radiusBottomLeft" }, "borderStartStartRadius": { "lower": "length", "wire": "radiusStartStart" }, "borderStartEndRadius": { "lower": "length", "wire": "radiusStartEnd" }, "borderEndStartRadius": { "lower": "length", "wire": "radiusEndStart" }, "borderEndEndRadius": { "lower": "length", "wire": "radiusEndEnd" }, "borderWidth": { "lower": "length", "wire": ["borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth"] }, "borderTopWidth": { "lower": "length", "wire": "borderTopWidth" }, "borderRightWidth": { "lower": "length", "wire": "borderRightWidth" }, "borderBottomWidth": { "lower": "length", "wire": "borderBottomWidth" }, "borderLeftWidth": { "lower": "length", "wire": "borderLeftWidth" }, "borderInlineStartWidth": { "lower": "length", "wire": "borderInlineStartWidth" }, "borderInlineEndWidth": { "lower": "length", "wire": "borderInlineEndWidth" }, "borderStyle": { "lower": "option", "wire": "borderStyle" }, "borderColor": { "lower": "color", "wire": "borderColor", "paint": true }, "boxShadow": { "lower": "shadow", "wire": "boxShadow", "paint": true }, "ring": { "lower": "ring", "wire": "ring", "paint": true }, "backdropFilter": { "lower": "backdrop", "wire": ["backdropBlur", "backdropSaturation"] }, "translate": { "lower": "transform", "wire": "transform", "paint": true }, "rotate": { "lower": "transform", "wire": "transform", "paint": true }, "scale": { "lower": "transform", "wire": "transform", "paint": true }, "transformOrigin": { "lower": "transform", "wire": "transform", "paint": true }, "animation": { "lower": "animation", "wire": "animation", "internal": true }, "transition": { "lower": "transition", "wire": "transition", "internal": true }, "maskImage": { "lower": "mask", "wire": "maskImage", "internal": true }, "filter": { "lower": "filter", "wire": "filterBlur" }, "padding": { "lower": "length", "wire": ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"] }, "paddingTop": { "lower": "length", "wire": "paddingTop" }, "paddingRight": { "lower": "length", "wire": "paddingRight" }, "paddingBottom": { "lower": "length", "wire": "paddingBottom" }, "paddingLeft": { "lower": "length", "wire": "paddingLeft" }, "paddingInlineStart": { "lower": "length", "wire": "paddingInlineStart" }, "paddingInlineEnd": { "lower": "length", "wire": "paddingInlineEnd" }, "display": { "lower": "option", "wire": "display" }, "touchTarget": { "lower": "option", "wire": "touchTarget" }, "flexDirection": { "lower": "option", "wire": "flexDirection" }, "alignItems": { "lower": "alignment", "wire": "align" }, "justifyContent": { "lower": "justification", "wire": "justify" }, "alignContent": { "lower": "align-content", "wire": "alignContent" }, "flexWrap": { "lower": "option", "wire": "flexWrap" }, "gap": { "lower": "length", "wire": "gap" }, "rowGap": { "lower": "length", "wire": "rowGap" }, "columnGap": { "lower": "length", "wire": "columnGap" }, "gridTemplateColumns": { "lower": "grid-template", "wire": "gridTemplateColumns" }, "gridTemplateRows": { "lower": "grid-template", "wire": "gridTemplateRows" }, "gridAutoColumns": { "lower": "grid-auto", "wire": "gridAutoColumns" }, "gridAutoRows": { "lower": "grid-auto", "wire": "gridAutoRows" }, "gridAutoFlow": { "lower": "grid-flow", "wire": "gridAutoFlow" }, "justifyItems": { "lower": "alignment", "wire": "justifyItems" }, "color": { "lower": "color", "wire": "color", "paint": true }, "fontSize": { "lower": "font-size", "wire": "fontSize" }, "fontWeight": { "lower": "font-weight", "wire": "fontWeight" }, "fontFamily": { "lower": "family", "wire": "fontFamily" }, "fontStyle": { "lower": "option", "wire": "fontStyle" }, "lineHeight": { "lower": "line-height", "wire": "lineHeight" }, "letterSpacing": { "lower": "letter-spacing", "wire": "letterSpacing" }, "fontVariantNumeric": { "lower": "font-variant-numeric", "wire": "fontVariantNumeric" }, "userSelect": { "lower": "option", "wire": "userSelect" }, "textAlign": { "lower": "option", "wire": "textAlign" }, "textDecorationLine": { "lower": "option", "wire": "textDecorationLine" }, "textDecorationStyle": { "lower": "option", "wire": "textDecorationStyle" }, "textDecorationColor": { "lower": "color", "wire": "textDecorationColor" }, "textTransform": { "lower": "option", "wire": "textTransform" }, "backgroundClip": { "lower": "option", "wire": "backgroundClip" }, "whiteSpace": { "lower": "option", "wire": "whiteSpace" }, "wordBreak": { "lower": "option", "wire": "wordBreak" }, "overflowWrap": { "lower": "option", "wire": "overflowWrap" }, "lineClamp": { "lower": "line-clamp", "wire": "textLines" }, "textOverflow": { "lower": "option", "wire": "textOverflow" }, "overflowX": { "lower": "scroll-axis", "wire": "scrollAxis" }, "overflowY": { "lower": "scroll-axis", "wire": "scrollAxis" }, "overscrollBehavior": { "lower": "option", "wire": "overscroll" }, "scrollSnapStop": { "lower": "option", "wire": "snapStop" }, "scrollSnapType": { "lower": "snap-type", "wire": "scrollSnap" }, "scrollSnapAlign": { "lower": "option", "wire": "snapAlign" }, "placeholderColor": { "lower": "color", "wire": "placeholderColor", "paint": true }, "trackPart": { "lower": "part", "wire": "trackStyle", "paint": true, "internal": true }, "rangePart": { "lower": "part", "wire": "rangeStyle", "paint": true, "internal": true }, "secondaryPart": { "lower": "part", "wire": "secondaryStyle", "paint": true, "internal": true }, "thumbPart": { "lower": "part", "wire": "thumbStyle", "paint": true, "internal": true }, "objectFit": { "lower": "fit", "wire": "imageFit" }, "objectPosition": { "lower": "object-position", "wire": "imagePosition" } };
  var limits = { "nodes": 4096, "depth": 128, "animationBytes": 262144, "itemKeyBytes": 262144, "imageBytes": 8388608, "sourceBytes": 8192, "visibleImageBytes": 67108864, "decodedImageBytes": 100663296, "transitionMs": 1e4, "evalBudgetMs": 100, "evalBudgetDebugMultiplier": 8, "evalWarmupMultiplier": 2, "evalLimitMs": 3e3, "evalOverrunFrames": 3, "editorTextBytes": 65536, "inputRequestBytes": 524288, "pasteHtmlBytes": 2097152, "pasteRequestBytes": 6291456, "inputBatchEdits": 64, "clipboardItems": 128, "wireValues": 1e5, "textBytes": 65536, "identifierBytes": 256, "listRows": 1e5, "gridTracks": 64, "vectorSourceBytes": 32768, "vectorPathBytes": 16384, "vectorPathSegments": 1024, "vectorPathWork": 16384, "vectorWork": 32768, "plotWork": 16384, "shadowMaskBytes": 16777216, "compositeFrames": 6, "leaseFrames": 6, "compositeBytes": 268435456, "outlineGlyphs": 4096, "ghosts": 64, "layoutMotions": 256, "dataAttributeBytes": 65536, "blurLayers": 8, "topLayer": 16, "dragSnapPoints": 8, "webMessageBytes": 65536, "webMessageQueue": 64, "styleNumber": 16384, "lineHeightMultiplier": 4 };
  var admission = { "tags": { "column": { "children": "any", "content": true }, "reserved-1": { "children": "none" }, "text": { "children": "runs", "content": true }, "button": { "children": "content" }, "reserved-4": { "children": "none" }, "scroll": { "children": "any" }, "list": { "children": "rows", "requires": ["listId", "listCount", "listLeading", "listTrailing"] }, "image": { "children": "none", "content": true, "requires": ["imageSrc"] }, "input": { "children": "none" }, "textarea": { "children": "none" }, "checkbox": { "children": "content" }, "switch": { "children": "content" }, "radio": { "children": "options" }, "radio-option": { "children": "text", "parent": "radio", "keyed": true, "requires": ["checked"] }, "search": { "children": "none" }, "slider": { "children": "none" }, "link-box": { "children": "content" }, "reserved-17": { "children": "none" }, "plot": { "children": "none" }, "vector": { "children": "none", "content": true }, "page": { "children": "any", "root": true }, "span": { "children": "text", "content": true, "parent": "text" }, "link": { "children": "text", "parent": "text" }, "select": { "children": "text" }, "video": { "children": "none", "content": true }, "webview": { "children": "none", "content": true, "requires": ["webSrc", "webSession", "accessibilityLabel"] } }, "fields": { "onClick": ["button", "link", "link-box"], "enabled": ["button", "input", "textarea", "checkbox", "switch", "radio", "radio-option", "search", "slider", "select", "link", "link-box"], "listId": ["list"], "listCount": ["list"], "listLeading": ["list"], "listTrailing": ["list"], "listRowIndex": ["column"], "listAnchor": ["list"], "onReachStart": ["scroll", "list"], "onReachEnd": ["scroll", "list"], "imageSrc": ["image"], "imageFit": ["image", "video"], "inputValue": ["input", "textarea", "search", "select"], "placeholder": ["input", "textarea", "search", "select"], "secure": ["input", "textarea"], "keyboard": ["input", "textarea", "search"], "autoFocus": ["input", "textarea", "search"], "onInput": ["input", "textarea", "search"], "onSubmit": ["input", "textarea", "search"], "checked": ["checkbox", "switch", "radio-option"], "onCheckedChange": ["checkbox", "switch", "radio-option"], "accessibilityLabel": ["button", "input", "textarea", "checkbox", "switch", "radio", "search", "slider", "plot", "link", "column", "page", "text", "select", "video", "webview", "list", "link-box"], "sliderValue": ["slider"], "sliderMin": ["slider"], "sliderMax": ["slider"], "sliderStep": ["slider"], "onSliderChange": ["slider"], "textLines": ["text"], "plotValues": ["plot"], "plotKind": ["plot"], "plotColors": ["plot"], "plotSmooth": ["plot"], "plotArea": ["plot"], "onPlotSelect": ["plot"], "plotSelectedIndex": ["plot"], "vectorSource": ["vector"], "submitOnEnter": ["textarea"], "plotDomainMin": ["plot"], "plotDomainMax": ["plot"], "plotPieLayout": ["plot"], "plotMarkerEmphasisScale": ["plot"], "focusStyle": ["input", "textarea", "search", "select"], "scrollAxis": ["scroll"], "listItemKeys": ["list"], "flexDirection": ["column", "page", "button", "link-box"], "imageAlt": ["image"], "imageRetryKey": ["image"], "onImageLoad": ["image"], "onImageError": ["image"], "pressedStyle": ["button", "checkbox", "switch", "radio", "link", "slider", "link-box"], "overscroll": ["scroll", "list"], "maxLength": ["input", "textarea", "search"], "autoComplete": ["input", "textarea", "search"], "enterKeyHint": ["input", "textarea", "search"], "autoCapitalize": ["input", "textarea", "search"], "spellCheck": ["input", "textarea", "search"], "inputMin": ["input"], "inputMax": ["input"], "inputStep": ["input"], "focusRequest": ["input", "textarea", "search"], "selectOptions": ["select"], "onSelect": ["select"], "rows": ["textarea"], "maxRows": ["textarea"], "imagePlaceholder": ["image"], "imagePosition": ["image", "video"], "anchorTo": ["column"], "scrollSnap": ["scroll", "list"], "topLayer": ["column"], "anchorArea": ["column"], "drag": ["column", "button"], "onDragEvent": ["column", "button"], "dragScrim": ["button"], "elementId": ["column", "text", "button", "scroll", "list", "image", "input", "textarea", "checkbox", "switch", "radio", "search", "slider", "plot", "vector", "page", "select", "video", "webview", "link-box"], "ariaActions": ["column", "button"], "snapStop": ["scroll", "list"], "snapIndex": ["scroll"], "onSnapChange": ["scroll"], "videoSrc": ["video"], "videoPlayer": ["video"], "videoPoster": ["video"], "videoPaused": ["video"], "videoMuted": ["video"], "videoLoop": ["video"], "videoRate": ["video"], "videoVolume": ["video"], "onVideoEvent": ["video"], "videoSources": ["video"], "ariaDescribedBy": ["column", "text", "button", "link", "scroll", "list", "image", "input", "textarea", "checkbox", "switch", "radio", "search", "slider", "plot", "vector", "page", "select", "video", "webview", "link-box"], "focusVisibleStyle": ["button", "checkbox", "switch", "radio", "slider", "link", "select", "input", "textarea", "search", "link-box"], "sliderSecondary": ["slider"], "interestFor": ["button", "link", "link-box"], "onPointerEnter": ["column", "button", "link-box"], "onPointerLeave": ["column", "button", "link-box"], "hint": ["column"], "onSliderPress": ["slider"], "closeAfter": ["column"], "displayValue": ["input"], "sizeText": ["input"], "onLayout": ["column", "text", "button", "vector", "plot", "scroll"], "onPointer": ["column", "button", "vector"], "pointerAxis": ["column", "button", "vector"], "onFocus": ["button", "input", "textarea", "search", "link", "link-box"], "onBlur": ["button", "input", "textarea", "search", "link", "link-box"], "readOnly": ["input", "textarea", "search"], "keyMap": ["input", "textarea", "search"], "onKeyDown": ["input", "textarea", "search"], "ariaActiveDescendant": ["input", "textarea", "search"], "mixed": ["checkbox", "button", "column"], "onContextMenu": ["column", "button", "link", "text", "link-box"], "onPage": ["column"], "sliderOther": ["slider"], "sliderThumb": ["slider"], "webSrc": ["webview"], "webSession": ["webview"], "onWebEvent": ["webview"], "webHandlesDrags": ["webview"], "onScrollEdge": ["scroll", "list"], "ariaControls": ["input", "textarea", "button"], "ariaAutocomplete": ["input", "button"], "scaleRefs": ["page"], "defaultValue": ["checkbox", "switch", "slider", "select"], "checkedStyle": ["checkbox", "switch", "radio-option"], "modal": ["column"], "bleed": ["column", "scroll", "list", "image", "video", "webview", "vector"], "pageTitle": ["page"], "pickerRequest": ["input"], "ariaCurrent": ["button", "link", "link-box"], "keepsFocus": ["button"], "selectionRequest": ["input", "textarea", "search"], "onSelectionChange": ["input", "textarea", "search"], "onVisibleChange": ["scroll", "list"], "onPaste": ["input", "textarea", "search"] } };
  var options = { "snap-stop": ["normal", "always"], "aria-current": ["page", "step", "true"], "font-family": ["sans", "mono", "serif"], "font-style": ["normal", "italic"], "flex-wrap": ["nowrap", "wrap", "wrap-reverse"], "flex-direction": ["row", "column", "row-reverse", "column-reverse"], "display": ["flex", "grid", "none", "block", "inline-flex", "inline-block"], "scroll-axis": ["horizontal", "vertical"], "word-break": ["normal", "break-all", "keep-all"], "overflow-wrap": ["normal", "break-word", "anywhere"], "text-decoration-line": ["none", "line-through", "underline"], "text-alignment": ["start", "end", "left", "center", "right", "justify"], "alignment": ["start", "center", "end", "stretch", "baseline"], "justification": ["start", "center", "end", "between", "around", "evenly"], "position": ["relative", "absolute", "fixed", "sticky"], "overflow": ["visible", "hidden"], "text-transform": ["none", "uppercase", "lowercase", "capitalize"], "background-clip": ["border-box", "text"], "white-space": ["normal", "nowrap", "pre", "pre-wrap", "pre-line"], "text-overflow": ["clip", "ellipsis"], "sizing-keyword": ["fit-content", "max-content", "min-content"], "border-style": ["solid", "dashed", "dotted"], "slider-thumb": ["lower", "upper"], "overscroll": ["auto", "contain", "none"], "scroll-snap": ["none", "x", "y"], "snap-align": ["none", "start", "center", "end"], "grid-auto-flow": ["row", "column", "row-dense", "column-dense"], "align-content": ["start", "center", "end", "between", "around", "evenly", "stretch"], "direction": ["ltr", "rtl", "auto"], "role": ["heading", "list", "listitem", "dialog", "alert", "status", "grid", "gridcell", "listbox", "option", "menu", "menuitem", "progressbar", "alertdialog", "group", "tooltip", "tablist", "tab", "tabpanel", "radiogroup", "radio", "toolbar", "tree", "treeitem", "row", "menuitemcheckbox", "menuitemradio", "img", "combobox", "spinbutton"], "anchor-area": ["bottom span-inline-end", "bottom span-inline-start", "bottom", "top span-inline-end", "top span-inline-start", "top", "inline-end span-block-end", "inline-end span-block-start", "inline-end", "inline-start span-block-end", "inline-start span-block-start", "inline-start"], "live-region": ["off", "polite", "assertive"], "input-keyboard": ["text", "number", "email", "phone", "search", "date", "time", "decimal"], "auto-complete-mode": ["none", "list"], "auto-complete": ["off", "on", "username", "current-password", "new-password", "one-time-code", "email", "tel", "name", "given-name", "family-name", "street-address", "postal-code", "country", "cc-number", "cc-exp", "cc-csc", "bday"], "enter-key-hint": ["enter", "done", "go", "next", "previous", "search", "send"], "auto-capitalize": ["none", "sentences", "words", "characters"], "transition-property": ["opacity", "transform", "bg", "fg", "border", "shadow", "layout", "size", "d"], "user-select": ["none", "text", "all"], "touch-target": ["auto", "none"], "pointer-axis": ["x", "y", "both"], "bleed": ["all", "top", "bottom", "horizontal"] };
  var commonProps = ["style", "className", "key", "data-testid", "dir", "children", "id", "role", "aria-hidden", "aria-live", "aria-pressed", "aria-selected", "aria-expanded", "aria-busy", "aria-invalid", "aria-required", "aria-describedby"];
  var authoringRules = { "role": { "tab": [{ "tags": ["button"], "error": 'role="tab" belongs on a <button>' }], "radio": [{ "tags": ["button"], "error": 'role="radio" belongs on a <button>' }], "menuitem": [{ "tags": ["button", "link-box"], "error": 'role="menuitem" belongs on a <button> or a <link-box>' }], "menuitemcheckbox": [{ "tags": ["button"], "error": 'role="menuitemcheckbox" belongs on a <button>' }], "menuitemradio": [{ "tags": ["button"], "error": 'role="menuitemradio" belongs on a <button>' }], "treeitem": [{ "tags": ["button"], "error": 'role="treeitem" belongs on a <button>' }], "tablist": [{ "tags": ["view"], "error": 'role="tablist" belongs on a <view>' }], "tabpanel": [{ "tags": ["view"], "error": 'role="tabpanel" belongs on a <view>' }], "radiogroup": [{ "tags": ["view"], "error": 'role="radiogroup" belongs on a <view>' }], "toolbar": [{ "tags": ["view"], "error": 'role="toolbar" belongs on a <view>' }], "tree": [{ "tags": ["view", "scroll", "list"], "error": 'role="tree" belongs on a <view> or a <scroll> or a <list>' }], "row": [{ "tags": ["view"], "error": 'role="row" belongs on a <view>' }], "combobox": [{ "tags": ["input", "button"], "error": 'role="combobox" belongs on an <input> or a <button>' }], "spinbutton": [{ "tags": ["input"], "error": 'role="spinbutton" belongs on an <input>' }], "alertdialog": [{ "tags": ["dialog"], "error": 'role="alertdialog" goes on a <dialog>' }], "img": [{ "tags": ["view", "text"], "with": ["aria-label"], "error": 'role="img" goes on a <view> or <text> with an aria-label, which is spoken in place of its content' }], "dialog": [{ "tags": ["view"], "with": ["popover"], "error": 'role="dialog" goes on a popover <view>, or is the <dialog> element' }] }, "elementRole": { "page": { "roles": [], "error": "the <page> keeps its own role; put a role on an element inside it" }, "text": { "roles": ["heading", "listitem", "alert", "status", "tooltip", "gridcell", "option", "img"], "error": "a <text> takes role heading, listitem, alert, status, tooltip, gridcell, option or img; put other roles on a <view> or <button>" }, "button": { "roles": ["tab", "radio", "option", "gridcell", "treeitem", "menuitem", "menuitemcheckbox", "menuitemradio", "combobox"], "error": "a <button> takes an item role (tab, radio, option, gridcell, treeitem, menuitem, menuitemcheckbox, menuitemradio) or combobox; put container roles on a <view>" }, "scroll": { "roles": ["list", "listbox", "grid", "menu", "group", "tree"], "error": "a <scroll> takes a container role: list, listbox, grid, menu, group or tree" }, "list": { "roles": ["list", "listbox", "grid", "menu", "group", "tree"], "error": "a <List> takes a container role: list, listbox, grid, menu, group or tree" }, "input": { "roles": ["combobox", "spinbutton"], "error": "an <input> takes role combobox or spinbutton only" }, "image": { "roles": [], "error": "an <image> keeps its own role" }, "vector": { "roles": [], "error": "a <vector> keeps its own role" }, "plot": { "roles": [], "error": "a <plot> keeps its own role" }, "textarea": { "roles": [], "error": "a <textarea> keeps its own role" }, "checkbox": { "roles": [], "error": "a <checkbox> keeps its own role" }, "switch": { "roles": [], "error": "a <switch> keeps its own role" }, "radio": { "roles": [], "error": "a <radio> keeps its own role" }, "slider": { "roles": [], "error": "a <slider> keeps its own role" }, "select": { "roles": [], "error": "a <select> keeps its own role" }, "video": { "roles": [], "error": "a <video> keeps its own role" }, "webview": { "roles": [], "error": "a <webview> keeps its own role" }, "link": { "roles": [], "error": "a <link> keeps its own role" }, "link-box": { "roles": ["menuitem"], "error": 'a block <link> takes only role="menuitem", as a navigation row of a menu' }, "dialog": { "roles": ["alertdialog"], "implied": "dialog", "error": 'a <dialog> keeps its own role; only role="alertdialog" marks one that interrupts' } }, "prop": { "open": [{ "on": ["view"], "with": ["popover"], "error": "open belongs to a popover; add popover" }], "onToggle": [{ "on": ["view"], "with": ["popover"], "error": "onToggle belongs to a popover; add popover" }], "closeAfter": [{ "on": ["view"], "with": ["popover"], "error": "closeAfter belongs to a popover; add popover" }], "anchor": [{ "on": ["view"], "with": ["popover"], "error": "anchor belongs to a popover; add popover" }], "onCloseRequest": [{ "on": ["view"], "with": ["popover"], "error": "onCloseRequest belongs to a popover; add popover" }, { "on": ["view"], "not": { "popover": ["manual"] }, "error": "onCloseRequest belongs on a popover that is not manual; nothing but your code closes a manual one" }], "dragSnapPoints": [{ "on": ["view", "button", "dialog"], "with": ["drag"], "error": 'dragSnapPoints belongs to an element with drag; add drag="x" or drag="y"' }], "bind": [{ "on": ["view", "button", "dialog"], "with": ["drag"], "error": 'bind belongs to an element with drag; add drag="x" or drag="y"' }, { "on": ["view", "button", "dialog", "scroll"], "without": ["snapIndex"], "error": "bind and snapIndex both set the snap index; bind a useLocal cell, or pass snapIndex and update it from onSnapChange" }], "snapIndex": [{ "on": ["view", "button", "dialog"], "with": ["drag"], "error": 'snapIndex belongs to an element with drag; add drag="x" or drag="y"' }, { "on": ["view", "button", "dialog", "scroll"], "without": ["bind"], "error": "snapIndex and bind both set the snap index; bind a useLocal cell, or pass snapIndex and update it from onSnapChange" }, { "on": ["view", "button", "dialog", "scroll"], "with": ["onSnapChange"], "error": "snapIndex controls the position, so it needs onSnapChange to hear where a swipe settled; use bind to let the element keep the index itself" }], "onSnapChange": [{ "on": ["view", "button", "dialog"], "with": ["drag"], "error": 'onSnapChange belongs to an element with drag; add drag="x" or drag="y"' }, { "on": ["view", "button", "dialog", "scroll"], "with": ["snapIndex"], "error": "onSnapChange reports the snap index you pass in snapIndex; add snapIndex, or bind a useLocal cell instead" }], "onDragStart": [{ "on": ["view", "button", "dialog"], "with": ["drag"], "error": 'onDragStart belongs to an element with drag; add drag="x" or drag="y"' }], "onDragEnd": [{ "on": ["view", "button", "dialog"], "with": ["drag"], "error": 'onDragEnd belongs to an element with drag; add drag="x" or drag="y"' }], "aria-actions": [{ "on": ["view", "button", "dialog"], "with": ["drag"], "error": 'aria-actions belongs to an element with drag; add drag="x" or drag="y"' }], "pointerAxis": [{ "on": ["view", "button", "vector"], "with": ["onPointer"], "error": "pointerAxis needs onPointer on the same element; add onPointer or remove pointerAxis" }], "src": [{ "on": ["video"], "without": ["sources"], "error": "src and sources both name the video; give src for one file, or sources for a choice of quality" }], "sources": [{ "on": ["video"], "without": ["src"], "error": "sources and src both name the video; give src for one file, or sources for a choice of quality" }], "aria-valuenow": [{ "not": { "role": ["combobox"] }, "error": 'role="combobox" takes aria-valuetext only, a string' }, { "any": [{ "role": ["progressbar", "spinbutton"] }, { "with": ["onPointer"] }], "error": 'aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element' }], "aria-valuemin": [{ "not": { "role": ["combobox"] }, "error": 'role="combobox" takes aria-valuetext only, a string' }, { "any": [{ "role": ["progressbar", "spinbutton"] }, { "with": ["onPointer"] }], "error": 'aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element' }], "aria-valuemax": [{ "not": { "role": ["combobox"] }, "error": 'role="combobox" takes aria-valuetext only, a string' }, { "any": [{ "role": ["progressbar", "spinbutton"] }, { "with": ["onPointer"] }], "error": 'aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element' }], "aria-valuetext": [{ "any": [{ "role": ["progressbar", "spinbutton", "combobox"] }, { "with": ["onPointer"] }, { "tags": ["slider"] }], "error": 'aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element' }], "headingLevel": [{ "role": ["heading"], "error": 'headingLevel belongs on role="heading"' }], "aria-level": [{ "role": ["treeitem"], "error": 'aria-level belongs on role="treeitem"' }], "aria-controls": [{ "any": [{ "role": ["combobox"] }, { "tags": ["textarea"] }], "error": 'aria-controls belongs on role="combobox" or a <textarea> whose keys move through a list' }], "aria-autocomplete": [{ "role": ["combobox"], "error": 'aria-controls and aria-autocomplete belong on role="combobox"' }], "aria-checked": [{ "value": "mixed", "role": ["menuitemcheckbox"], "error": 'aria-checked="mixed" goes on role="menuitemcheckbox"; a <checkbox> takes indeterminate' }, { "role": ["radio", "menuitemcheckbox", "menuitemradio"], "error": 'aria-checked goes on role="radio", "menuitemcheckbox" or "menuitemradio"; a checkbox has its own checked' }], "aria-modal": [{ "role": ["dialog", "alertdialog"], "error": 'aria-modal belongs on a popover with role="dialog"' }, { "value": false, "tags": ["view"], "error": 'a <dialog> is always modal; a non-modal dialog is a popover with role="dialog"' }], "onPage": [{ "tags": ["view"], "role": ["grid"], "error": 'onPage belongs to a <view role="grid">' }], "keepOpen": [{ "role": ["menuitem", "menuitemcheckbox", "menuitemradio"], "error": 'keepOpen belongs on a menu item: role="menuitem", "menuitemcheckbox" or "menuitemradio"' }] }, "run": { "span": ["style", "className", "key", "data-testid", "dir", "aria-hidden", "children"], "link": ["style", "className", "key", "data-testid", "dir", "children", "aria-describedby"] }, "trait": { "popover": { "key": "popover", "on": ["view"], "props": ["open", "onToggle", "closeAfter", "anchor", "onCloseRequest"], "error": "{prop} belongs to a popover; add popover", "forms": [{ "values": [true, "auto", "hint"] }, { "values": ["manual"], "without": { "onCloseRequest": "onCloseRequest belongs on a popover that is not manual; nothing but your code closes a manual one" } }] }, "drag": { "key": "drag", "on": ["view", "button", "dialog"], "props": ["dragSnapPoints", "bind", "snapIndex", "onSnapChange", "onDragStart", "onDragEnd", "aria-actions"], "error": '{prop} belongs to an element with drag; add drag="x" or drag="y"' }, "pointer": { "key": "onPointer", "on": ["view", "button", "vector"], "props": ["pointerAxis"], "error": "pointerAxis needs onPointer on the same element; add onPointer or remove pointerAxis" } }, "exclusive": { "snap": { "on": ["view", "button", "dialog", "scroll"], "props": ["bind", "snapIndex"], "with": { "snapIndex": ["onSnapChange"] }, "error": "{prop} and {other} both set the snap index; bind a useLocal cell, or pass snapIndex and update it from onSnapChange", "companion": "{prop} reports the snap index you pass in snapIndex; add snapIndex, or bind a useLocal cell instead", "needs": "{prop} controls the position, so it needs onSnapChange to hear where a swipe settled; use bind to let the element keep the index itself" }, "videoSource": { "on": ["video"], "props": ["src", "sources"], "orElse": "player", "error": "{prop} and {other} both name the video; give src for one file, or sources for a choice of quality", "missing": "a <video> needs a source: add src (one file) or sources (a choice of quality), or player={useVideo(name)} to show a player another <video> gives a source" } } };
  var roles = ["canvas", "surface", "surface-raised", "surface-sunken", "line", "text", "text-muted", "text-faint", "accent", "on-accent", "accent-soft", "on-accent-soft", "danger", "on-danger", "danger-soft", "success", "on-success", "success-soft", "warning", "on-warning", "warning-soft", "focus", "scrim", "chart-1", "chart-2", "chart-3", "chart-4", "chart-5", "line-strong", "selection", "shadow", "media", "on-media", "surface-overlay", "knob", "success-mark", "warning-mark", "danger-mark", "media-canvas", "info", "on-info", "info-soft", "info-mark", "hue-1", "hue-2", "hue-3", "hue-4", "hue-5", "hue-6", "hue-7", "hue-8", "hue-1-soft", "hue-2-soft", "hue-3-soft", "hue-4-soft", "hue-5-soft", "hue-6-soft", "hue-7-soft", "hue-8-soft", "chart-6", "chart-7", "chart-8", "accent-solid", "on-accent-solid", "danger-solid", "on-danger-solid", "accent-solid-hover", "accent-solid-pressed", "danger-solid-hover", "danger-solid-pressed", "neutral-soft"];
  var scales = [{ "name": "text.xs", "family": "type" }, { "name": "text.sm", "family": "type" }, { "name": "text.base", "family": "type" }, { "name": "text.lg", "family": "type" }, { "name": "text.xl", "family": "type" }, { "name": "text.2xl", "family": "type" }, { "name": "text.3xl", "family": "type" }, { "name": "text.4xl", "family": "type" }, { "name": "leading.xs", "family": "leading" }, { "name": "leading.sm", "family": "leading" }, { "name": "leading.base", "family": "leading" }, { "name": "leading.lg", "family": "leading" }, { "name": "leading.xl", "family": "leading" }, { "name": "leading.2xl", "family": "leading" }, { "name": "leading.3xl", "family": "leading" }, { "name": "leading.4xl", "family": "leading" }, { "name": "radius.sm", "family": "radius" }, { "name": "radius.DEFAULT", "family": "radius" }, { "name": "radius.md", "family": "radius" }, { "name": "radius.lg", "family": "radius" }, { "name": "radius.xl", "family": "radius" }, { "name": "radius.2xl", "family": "radius" }, { "name": "radius.3xl", "family": "radius" }, { "name": "space", "family": "space" }, { "name": "duration.fast", "family": "duration" }, { "name": "duration.base", "family": "duration" }, { "name": "duration.slow", "family": "duration" }, { "name": "ease.standard", "family": "ease" }, { "name": "ease.enter", "family": "ease" }, { "name": "ease.exit", "family": "ease" }, { "name": "em", "family": "em" }];
  var scaleDefaults = [12, 13, 15, 17, 20, 24, 28, 34, 16, 18, 22, 24, 28, 32, 34, 40, 6, 8, 8, 10, 14, 20, 28, 4, 150, 200, 300, [0, 0, 0.2, 1], [0, 0, 0.2, 1], [0.4, 0, 1, 1], null];
  var scaledFields = { "type": ["fontSize", "lineHeight", "letterSpacing"], "leading": ["lineHeight"], "radius": ["radius", "radiusTopLeft", "radiusTopRight", "radiusBottomRight", "radiusBottomLeft", "radiusStartStart", "radiusStartEnd", "radiusEndStart", "radiusEndEnd"], "space": ["lineHeight", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "gap", "rowGap", "columnGap", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "insetTop", "insetRight", "insetBottom", "insetLeft", "insetInlineStart", "insetInlineEnd", "flexBasis", "width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight"], "em": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "gap", "rowGap", "columnGap", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "insetTop", "insetRight", "insetBottom", "insetLeft", "insetInlineStart", "insetInlineEnd"], "duration": ["transition"], "ease": ["transition"] };

  // src/runtime/fonts.js
  var declared = [];
  var pageFamily;
  function configureFonts(faces, family2) {
    if (!Array.isArray(faces) || faces.length > 32) throw Error("UI_FONT: invalid font list");
    if (family2 !== void 0 && typeof family2 !== "string") throw Error("UI_FONT: invalid page family");
    declared = faces.map(({ family: family3, src, weight, style: style2 }) => ({ family: String(family3), src: String(src), ...weight ? { weight: String(weight) } : {}, ...style2 ? { style: String(style2) } : {} }));
    pageFamily = family2;
  }
  var declaredFamily = (name) => declared.some((face) => face.family === name);
  var takeFonts = () => JSON.stringify(declared);

  // src/generated/wire.js
  var maxField = 289;
  var ignorableFields = ["stateInvalid", "stateRequired", "ariaDescribedBy", "ariaActiveDescendant", "ariaCurrent"];
  var tags = { "column": 0, "reserved-1": 1, "text": 2, "button": 3, "reserved-4": 4, "scroll": 5, "list": 6, "image": 7, "input": 8, "textarea": 9, "checkbox": 10, "switch": 11, "radio": 12, "radio-option": 13, "search": 14, "slider": 15, "link-box": 16, "reserved-17": 17, "plot": 18, "vector": 19, "page": 20, "span": 21, "link": 22, "select": 23, "video": 24, "webview": 25 };
  var field = { "width": 0, "height": 1, "gap": 2, "padding": 3, "fontSize": 4, "color": 5, "background": 6, "grow": 7, "onClick": 8, "enabled": 9, "radius": 10, "listId": 11, "listCount": 12, "listLeading": 13, "listTrailing": 14, "listRowIndex": 15, "listAnchor": 16, "onReachStart": 17, "onReachEnd": 18, "imageSrc": 19, "imageFit": 20, "inputValue": 21, "placeholder": 22, "secure": 23, "keyboard": 24, "autoFocus": 25, "onInput": 26, "onSubmit": 27, "minWidth": 28, "minHeight": 29, "maxWidth": 30, "maxHeight": 31, "shrink": 32, "align": 33, "justify": 34, "alignSelf": 35, "fontWeight": 36, "lineHeight": 37, "fontFamily": 38, "checked": 39, "onCheckedChange": 40, "accessibilityLabel": 41, "sliderValue": 42, "sliderMin": 43, "sliderMax": 44, "sliderStep": 45, "onSliderChange": 46, "gridColumns": 47, "offsetX": 48, "offsetY": 49, "textLines": 50, "plotValues": 51, "plotKind": 52, "plotColors": 53, "plotSmooth": 54, "plotArea": 55, "onPlotSelect": 56, "plotSelectedIndex": 57, "opacity": 58, "paddingTop": 59, "paddingRight": 60, "paddingBottom": 61, "paddingLeft": 62, "wordBreak": 63, "overflowWrap": 64, "borderWidth": 65, "borderColor": 66, "borderTopWidth": 67, "borderRightWidth": 68, "borderBottomWidth": 69, "borderLeftWidth": 70, "textAlign": 71, "backgroundGradient": 72, "boxShadow": 73, "widthPercent": 74, "heightPercent": 75, "minWidthPercent": 76, "minHeightPercent": 77, "maxWidthPercent": 78, "maxHeightPercent": 79, "vectorSource": 80, "textDecorationLine": 81, "heightAuto": 82, "submitOnEnter": 83, "fontStyle": 84, "plotDomainMin": 85, "plotDomainMax": 86, "flexWrap": 87, "plotPieLayout": 88, "letterSpacing": 89, "radiusTopLeft": 90, "radiusTopRight": 91, "radiusBottomRight": 92, "radiusBottomLeft": 93, "plotMarkerEmphasisScale": 94, "backdropBlur": 95, "backdropSaturation": 96, "placeholderColor": 97, "focusStyle": 98, "flexBasis": 99, "scrollAxis": 100, "animation": 101, "listItemKeys": 102, "flexDirection": 103, "display": 104, "imageAlt": 105, "imageRetryKey": 106, "onImageLoad": 107, "onImageError": 108, "marginTop": 109, "marginRight": 110, "marginBottom": 111, "marginLeft": 112, "position": 113, "insetTop": 114, "insetRight": 115, "insetBottom": 116, "insetLeft": 117, "zIndex": 118, "overflow": 119, "rowGap": 120, "columnGap": 121, "textTransform": 122, "whiteSpace": 123, "textOverflow": 124, "pressedStyle": 125, "hoverStyle": 126, "widthKeyword": 127, "heightKeyword": 128, "borderStyle": 129, "transform": 130, "fontVariantNumeric": 131, "overscroll": 132, "aspectRatio": 133, "maxWidthCh": 134, "gridTemplateColumns": 135, "gridTemplateRows": 136, "gridAutoColumns": 137, "gridAutoRows": 138, "gridAutoFlow": 139, "gridColumn": 140, "gridRow": 141, "justifyItems": 142, "justifySelf": 143, "alignContent": 144, "order": 145, "direction": 146, "marginInlineStart": 147, "marginInlineEnd": 148, "paddingInlineStart": 149, "paddingInlineEnd": 150, "insetInlineStart": 151, "insetInlineEnd": 152, "borderInlineStartWidth": 153, "borderInlineEndWidth": 154, "radiusStartStart": 155, "radiusStartEnd": 156, "radiusEndStart": 157, "radiusEndEnd": 158, "role": 159, "headingLevel": 160, "accessibilityHidden": 161, "accessibilityLiveRegion": 162, "maxLength": 163, "autoComplete": 164, "enterKeyHint": 165, "autoCapitalize": 166, "spellCheck": 167, "inputMin": 168, "inputMax": 169, "inputStep": 170, "transition": 171, "userSelect": 172, "focusRequest": 173, "statePressed": 174, "stateSelected": 175, "stateExpanded": 176, "stateBusy": 177, "selectOptions": 178, "onSelect": 179, "rows": 180, "maxRows": 181, "imagePlaceholder": 182, "imagePosition": 183, "childSlot": 184, "startingStyle": 185, "exitStyle": 186, "focusWithinStyle": 187, "dataAttributes": 188, "backgroundClip": 189, "maskImage": 190, "filterBlur": 191, "groupPressedStyle": 192, "anchorTo": 193, "scrollSnap": 194, "snapAlign": 195, "topLayer": 196, "anchorArea": 197, "anchorName": 198, "drag": 199, "onDragEvent": 200, "dragScrim": 201, "elementId": 202, "ariaActions": 203, "snapStop": 204, "snapIndex": 205, "onSnapChange": 206, "videoSrc": 207, "videoPlayer": 208, "videoPoster": 209, "videoPaused": 210, "videoMuted": 211, "videoLoop": 212, "videoRate": 213, "videoVolume": 214, "onVideoEvent": 215, "videoSources": 216, "ariaValueNow": 217, "ariaValueMin": 218, "ariaValueMax": 219, "ariaValueText": 220, "stateInvalid": 221, "stateRequired": 222, "ariaDescribedBy": 223, "stateChecked": 224, "focusVisibleStyle": 225, "trackStyle": 226, "rangeStyle": 227, "secondaryStyle": 228, "thumbStyle": 229, "sliderSecondary": 230, "interestFor": 231, "onPointerEnter": 232, "onPointerLeave": 233, "ring": 234, "hint": 235, "onSliderPress": 236, "closeAfter": 237, "touchTarget": 238, "minWidthCh": 239, "widthCh": 240, "displayValue": 241, "sizeText": 242, "alignSafe": 243, "onLayout": 244, "onPointer": 245, "pointerAxis": 246, "onFocus": 247, "onBlur": 248, "readOnly": 249, "keyMap": 250, "onKeyDown": 251, "ariaActiveDescendant": 252, "mixed": 253, "onContextMenu": 254, "onPage": 255, "textDecorationStyle": 256, "textDecorationColor": 257, "sliderOther": 258, "sliderThumb": 259, "webSrc": 260, "webSession": 261, "onWebEvent": 262, "webHandlesDrags": 263, "onScrollEdge": 264, "ariaControls": 265, "ariaAutocomplete": 266, "scaleRefs": 267, "minWidthKeyword": 268, "minHeightKeyword": 269, "maxWidthKeyword": 270, "maxHeightKeyword": 271, "defaultValue": 272, "checkedStyle": 273, "hasCheckedStyle": 274, "modal": 275, "sideTopStyle": 276, "sideBottomStyle": 277, "sideStartStyle": 278, "sideEndStyle": 279, "bleed": 280, "pageTitle": 281, "pickerRequest": 282, "ariaLevel": 283, "ariaCurrent": 284, "keepsFocus": 285, "selectionRequest": 286, "onSelectionChange": 287, "onVisibleChange": 288, "onPaste": 289 };
  var fieldOptions = { "keyboard": ["text", "number", "email", "phone", "search", "date", "time", "decimal"], "align": ["start", "center", "end", "stretch", "baseline"], "justify": ["start", "center", "end", "between", "around", "evenly"], "alignSelf": ["start", "center", "end", "stretch", "baseline"], "wordBreak": ["normal", "break-all", "keep-all"], "overflowWrap": ["normal", "break-word", "anywhere"], "textAlign": ["start", "end", "left", "center", "right", "justify"], "textDecorationLine": ["none", "line-through", "underline"], "fontStyle": ["normal", "italic"], "flexWrap": ["nowrap", "wrap", "wrap-reverse"], "scrollAxis": ["horizontal", "vertical"], "flexDirection": ["row", "column", "row-reverse", "column-reverse"], "display": ["flex", "grid", "none", "block", "inline-flex", "inline-block"], "position": ["relative", "absolute", "fixed", "sticky"], "overflow": ["visible", "hidden"], "textTransform": ["none", "uppercase", "lowercase", "capitalize"], "whiteSpace": ["normal", "nowrap", "pre", "pre-wrap", "pre-line"], "textOverflow": ["clip", "ellipsis"], "widthKeyword": ["fit-content", "max-content", "min-content"], "heightKeyword": ["fit-content", "max-content", "min-content"], "borderStyle": ["solid", "dashed", "dotted"], "overscroll": ["auto", "contain", "none"], "gridAutoFlow": ["row", "column", "row-dense", "column-dense"], "justifyItems": ["start", "center", "end", "stretch", "baseline"], "justifySelf": ["start", "center", "end", "stretch", "baseline"], "alignContent": ["start", "center", "end", "between", "around", "evenly", "stretch"], "direction": ["ltr", "rtl", "auto"], "role": ["heading", "list", "listitem", "dialog", "alert", "status", "grid", "gridcell", "listbox", "option", "menu", "menuitem", "progressbar", "alertdialog", "group", "tooltip", "tablist", "tab", "tabpanel", "radiogroup", "radio", "toolbar", "tree", "treeitem", "row", "menuitemcheckbox", "menuitemradio", "img", "combobox", "spinbutton"], "accessibilityLiveRegion": ["off", "polite", "assertive"], "autoComplete": ["off", "on", "username", "current-password", "new-password", "one-time-code", "email", "tel", "name", "given-name", "family-name", "street-address", "postal-code", "country", "cc-number", "cc-exp", "cc-csc", "bday"], "enterKeyHint": ["enter", "done", "go", "next", "previous", "search", "send"], "autoCapitalize": ["none", "sentences", "words", "characters"], "userSelect": ["none", "text", "all"], "backgroundClip": ["border-box", "text"], "scrollSnap": ["none", "x", "y"], "snapAlign": ["none", "start", "center", "end"], "anchorArea": ["bottom span-inline-end", "bottom span-inline-start", "bottom", "top span-inline-end", "top span-inline-start", "top", "inline-end span-block-end", "inline-end span-block-start", "inline-end", "inline-start span-block-end", "inline-start span-block-start", "inline-start"], "snapStop": ["normal", "always"], "touchTarget": ["auto", "none"], "pointerAxis": ["x", "y", "both"], "textDecorationStyle": ["solid", "dashed", "dotted"], "sliderThumb": ["lower", "upper"], "ariaAutocomplete": ["none", "list"], "minWidthKeyword": ["fit-content", "max-content", "min-content"], "minHeightKeyword": ["fit-content", "max-content", "min-content"], "maxWidthKeyword": ["fit-content", "max-content", "min-content"], "maxHeightKeyword": ["fit-content", "max-content", "min-content"], "bleed": ["all", "top", "bottom", "horizontal"], "ariaCurrent": ["page", "step", "true"] };

  // src/style/color.js
  var names = new Set(roles);
  var alpha = (percent2) => percent2 === void 0 || Number(percent2) <= 100;
  function isRole(value) {
    const match = typeof value === "string" && /^([a-z0-9-]+)(?:\/(\d{1,3}))?$/.exec(value);
    return Boolean(match) && names.has(match[1]) && alpha(match[2]);
  }
  function appColorName(value) {
    const match = typeof value === "string" && /^app\.([a-z][a-z0-9]*(?:-[a-z0-9]+)*)(?:\/(\d{1,3}))?$/.exec(value);
    return match && alpha(match[2]) ? match[1] : void 0;
  }
  var isColorName = (value) => isRole(value) || appColorName(value) !== void 0;
  var hex = /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/;
  var look = /^light-dark\(([^,()]+),([^,()]+)\)$/;
  function isPaintColor(value) {
    if (typeof value !== "string") return false;
    const pair = look.exec(value);
    return pair ? [pair[1], pair[2]].every((side) => hex.test(side.trim()) || isColorName(side.trim())) : hex.test(value) || isColorName(value);
  }
  var appList = (appColors) => appColors?.size ? `; this app's colours: ${[...appColors].map((name) => `app.${name}`).join(", ")}` : "";
  function colorProblem(value, where2 = "a page colour", appColors) {
    if (value === "currentColor" || value === "transparent" || /^#[0-9a-fA-F]{6}00$/.test(value) || isRole(value)) return void 0;
    const app = appColorName(value);
    if (app !== void 0) return !appColors || appColors.has(app) ? void 0 : `UI_COLOR: ${JSON.stringify(value)} is not in this app's theme${appList(appColors)}`;
    return `UI_COLOR: ${JSON.stringify(value)} is not a role; ${where2} uses a role such as 'surface', 'text-muted', 'line', 'accent' or 'accent/12'${appList(appColors)}`;
  }
  function wireColorProblem(value, where2) {
    return look.test(value) ? isPaintColor(value) ? void 0 : colorProblem(value, where2) : hex.test(value) ? void 0 : colorProblem(value, where2);
  }
  function seriesProblem(value, appColors) {
    if (isRole(value)) return void 0;
    const app = appColorName(value);
    if (app !== void 0 && (!appColors || appColors.has(app))) return void 0;
    return `UI_COLOR: ${JSON.stringify(value)} is not a role; plot series use 'chart-1' to 'chart-8'${appList(appColors)}`;
  }
  var defaultSeries = ["chart-1", "chart-2", "chart-3", "chart-4", "chart-5"];

  // src/style/shadow.js
  var show = (value) => {
    if (typeof value === "number") return String(value);
    try {
      return (JSON.stringify(value) ?? typeof value).slice(0, 40);
    } catch {
      return typeof value;
    }
  };
  var COLOR_FORMS = "'currentColor', #RRGGBB/#RRGGBBAA or a theme role";
  function normalizeShadows(value) {
    if (value === "none") return value;
    const invalid2 = () => {
      throw Error(`UI_SHADOW: boxShadow takes 'none' or 0..8 layers of { offsetX?: -1024..1024, offsetY?: -1024..1024, blur?: 0..256, spread?: -1024..1024, color?: ${COLOR_FORMS} }; got ${show(value)}`);
    };
    if (!Array.isArray(value) || value.length > 8) invalid2();
    return value.map((shadow) => {
      if (!shadow || typeof shadow !== "object" || Array.isArray(shadow) || Object.keys(shadow).some((key) => !["offsetX", "offsetY", "blur", "spread", "color"].includes(key))) invalid2();
      const result = {};
      for (const key of ["offsetX", "offsetY", "blur", "spread"]) {
        const number = shadow[key] === void 0 ? 0 : shadow[key];
        if (typeof number !== "number" || !Number.isFinite(number) || (key === "blur" ? number < 0 || number > 256 : Math.abs(number) > 1024)) invalid2();
        result[key] = number;
      }
      result.color = shadow.color === void 0 ? "currentColor" : shadow.color;
      if (result.color !== "currentColor" && !isPaintColor(result.color)) invalid2();
      return result;
    });
  }
  var ringColor = (value, invalid2) => {
    const color = value === void 0 ? "currentColor" : value;
    if (color !== "currentColor" && !isPaintColor(color)) invalid2();
    return color;
  };
  function normalizeRing(value) {
    if (value === "none") return value;
    const invalid2 = () => {
      throw Error(`UI_SHADOW: ring takes 'none' or { outer?: { width: 0..1024, offset?: 0..1024, color?: ${COLOR_FORMS}, offsetColor?: ${COLOR_FORMS} }, inner?: { width: 0..1024, color?: ${COLOR_FORMS} } }; got ${show(value)}`);
    };
    const plain3 = (object, keys2) => object && typeof object === "object" && !Array.isArray(object) && Object.keys(object).every((key) => keys2.includes(key));
    const size = (number) => typeof number === "number" && Number.isFinite(number) && number >= 0 && number <= 1024;
    if (!plain3(value, ["outer", "inner"])) invalid2();
    const result = {};
    if (value.outer !== void 0) {
      const { width, offset = 0, color, offsetColor } = value.outer;
      if (!plain3(value.outer, ["width", "offset", "color", "offsetColor"]) || !size(width) || !size(offset)) invalid2();
      result.outer = { width, offset, color: ringColor(color, invalid2) };
      if (offsetColor !== void 0) result.outer.offsetColor = ringColor(offsetColor, invalid2);
    }
    if (value.inner !== void 0) {
      if (!plain3(value.inner, ["width", "color"]) || !size(value.inner.width)) invalid2();
      result.inner = { width: value.inner.width, color: ringColor(value.inner.color, invalid2) };
    }
    return result;
  }

  // src/style/hints.js
  function lineHeightProblem(value) {
    if (typeof value !== "number" || !(value > limits.lineHeightMultiplier)) return void 0;
    return `UI_STYLE_VALUE: lineHeight ${value} is a multiple of the font size (${value}\xD7), not points; write "${value}px", or a leading-* class`;
  }
  var container = /* @__PURE__ */ new Set([
    "alignItems",
    "alignContent",
    "justifyContent",
    "justifyItems",
    "flexDirection",
    "flexWrap",
    "gap",
    "rowGap",
    "columnGap",
    "gridTemplateColumns",
    "gridTemplateRows",
    "gridAutoColumns",
    "gridAutoRows",
    "gridAutoFlow"
  ]);
  var layouts = /* @__PURE__ */ new Set(["view", "scroll", "list", "button", "checkbox", "page"]);
  var boxes = {
    column: "<view>",
    row: '<view className="flex-row">',
    grid: '<view className="grid">',
    stack: '<view className="grid *:col-start-1 *:row-start-1">',
    spacer: "<view>",
    iframe: "<webview>",
    details: "<button aria-expanded> and a body shown from useLocal state",
    summary: "<button aria-expanded> and a body shown from useLocal state",
    progress: '<view role="progressbar" aria-valuenow>'
  };
  function tagHint(tag) {
    return Object.hasOwn(boxes, tag) ? `; use ${boxes[tag]}` : "";
  }
  var gridOnly = {
    flexWrap: "; grid items wrap into rows by themselves (use grid-cols-[repeat(auto-fit,minmax(\u2026,1fr))])",
    flexDirection: "; use grid-flow-col to fill columns first"
  };
  var gridRefuses = (props) => props.find((prop) => Object.hasOwn(gridOnly, prop));
  function gridProblem(name, prop) {
    return `UI_CLASS_ELEMENT: ${name} does not apply to <view className="grid">${gridOnly[prop]}`;
  }
  function classElementProblem(name, prop, tag) {
    let hint = "";
    if (prop === "justifyItems" && layouts.has(tag)) hint = "; on a flex element center with items-center justify-center";
    else if (container.has(prop) && !layouts.has(tag)) hint = "; layout classes go on the element that holds it";
    else hint = appliesTo(prop);
    return `UI_CLASS_ELEMENT: ${name} does not apply to <${blocks[tag]?.of ?? tag}>${hint}`;
  }
  function appliesTo(prop) {
    const takers = Object.keys(elements).filter((name) => elements[name].style.includes(prop));
    return takers.length && takers.length <= 3 ? `; it applies to ${takers.map((name) => `<${elements[name].component ?? name}>`).join(", ")}` : "";
  }

  // src/style/gradient.js
  var hex2 = /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/;
  var directions = ["top", "right", "bottom", "left", "top-right", "bottom-right", "bottom-left", "top-left"];
  var geometry = { linear: ["direction", "angle"], radial: ["shape", "size", "radii", "at"], conic: ["from", "at"] };
  var sizes = ["closest-side", "closest-corner", "farthest-side", "farthest-corner"];
  var angle = (value) => typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 36e3;
  function normalizeGradient(value) {
    if (value === "none") return value;
    const invalid2 = () => {
      throw Error("UI_GRADIENT: invalid gradient");
    };
    if (!value || typeof value !== "object" || Array.isArray(value)) invalid2();
    const kind = value.kind ?? "linear";
    const keys2 = geometry[kind] ?? invalid2();
    const pair = (v, low2, high2) => Array.isArray(v) && v.length === 2 && v.every((n) => typeof n === "number" && Number.isFinite(n) && n > low2 && n <= high2);
    if (Object.keys(value).some((key) => !["kind", "colorSpace", "stops", "layerSize", "layerPosition", ...keys2].includes(key)) || value.layerSize !== void 0 && !pair(value.layerSize, 0, 16) || value.layerPosition !== void 0 && !pair(value.layerPosition, -16.0001, 16) || value.direction !== void 0 && !directions.includes(value.direction) || value.angle !== void 0 && !angle(value.angle) || value.direction !== void 0 && value.angle !== void 0 || value.from !== void 0 && !angle(value.from) || value.shape !== void 0 && !["circle", "ellipse"].includes(value.shape) || value.size !== void 0 && !sizes.includes(value.size) || value.radii !== void 0 && (value.size !== void 0 || value.shape !== void 0 || !Array.isArray(value.radii) || value.radii.length !== 2 || value.radii.some((v) => typeof v !== "number" || !Number.isFinite(v) || v <= 0 || v > 16)) || value.at !== void 0 && (!Array.isArray(value.at) || value.at.length !== 2 || value.at.some((v) => typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > 16)) || value.colorSpace !== void 0 && !["srgb", "oklab"].includes(value.colorSpace) || !Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 16) invalid2();
    const [low, high] = kind === "linear" ? [-16, 16] : kind === "radial" ? [0, 16] : [0, 1];
    let previous = -Infinity;
    const stops = value.stops.map((stop) => {
      if (!stop || typeof stop !== "object" || Array.isArray(stop) || Object.keys(stop).some((key) => key !== "offset" && key !== "color") || typeof stop.offset !== "number" || !Number.isFinite(stop.offset) || stop.offset < low || stop.offset > high || stop.offset < previous || typeof stop.color !== "string" || !(hex2.test(stop.color) || isColorName(stop.color))) invalid2();
      previous = stop.offset;
      return { offset: stop.offset, color: stop.color };
    });
    const result = kind === "linear" ? {} : { kind };
    for (const key of [...keys2, "layerSize", "layerPosition"]) if (value[key] !== void 0) result[key] = Array.isArray(value[key]) ? [...value[key]] : value[key];
    return { ...result, colorSpace: value.colorSpace ?? "srgb", stops };
  }

  // src/runtime/data.js
  var frozen = /* @__PURE__ */ new WeakSet();
  function freeze(value) {
    Object.freeze(value);
    frozen.add(value);
    return value;
  }
  function isFrozen(value) {
    if (value === null || typeof value !== "object") return true;
    if (frozen.has(value)) return true;
    if (Object.isExtensible(value) || !Object.isFrozen(value)) return false;
    frozen.add(value);
    return true;
  }
  var extensions = /* @__PURE__ */ new WeakMap();
  var extended = (items) => extensions.get(items);
  function immutable(value, depth = 0, omitUndefined = false, budget) {
    if (depth > 64) throw Error("UI_LOCAL: value too deep");
    if (budget && (budget.left -= typeof value === "string" ? value.length + 2 : 8) < 0) throw Error(`UI_LIMIT: ${budget.what} payload too large`);
    if (value === null || typeof value === "string" || typeof value === "boolean") return value;
    if (typeof value === "number" && Number.isFinite(value)) return value;
    if (Array.isArray(value)) return freeze(value.map((v) => immutable(v, depth + 1, omitUndefined, budget)));
    if (value && Object.getPrototypeOf(value) === Object.prototype) {
      const out = {};
      for (const k of Object.keys(value)) {
        const v = value[k];
        if (omitUndefined && v === void 0) continue;
        if (budget) budget.left -= k.length + 3;
        const item = immutable(v, depth + 1, omitUndefined, budget);
        if (k === "__proto__") define(out, k, item);
        else out[k] = item;
      }
      return freeze(out);
    }
    throw Error("UI_LOCAL: expected JSON value");
  }
  function shared(value, previous, depth = 0) {
    if (depth > 64) throw Error("UI_LOCAL: value too deep");
    if (previous === void 0 || value === null || typeof value !== "object") return immutable(value, depth);
    if (Array.isArray(value)) {
      const old2 = Array.isArray(previous) ? previous : [];
      let same5 = Array.isArray(previous) && old2.length === value.length;
      const next = value.map((v, i) => {
        const item = shared(v, old2[i], depth + 1);
        same5 && (same5 = item === old2[i]);
        return item;
      });
      return same5 ? previous : freeze(next);
    }
    if (Object.getPrototypeOf(value) !== Object.prototype) return immutable(value, depth);
    const old = previous && typeof previous === "object" && Object.getPrototypeOf(previous) === Object.prototype ? previous : null;
    const entries2 = [];
    let same4 = old !== null;
    for (const key of Object.keys(value)) {
      const had = old !== null && Object.prototype.hasOwnProperty.call(old, key);
      const item = shared(value[key], had ? old[key] : void 0, depth + 1);
      entries2.push([key, item]);
      same4 && (same4 = had && item === old[key]);
    }
    return same4 && entries2.length === Object.keys(old).length ? old : freeze(Object.fromEntries(entries2));
  }
  function applyPatch(root, ops) {
    const fresh2 = /* @__PURE__ */ new Map();
    for (const op of ops) {
      if (op.path === "") {
        root = op.op === "remove" ? freeze({}) : shared(op.value, root);
        continue;
      }
      const tokens = op.path.slice(1).split("/");
      if (op.path.includes("~")) for (let i = 0; i < tokens.length; i++) tokens[i] = tokens[i].replace(/~1/g, "/").replace(/~0/g, "~");
      root = patchAt(root, tokens, 0, op, fresh2);
    }
    for (const [container2, origin] of fresh2) {
      freeze(container2);
      if (origin) extensions.set(container2, origin);
    }
    return root;
  }
  function patchAt(node, tokens, i, op, fresh2) {
    const key = tokens[i];
    const array = Array.isArray(node);
    if (i === tokens.length - 1) return patchLeaf(node, array, key, op, fresh2);
    const slot = array ? Number(key) : key;
    const had = array || has(node, key);
    let child = had ? node[slot] : void 0;
    if (child === void 0 && !array) child = /^\d+$/.test(tokens[i + 1]) ? [] : {};
    const next = patchAt(child, tokens, i + 1, op, fresh2);
    if (had && next === node[slot]) return node;
    const copy2 = mutable(node, fresh2);
    if (array) fresh2.set(copy2, null);
    define(copy2, slot, next);
    if (!had) sortKeys(copy2);
    return copy2;
  }
  function patchLeaf(node, array, key, op, fresh2) {
    if (array) {
      const index = key === "-" ? node.length : Number(key);
      if (op.op === "remove" && index >= node.length) return node;
      const copy3 = mutable(node, fresh2);
      if (op.op !== "add" || index < copy3.length) fresh2.set(copy3, null);
      if (op.op === "add" && index >= copy3.length) copy3.push(immutable(op.value));
      else if (op.op === "add") copy3.splice(index, 0, immutable(op.value));
      else if (op.op === "remove") copy3.splice(index, 1);
      else {
        while (copy3.length < index) copy3.push(null);
        copy3[index] = shared(op.value, node[index]);
      }
      return copy3;
    }
    const had = has(node, key);
    if (op.op === "remove") {
      if (!had) return node;
      const copy3 = mutable(node, fresh2);
      delete copy3[key];
      return copy3;
    }
    const value = shared(op.value, had ? node[key] : void 0);
    if (had && value === node[key]) return node;
    const copy2 = mutable(node, fresh2);
    define(copy2, key, value);
    if (!had) sortKeys(copy2);
    return copy2;
  }
  function sortKeys(object) {
    const entries2 = Object.entries(object).sort(([a], [b]) => compareKeys(a, b));
    for (const [key] of entries2) delete object[key];
    for (const [key, value] of entries2) define(object, key, value);
  }
  function compareKeys(a, b) {
    for (let i = 0; i < a.length && i < b.length; i++) {
      const x = a.codePointAt(i), y = b.codePointAt(i);
      if (x !== y) return x - y;
      if (x > 65535) i++;
    }
    return a.length - b.length;
  }
  function mutable(node, fresh2) {
    if (fresh2.has(node)) return node;
    const copy2 = Array.isArray(node) ? node.slice() : Object.fromEntries(Object.entries(node));
    fresh2.set(copy2, Array.isArray(node) && frozen.has(node) ? node : null);
    return copy2;
  }
  function define(target, key, value) {
    Object.defineProperty(target, key, { value, writable: true, enumerable: true, configurable: true });
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
  function validInputText(value) {
    return typeof value === "string" && utf8Bytes(value, 65536) <= 65536;
  }
  var has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);

  // src/style/lower.js
  var finite = (value) => typeof value === "number" && Number.isFinite(value);
  var percent = (value) => typeof value === "string" && /^(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) ? Number(value.slice(0, -1)) : void 0;
  var shorthand = (name) => Array.isArray(style[name]?.wire);
  var logical = {
    margin: ["marginInlineStart", "marginInlineEnd"],
    padding: ["paddingInlineStart", "paddingInlineEnd"],
    inset: ["insetInlineStart", "insetInlineEnd"],
    borderWidth: ["borderInlineStartWidth", "borderInlineEndWidth"],
    borderRadius: ["borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius"]
  };
  var longhands = Object.fromEntries(Object.entries(style).filter(([name]) => shorthand(name)).map(([name, entry]) => [name, [...Object.keys(style).filter((other) => !shorthand(other) && entry.wire.includes(style[other].wire)), ...logical[name] ?? []]]));
  var sides = [
    ["marginInlineStart", "marginLeft", "marginRight"],
    ["marginInlineEnd", "marginRight", "marginLeft"],
    ["paddingInlineStart", "paddingLeft", "paddingRight"],
    ["paddingInlineEnd", "paddingRight", "paddingLeft"],
    ["insetInlineStart", "left", "right"],
    ["insetInlineEnd", "right", "left"],
    ["borderInlineStartWidth", "borderLeftWidth", "borderRightWidth"],
    ["borderInlineEndWidth", "borderRightWidth", "borderLeftWidth"],
    ["borderStartStartRadius", "borderTopLeftRadius", "borderTopRightRadius"],
    ["borderStartEndRadius", "borderTopRightRadius", "borderTopLeftRadius"],
    ["borderEndStartRadius", "borderBottomLeftRadius", "borderBottomRightRadius"],
    ["borderEndEndRadius", "borderBottomRightRadius", "borderBottomLeftRadius"]
  ];
  var aliases = (direction) => {
    const map = {};
    for (const [flow, ltr, rtl] of sides) {
      const physical = direction === "rtl" ? rtl : ltr;
      (map[flow] ?? (map[flow] = [])).push(physical);
      (map[physical] ?? (map[physical] = [])).push(flow);
    }
    return map;
  };
  var aliasesByDirection = { ltr: aliases("ltr"), rtl: aliases("rtl") };
  function resetLonghands(result, layer) {
    for (const name of Object.keys(layer))
      for (const longhand of longhands[name] ?? []) if (!(longhand in layer)) delete result[longhand];
  }
  function mergeStyle(direction, ...layers) {
    const result = {};
    const alias = aliasesByDirection[direction] ?? aliasesByDirection.ltr;
    for (const layer of layers) {
      if (!layer) continue;
      for (const name of Object.keys(layer)) {
        for (const longhand of longhands[name] ?? []) if (!(longhand in layer)) delete result[longhand];
        for (const other of alias[name] ?? []) if (!(other in layer)) delete result[other];
      }
      Object.assign(result, layer);
    }
    return result;
  }
  function admitStyle(tag, style2) {
    const accepted = kindOf(tag).style;
    for (const name of Object.keys(style2)) {
      if (!has(style, name)) throw Error(`UI_STYLE: unknown style property ${name}`);
      if (!accepted.includes(name)) throw Error(`UI_STYLE: ${name} is not supported on <${tag}>${appliesTo(name)}`);
    }
  }
  function invalid(name, value, takes) {
    let got;
    if (typeof value === "number") got = String(value);
    else {
      try {
        got = (JSON.stringify(value) ?? typeof value).slice(0, 40);
      } catch {
        got = typeof value;
      }
    }
    throw Error(`UI_STYLE_VALUE: ${name} takes ${takes}; got ${got}`);
  }
  var isStep = (value) => Array.isArray(value) && value.length === 3 && Number.isInteger(value[0]) && scales[value[0]] !== void 0 && finite(value[1]) && finite(value[2]);
  var fold = (value) => isStep(value) && finite(scaleDefaults[value[0]]) ? value[1] * scaleDefaults[value[0]] + value[2] : value;
  var times = (value, factor) => isStep(value) ? [value[0], value[1] * factor, value[2] * factor] : value * factor;
  function length(name, value, { signed = false, max = limits.styleNumber, clamp = max === limits.styleNumber } = {}, warn) {
    if (isStep(value)) {
      if (!signed && value[1] < 0 && value[2] <= 0) invalid(name, value, "a step that resolves to 0 or more");
      return value;
    }
    const min = signed ? -max : 0;
    if (!finite(value)) invalid(name, value, `a number from ${min} to ${max}`);
    if (value > max || value < min) {
      if (!clamp) invalid(name, value, `a number from ${min} to ${max}`);
      const bound = value < min ? min : max;
      warn?.(name, `UI_STYLE_VALUE: ${name} takes a number from ${min} to ${max}; got ${value}; clamped to ${bound}`);
      return bound;
    }
    return value;
  }
  function dimension(name, value, env, out, key, warn) {
    const ch = typeof value === "string" && value.match(/^(\d+(?:\.\d+)?)ch$/);
    if (ch) {
      if (!["width", "minWidth", "maxWidth"].includes(key)) invalid(name, value, "points, a percentage or vw/vh; ch sizes widths only");
      out[`${key}Ch`] = length(name, Number(ch[1]), { max: 4096 }, warn);
      return;
    }
    const viewport = typeof value === "string" && value.match(/^(\d+(?:\.\d+)?)(vw|vh)$/);
    if (viewport) {
      const insets = env?.insets ?? {}, keyboard = env?.keyboard?.height ?? 0;
      const extent = viewport[2] === "vw" ? env?.viewport?.width - (insets.left ?? 0) - (insets.right ?? 0) : env?.viewport?.height - (insets.top ?? 0) - Math.max(insets.bottom ?? 0, keyboard);
      if (!finite(extent)) invalid(name, value, "vw/vh only once the page has a viewport");
      out[key] = length(name, extent * Number(viewport[1]) / 100, void 0, warn);
    } else if (percent(value) !== void 0) out[`${key}Percent`] = length(name, percent(value), void 0, warn);
    else if (value === "auto" && key === "height") out.heightAuto = 1;
    else if (value === "auto" && key === "width") {
    } else if (["fit-content", "max-content", "min-content"].includes(value) && ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight"].includes(key)) out[`${key}Keyword`] = value;
    else out[key] = length(name, value, void 0, warn);
  }
  var alignment = { "flex-start": "start", "flex-end": "end" };
  var justification = { "flex-start": "start", "flex-end": "end", "space-between": "between", "space-around": "around", "space-evenly": "evenly" };
  var safeBits = { justify: 1, align: 2, alignSelf: 4, alignContent: 8, justifyItems: 16, justifySelf: 32 };
  function aligned(name, key, value, names3, safe) {
    if (typeof value === "string" && value.startsWith("safe ")) {
      value = value.slice(5);
      if (!["center", "end", "flex-end"].includes(value)) invalid(name, `safe ${value}`, "safe center, safe end or safe flex-end");
      safe.bits |= safeBits[key];
    }
    return option(name, key, names3[value] ?? value);
  }
  function option(name, key, value) {
    if (!fieldOptions[key]?.includes(value)) invalid(name, value, `one of ${fieldOptions[key]?.join(", ")}`);
    return value;
  }
  var generics = { "sans-serif": "sans", monospace: "mono", serif: "serif" };
  function family(name, value) {
    if (typeof value !== "string") invalid(name, value, "a family name");
    const parts2 = value.split(",").map((part) => part.trim().replace(/^['"]|['"]$/g, "")).filter(Boolean);
    if (parts2.length === 1 && generics[parts2[0]]) return generics[parts2[0]];
    const [face, fallback = "sans-serif"] = parts2;
    if (parts2.length > 2 || !generics[fallback] || Object.values(generics).includes(face)) invalid(name, value, "sans-serif, serif, monospace, or one @font-face family before one of them");
    if (!declaredFamily(face)) throw Error(`UI_FONT: ${face} is not declared with @font-face in the page's theme`);
    return `${face},${generics[fallback]}`;
  }
  function backdrop(name, value) {
    if (value === "none") return [0, 1];
    const match = typeof value === "string" && value.trim().match(/^(?:blur\((\d+(?:\.\d+)?)px\))?\s*(?:saturate\((\d+(?:\.\d+)?)(%?)\))?$/);
    if (!match || match[1] === void 0 && match[2] === void 0) invalid(name, value, "none, blur(<px>) and/or saturate(<n>)");
    const blur = match[1] === void 0 ? 0 : Number(match[1]);
    const saturation = match[2] === void 0 ? 1 : Number(match[2]) / (match[3] ? 100 : 1);
    if (blur > 64 || saturation > 16) invalid(name, value, "a blur up to 64px and a saturation up to 16");
    return [blur, saturation];
  }
  function tracks(name, value) {
    if (typeof value !== "string" || value.length > 1024 || !/^[a-z0-9\s().,%-]+$/.test(value)) invalid(name, value, "a track list such as repeat(3, 1fr) or 120px 1fr");
    return value.trim().replace(/\s+/g, " ");
  }
  function gridEdge(name, value) {
    const text = typeof value === "number" ? String(value) : typeof value === "string" ? value.trim().replace(/\s+/g, " ") : "";
    if (!/^(?:auto|-?[1-9]\d?|span [1-9]\d?)$/.test(text)) invalid(name, value, "auto, a line from -99 to 99 (not 0), or span 1 to 99");
    return text;
  }
  function gridLine(name, value, lines) {
    const axis = name.startsWith("gridColumn") ? "Column" : "Row";
    const line = lines[axis] ?? (lines[axis] = {});
    if (name.endsWith("Start")) line.start = gridEdge(name, value);
    else if (name.endsWith("End")) line.end = gridEdge(name, value);
    else {
      const parts2 = String(value).split("/");
      if (parts2.length > 2) invalid(name, value, "start / end");
      line.shorthand = [gridEdge(name, parts2[0]), parts2[1] === void 0 ? "auto" : gridEdge(name, parts2[1])];
    }
  }
  var translateLength = (value) => finite(value) ? Math.abs(value) <= limits.styleNumber : typeof value === "string" && /^-?(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) && Math.abs(parseFloat(value)) <= 1e3;
  function transformPart(name, value, out) {
    const pair = (check) => {
      const parts2 = Array.isArray(value) ? value : [value, name === "scale" ? value : 0];
      if (parts2.length !== 2 || !parts2.every(check)) invalid(name, value, name === "scale" ? "a factor or [x, y], each up to 1000" : `points or a percentage, or [x, y] of them, up to ${limits.styleNumber} points or 1000%`);
      return [...parts2];
    };
    if (name === "translate") out.translate = pair(translateLength);
    else if (name === "scale") out.scale = pair((v) => finite(v) && Math.abs(v) <= 1e3);
    else if (name === "rotate") {
      if (!finite(value) || Math.abs(value) > 36e3) invalid(name, value, "degrees from -36000 to 36000");
      out.rotate = value;
    } else {
      if (!Array.isArray(value) || value.length !== 2 || !value.every((v) => finite(v) && Math.abs(v) <= 16)) invalid(name, value, "[x, y] as box fractions from -16 to 16");
      out.origin = [...value];
    }
  }
  var numericGroups = [["ordinal"], ["slashed-zero"], ["lining-nums", "oldstyle-nums"], ["proportional-nums", "tabular-nums"], ["diagonal-fractions", "stacked-fractions"]];
  function numericVariant(name, value) {
    if (value === "normal") return value;
    const tokens = typeof value === "string" ? value.trim().split(/\s+/) : [];
    const picked = numericGroups.map((group) => tokens.filter((token) => group.includes(token)));
    if (!tokens.length || picked.some((group) => group.length > 1) || picked.flat().length !== tokens.length) invalid(name, value, `normal, or at most one of each of ${numericGroups.map((group) => group.join("/")).join(", ")}`);
    return picked.flat().join(" ");
  }
  function maskValue(value) {
    if (value === "none") return void 0;
    const fail = () => invalid("maskImage", value, "'none' or 1..4 layers of { to: 'top'|'bottom'|'left'|'right', stops: 2..16 { alpha: 0..1, offset | px } }");
    const position = (stop) => stop.offset !== void 0 !== (stop.px !== void 0) && (stop.offset === void 0 || finite(stop.offset) && Math.abs(stop.offset) <= 16) && (stop.px === void 0 || finite(stop.px) && Math.abs(stop.px) <= limits.styleNumber);
    if (!Array.isArray(value) || !value.length || value.length > 4 || value.some((layer) => !layer || !["top", "bottom", "left", "right"].includes(layer.to) || Object.keys(layer).some((key) => !["to", "stops"].includes(key)) || !Array.isArray(layer.stops) || layer.stops.length < 2 || layer.stops.length > 16 || layer.stops.some((stop) => !stop || Object.keys(stop).some((key) => !["alpha", "offset", "px"].includes(key)) || !finite(stop.alpha) || stop.alpha < 0 || stop.alpha > 1 || !position(stop)))) fail();
    return JSON.stringify(value.map(({ to, stops }) => ({ to, stops: stops.map(({ alpha: alpha2, offset, px }) => offset === void 0 ? { px, alpha: alpha2 } : { offset, alpha: alpha2 }) })));
  }
  function transitionValue(value) {
    if (value === "none") return void 0;
    const fail = () => invalid("transition", value, `'none' or { properties: 1..${options["transition-property"].length} of ${options["transition-property"].join("|")}, durationMs: 1..${limits.transitionMs} or duration step, delayMs?: 0..${limits.transitionMs}, easing?: 'linear'|'step-end'|[x1,y1,x2,y2]|{ spring: 0..0.9 }|ease step }`);
    if (!value || typeof value !== "object" || Object.keys(value).some((key) => !["properties", "durationMs", "delayMs", "easing"].includes(key))) fail();
    const { properties, delayMs = 0 } = value;
    let { durationMs, easing = [0.4, 0, 0.2, 1] } = value;
    const step = (ref, family2) => isStep(ref) && scales[ref[0]].family === family2 && ref[1] === 1 && ref[2] === 0;
    const durationStep = step(durationMs, "duration"), easingStep = step(easing, "ease");
    if (durationStep) durationMs = 1;
    if (easingStep) easing = "linear";
    if (!Array.isArray(properties) || !properties.length || properties.length > options["transition-property"].length || new Set(properties).size !== properties.length || properties.some((name) => !options["transition-property"].includes(name)) || !finite(durationMs) || durationMs <= 0 || durationMs > limits.transitionMs || !finite(delayMs) || delayMs < 0 || delayMs > limits.transitionMs || !(easing === "linear" || easing === "step-end" || Array.isArray(easing) && easing.length === 4 && easing.every(finite) || easing && typeof easing === "object" && Object.keys(easing).join() === "spring" && finite(easing.spring) && easing.spring >= 0 && easing.spring <= 0.9)) fail();
    return JSON.stringify({ properties, durationMs: durationStep ? value.durationMs : durationMs, delayMs, easing: easingStep ? value.easing : easing });
  }
  function animation(value) {
    if (value === null) return void 0;
    if (!value || typeof value !== "object" || Array.isArray(value)) throw Error("UI_ANIMATION: expected definition");
    const encoded = JSON.stringify(value, (_, item) => {
      if (typeof item === "number" && !Number.isFinite(item) || ["undefined", "function", "symbol", "bigint"].includes(typeof item))
        throw Error("UI_ANIMATION: invalid payload value");
      return item;
    });
    if (utf8Bytes(encoded, 32768) > 32768) throw Error("UI_ANIMATION: wire budget exceeded");
    return JSON.parse(encoded);
  }
  function objectPosition(value) {
    const parts2 = typeof value === "string" ? value.trim().split(/\s+/) : [];
    const horizontal = { left: 0, right: 1 }, vertical = { top: 0, bottom: 1 };
    const fraction = (part) => {
      if (part === "center") return 0.5;
      const match = /^(-?(?:\d*\.)?\d+)%$/.exec(part);
      if (!match || Math.abs(Number(match[1])) > 1600) invalid("objectPosition", part, "center, left/right/top/bottom or a percentage up to 1600%");
      return Number(match[1]) / 100;
    };
    if (parts2.length === 1) {
      const [part] = parts2;
      return part in vertical ? [0.5, vertical[part]] : [part in horizontal ? horizontal[part] : fraction(part), 0.5];
    }
    if (parts2.length !== 2) invalid("objectPosition", value, "one or two positions");
    let [x, y] = parts2;
    if (x in vertical || y in horizontal) [x, y] = [y, x];
    if (x in vertical || y in horizontal) invalid("objectPosition", value, "one horizontal and one vertical position");
    return [x in horizontal ? horizontal[x] : fraction(x), y in vertical ? vertical[y] : fraction(y)];
  }
  function lowerStyle(tag, style2, env, inherited2 = { fontSize: 14 }, warn) {
    admitStyle(tag, style2);
    const out = {};
    if (style2.fontSize !== void 0 && !isStep(style2.fontSize) && finite(style2.fontSize) && style2.fontSize <= 0) invalid("fontSize", style2.fontSize, "a number above 0");
    const fontSize = style2.fontSize === void 0 ? inherited2.fontSize : length("fontSize", style2.fontSize, void 0, warn);
    let lineHeight = inherited2.lineHeight;
    const transform = {}, lines = {}, safe = { bits: 0 };
    const names3 = Object.keys(style2).sort((a, b) => Number(shorthand(b)) - Number(shorthand(a)));
    for (const name of names3) {
      const value = style2[name], entry = style[name], key = entry.wire;
      switch (entry.lower) {
        case "dimension":
          dimension(name, value, env, out, key, warn);
          break;
        case "margin": {
          const lowered2 = value === "auto" ? value : length(name, value, { signed: true }, warn);
          for (const side of [key].flat()) out[side] = lowered2;
          break;
        }
        case "inset": {
          const pct = percent(value);
          let lowered2 = value;
          if (value !== "anchor(center)") {
            const clamped = length(name, pct !== void 0 ? pct : value, { signed: true }, warn);
            lowered2 = pct !== void 0 ? `${clamped}%` : clamped;
          }
          for (const side of [key].flat()) out[side] = lowered2;
          break;
        }
        case "length": {
          const lowered2 = length(name, value, void 0, warn);
          for (const side of [key].flat()) out[side] = lowered2;
          break;
        }
        case "z-index":
          if (!Number.isInteger(value) || Math.abs(value) > 1e4) invalid(name, value, "a whole number from -10000 to 10000");
          out[key] = value;
          break;
        case "grid-template":
          if (Number.isInteger(value) && value >= 1 && value <= limits.gridTracks) out[key] = `repeat(${value}, minmax(0, 1fr))`;
          else out[key] = tracks(name, value);
          break;
        case "grid-auto":
          out[key] = tracks(name, value);
          break;
        case "grid-flow":
          out[key] = option(name, key, typeof value === "string" ? value.trim().replace(/\s+/, "-") : value);
          break;
        case "grid-line":
          gridLine(name, value, lines);
          break;
        case "order":
          if (!Number.isInteger(value) || Math.abs(value) > 1e4) invalid(name, value, "a whole number from -10000 to 10000");
          out[key] = value;
          break;
        case "align-content":
          out[key] = aligned(name, key, value, justification, safe);
          break;
        case "line-clamp":
          if (!Number.isInteger(value) || value < 1 || value > 1024) invalid(name, value, "a whole number of lines from 1 to 1024");
          out[key] = value;
          break;
        case "number":
          out[key] = length(name, value, void 0, warn);
          break;
        case "opacity":
          if (!finite(value) || value < 0 || value > 1) invalid(name, value, "a number from 0 to 1");
          out[key] = value;
          break;
        case "font-size":
          out[key] = fontSize;
          break;
        case "font-weight":
          if (!finite(value) || value < 1 || value > 1e3) invalid(name, value, "a number from 1 to 1000");
          out[key] = value;
          break;
        case "color": {
          if (typeof value !== "string" || !value) invalid(name, value, "a colour");
          const problem = wireColorProblem(value, name);
          if (problem) throw Error(problem);
          out[key] = value === "transparent" ? "#00000000" : value;
          break;
        }
        case "gradient":
          out[key] = normalizeGradient(value);
          break;
        case "shadow":
          out[key] = normalizeShadows(value);
          break;
        case "ring":
          out[key] = normalizeRing(value);
          break;
        case "backdrop":
          [out[key[0]], out[key[1]]] = backdrop(name, value);
          break;
        case "animation": {
          const lowered2 = animation(value);
          if (lowered2 !== void 0) out[key] = lowered2;
          break;
        }
        case "transform":
          transformPart(name, value, transform);
          break;
        case "transition": {
          const lowered2 = transitionValue(value);
          if (lowered2 !== void 0) out[key] = lowered2;
          break;
        }
        case "mask": {
          const lowered2 = maskValue(value);
          if (lowered2 !== void 0) out[key] = lowered2;
          break;
        }
        case "filter": {
          if (value === "none") {
            out[key] = 0;
            break;
          }
          const blur = typeof value === "string" && value.trim().match(/^blur\((\d+(?:\.\d+)?)px\)$/);
          if (!blur || Number(blur[1]) > 64) invalid(name, value, "none or blur(<px>) up to 64px");
          out[key] = Number(blur[1]);
          break;
        }
        case "aspect-ratio":
          if (value === "auto") break;
          if (!finite(value) || value <= 0 || value > 1e4) invalid(name, value, "auto or a ratio above 0 up to 10000");
          out[key] = value;
          break;
        case "font-variant-numeric":
          out[key] = numericVariant(name, value);
          break;
        case "basis": {
          if (value === "auto") {
            out[key] = value;
            break;
          }
          const pct = percent(value);
          const clamped = length(name, pct !== void 0 ? pct : value, void 0, warn);
          out[key] = pct !== void 0 ? `${clamped}%` : clamped;
          break;
        }
        case "snap-type": {
          const axis = String(value).trim().split(/\s+/)[0];
          out[key] = option(name, key, axis);
          break;
        }
        case "option":
          if (name === "userSelect" && ["input", "textarea"].includes(tag)) throw Error("UI_STYLE: editors select their text already; userSelect is for static text");
          out[key] = option(name, key, value);
          break;
        case "alignment":
          out[key] = aligned(name, key, value, alignment, safe);
          break;
        case "justification":
          out[key] = aligned(name, key, value, justification, safe);
          break;
        case "family":
          out[key] = family(name, value);
          break;
        case "line-height":
          if (isStep(value)) lineHeight = value;
          else if (typeof value === "string" && /^\d+(?:\.\d+)?px$/.test(value)) lineHeight = length(name, Number(value.slice(0, -2)), void 0, warn);
          else if (finite(value) && value > 0 && value <= limits.lineHeightMultiplier) lineHeight = { multiplier: value };
          else if (finite(value) && lineHeightProblem(value)) {
            warn?.(name, `${lineHeightProblem(value)}; clamped to ${limits.lineHeightMultiplier}`);
            lineHeight = { multiplier: limits.lineHeightMultiplier };
          } else invalid(name, value, `a multiplier above 0 up to ${limits.lineHeightMultiplier}, or <n>px`);
          break;
        case "letter-spacing": {
          const em = typeof value === "string" && value.match(/^(-?\d+(?:\.\d+)?)em$/);
          const pixels2 = em ? times(fontSize, Number(em[1])) : value;
          out[key] = length(name, pixels2, { signed: true, max: 1024 });
          break;
        }
        case "scroll-axis":
          if (value !== "auto" || style2.overflowX !== void 0 && style2.overflowY !== void 0) invalid(name, value, "auto, on one axis only");
          out[key] = name === "overflowX" ? "horizontal" : "vertical";
          break;
        case "fit":
          if (!(tag === "vector" ? ["contain", "fill"] : ["cover", "contain", "fill"]).includes(value)) invalid(name, value, tag === "vector" ? "contain or fill" : "cover, contain or fill");
          out[key] = value;
          break;
        case "object-position":
          out[key] = JSON.stringify(objectPosition(value));
          break;
        case "part":
          out[key] = partRecord(name, value, env);
          break;
        default:
          throw Error(`UI_STYLE: ${name} has no native lowering`);
      }
    }
    if (Object.keys(transform).length) out.transform = JSON.stringify(transform);
    if (safe.bits) out.alignSafe = safe.bits;
    for (const axis of ["Column", "Row"]) {
      const line = lines[axis];
      if (!line) continue;
      const start = line.start ?? line.shorthand?.[0] ?? "auto", end = line.end ?? line.shorthand?.[1] ?? "auto";
      out[`grid${axis}`] = end === "auto" ? start : `${start} / ${end}`;
    }
    if (style2.lineHeight !== void 0 || lineHeight && typeof lineHeight === "object" && !isStep(lineHeight)) {
      let pixels2 = lineHeight?.multiplier !== void 0 ? times(fontSize, lineHeight.multiplier) : lineHeight;
      if (!isStep(pixels2)) {
        if (!(pixels2 > 0)) invalid("lineHeight", pixels2, `a height above 0 up to ${limits.styleNumber} points`);
        if (pixels2 > limits.styleNumber) {
          warn?.("lineHeight", `UI_STYLE_VALUE: lineHeight takes a height above 0 up to ${limits.styleNumber} points; got ${pixels2}; clamped to ${limits.styleNumber}`);
          pixels2 = limits.styleNumber;
        }
      }
      out.lineHeight = pixels2;
    }
    for (const key in out) {
      const value = out[key];
      if (!isStep(value)) continue;
      const { name, family: family2 } = scales[value[0]];
      if (!scaledFields[family2].includes(key)) throw Error(`UI_SCALE: ${key} does not take the theme step ${name}; it takes ${Object.entries(scaledFields).filter(([, keys2]) => keys2.includes(key)).map(([family3]) => family3).join(", ") || "no step"}`);
    }
    return { fields: out, typography: { fontSize, lineHeight } };
  }
  function partRecord(name, value, env) {
    if (!value || typeof value !== "object" || Array.isArray(value)) invalid(name, value, "a style object");
    value = Object.fromEntries(Object.entries(value).map(([key, item]) => [key, key === "width" || key === "height" ? fold(item) : item]));
    for (const key of ["width", "height"]) if (value[key] !== void 0 && !(finite(value[key]) && value[key] >= 0 && value[key] <= 1024)) invalid(`${name}.${key}`, value[key], "a number from 0 to 1024");
    const pairs = [];
    for (const [key, lowered2] of Object.entries(lowerStyle("view", value, env).fields))
      pairs.push(field[key], (key === "boxShadow" || key === "ring") && lowered2 !== "none" ? JSON.stringify(lowered2) : lowered2);
    return JSON.stringify(pairs);
  }
  function paintOnly(state, tag, style2) {
    admitStyle(tag, style2);
    for (const name of Object.keys(style2))
      if (!style[name].paint) throw Error(`UI_STYLE: ${state}: supports paint properties only, not ${name}`);
  }

  // src/style/classes.js
  var byName = /* @__PURE__ */ new Map();
  var cache = /* @__PURE__ */ new Map();
  var states = ["focus", "focusVisible", "active", "hover", "disabled", "starting", "exit", "hasChecked", "hasDisabled", "focusWithin", "groupActive", "sideTop", "sideBottom", "sideStart", "sideEnd"];
  var parts = ["track", "range", "secondary", "thumb"];
  var variantNames = { focusVisible: "focus-visible", hasChecked: "has-checked", hasDisabled: "has-disabled", focusWithin: "focus-within", groupActive: "group-active", sideTop: "side-top", sideBottom: "side-bottom", sideStart: "side-start", sideEnd: "side-end", children: "space-*/divide-*", every: "*" };
  var geometryKeys = { linear: ["direction", "angle"], radial: ["shape", "size", "at"], conic: ["from", "at"] };
  function copy(value) {
    if (Array.isArray(value)) return value.map(copy);
    if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, copy(item)]));
    return value;
  }
  var classGeneration = 0;
  var themed = false;
  var classesReadTheme = () => themed;
  var sized = false;
  var classesReadSizeClass = () => sized;
  var NO_FIELDS = Object.freeze([]);
  var VIEWPORT_FIELDS = Object.freeze(["viewport", "insets", "keyboard"]);
  var viewportUnit = /\d(?:vw|vh)\b/;
  function usesViewportUnits(style2, depth = 0) {
    if (!style2 || typeof style2 !== "object" || depth > 4) return false;
    for (const key in style2) {
      const value = style2[key];
      if (typeof value === "string" ? viewportUnit.test(value) : usesViewportUnits(value, depth + 1)) return true;
    }
    return false;
  }
  var fieldsByClass = /* @__PURE__ */ new Map();
  function classEnvFields(className) {
    let fields3 = fieldsByClass.get(className);
    if (fields3) return fields3;
    const out = /* @__PURE__ */ new Set();
    for (const name of className.split(/\s+/)) for (const rule of byName.get(name) ?? []) {
      const when = rule.when;
      if (when?.appearance !== void 0) out.add("appearance");
      if (when?.sizeClass !== void 0) out.add("sizeClass");
      if (when?.reducedMotion !== void 0) out.add("reducedMotion");
      if (when?.minWidth !== void 0 || when?.maxWidth !== void 0) out.add("viewport");
      if (usesViewportUnits(rule.style)) for (const field2 of VIEWPORT_FIELDS) out.add(field2);
    }
    fields3 = out.size ? Object.freeze([...out]) : NO_FIELDS;
    if (fieldsByClass.size >= 4096) fieldsByClass.delete(fieldsByClass.keys().next().value);
    fieldsByClass.set(className, fields3);
    return fields3;
  }
  function configureClasses(compiled) {
    classGeneration++;
    byName = /* @__PURE__ */ new Map();
    cache.clear();
    sizedNames.clear();
    fieldsByClass.clear();
    themed = compiled.some((rule) => rule.when?.appearance !== void 0);
    sized = compiled.some((rule) => rule.when?.sizeClass !== void 0);
    compiled.forEach((rule, order) => {
      if (!byName.has(rule.name)) byName.set(rule.name, []);
      byName.get(rule.name).push({ ...copy(rule), order });
    });
  }
  function applies(rule, env, checked2) {
    const { appearance, minWidth, maxWidth, direction, sizeClass } = rule.when ?? {};
    if (appearance && (env.appearance ?? "light") !== appearance) return false;
    if (direction && (env.direction ?? "ltr") !== direction) return false;
    if (sizeClass && (env.sizeClass ?? "compact") !== sizeClass) return false;
    if (rule.when?.structure && !(env.structure ?? []).includes(rule.when.structure)) return false;
    if (rule.when?.checked && !checked2) return false;
    if (rule.when?.indeterminate && env.indeterminate !== true) return false;
    if (rule.when?.data) {
      const [name, value] = rule.when.data, own2 = env.data?.[name];
      if (own2 === void 0 || value !== void 0 && own2 !== value) return false;
    }
    if (rule.when?.reducedMotion !== void 0 && env.reducedMotion === true !== rule.when.reducedMotion) return false;
    if (minWidth !== void 0 && !(env.viewport?.width >= minWidth)) return false;
    if (maxWidth !== void 0 && !(env.viewport?.width < maxWidth)) return false;
    return true;
  }
  var pixels = (value) => `${value}px`;
  function cascade(rules, meta) {
    const style2 = {}, gradient = {}, shadow = {}, backdrop2 = {}, transform = {}, motion = {}, mask = {};
    let numeric = {};
    for (const rule of rules) {
      if (rule.style) resetLonghands(style2, rule.style);
      Object.assign(style2, copy(rule.style));
      if (meta && rule.style?.lineHeight !== void 0) meta.pairedLeading = rule.pairedLeading === true;
      Object.assign(motion, rule.motion);
      Object.assign(transform, rule.transform);
      if (rule.numeric?.normal || rule.numeric?.fixed) numeric = { ...rule.numeric };
      else if (rule.numeric) numeric = { ...numeric, ...rule.numeric, normal: false, fixed: void 0 };
      Object.assign(gradient, rule.gradient);
      Object.assign(shadow, rule.shadow);
      Object.assign(backdrop2, rule.backdrop);
      Object.assign(mask, rule.mask);
    }
    const filter = backdrop2.mode === "fixed" ? backdrop2.fixed : backdrop2.mode === "variables" ? backdrop2 : void 0;
    if (filter && (filter.blur != null || filter.saturation != null))
      style2.backdropFilter = `blur(${pixels(filter.blur ?? 0)}) saturate(${filter.saturation ?? 1})`;
    const properties = [.../* @__PURE__ */ new Set([...motion.properties ?? [], ...motion.layout ? ["layout"] : [], ...motion.size ? ["size"] : []])];
    if (properties.length) style2.transition = {
      properties,
      durationMs: motion.duration ?? motion.durationDefault ?? 150,
      delayMs: motion.delay ?? 0,
      easing: copy(motion.easing ?? motion.easingDefault ?? [0.4, 0, 0.2, 1])
    };
    else if (motion.properties) style2.transition = "none";
    else if (meta && (motion.duration !== void 0 || motion.easing !== void 0))
      meta.release = { durationMs: motion.duration ?? 300, easing: copy(motion.easing ?? [0.4, 0, 0.2, 1]) };
    if (mask.none) style2.maskImage = "none";
    else if (mask.fixed) style2.maskImage = copy(mask.fixed);
    else if (mask.edges) {
      const layers = ["top", "bottom", "left", "right"].filter((side) => mask[side]).map((side) => ({ to: side, stops: [
        { alpha: mask[`${side}-from-color`] ?? 1, ...mask[`${side}-from-position`] ?? { offset: 0 } },
        { alpha: mask[`${side}-to-color`] ?? 0, ...mask[`${side}-to-position`] ?? { offset: 1 } }
      ] }));
      if (layers.length) style2.maskImage = layers;
    }
    if (numeric.normal) style2.fontVariantNumeric = "normal";
    else if (numeric.fixed) style2.fontVariantNumeric = numeric.fixed;
    else if (numeric.enabled) {
      const keywords = [numeric.ordinal, numeric.slashedZero, numeric.figure, numeric.spacing, numeric.fraction].filter(Boolean);
      style2.fontVariantNumeric = keywords.length ? keywords.join(" ") : "normal";
    }
    if (transform.translate) style2.translate = [transform.translateX ?? 0, transform.translateY ?? 0];
    if (transform.scale) style2.scale = [transform.scaleX ?? 1, transform.scaleY ?? 1];
    if (gradient.enabled !== void 0) {
      style2.backgroundImage = "none";
      if (gradient.enabled && gradient.fixed) style2.backgroundImage = copy(gradient.fixed);
      else if (gradient.enabled && gradient.source && (gradient.source !== "via" || gradient.via)) {
        const stops = [{ offset: gradient.fromOffset ?? 0, color: gradient.fromColor ?? "#00000000" }];
        if (gradient.via) stops.push({ offset: gradient.viaOffset ?? 0.5, color: gradient.viaColor ?? "#00000000" });
        stops.push({ offset: gradient.toOffset ?? 1, color: gradient.toColor ?? "#00000000" });
        for (let i = 1; i < stops.length; i++) stops[i].offset = Math.max(stops[i - 1].offset, stops[i].offset);
        const kind = gradient.kind ?? "linear";
        const { colorSpace = "srgb", ...geometry2 } = gradient.position ?? {};
        if (Object.keys(geometry2).some((key) => !geometryKeys[kind].includes(key)))
          throw Error(`UI_GRADIENT: ${kind} gradients do not take ${Object.keys(geometry2).join(", ")}`);
        if (kind === "linear" && geometry2.angle === void 0 && geometry2.direction === void 0) geometry2.direction = "bottom";
        style2.backgroundImage = { ...kind === "linear" ? {} : { kind }, ...geometry2, colorSpace, stops };
      }
    }
    if (shadow.enabled) {
      style2.boxShadow = (shadow.composite === false ? shadow.fixedLayers ?? [] : shadow.layers ?? []).map(({ useColor, ...layer }) => ({
        ...layer,
        color: useColor && shadow.color !== void 0 ? shadow.color : layer.color
      }));
      if (style2.boxShadow.length > 8) throw Error("UI_STYLE: too many box shadows");
      const ring = {};
      if (shadow.composite !== false && shadow.ringWidth > 0) {
        ring.outer = { width: shadow.ringWidth, offset: shadow.ringOffset ?? 0, color: shadow.ringColor ?? "currentColor" };
        if (shadow.ringOffsetColor !== void 0) ring.outer.offsetColor = shadow.ringOffsetColor;
      }
      if (shadow.composite !== false && shadow.insetRingWidth > 0)
        ring.inner = { width: shadow.insetRingWidth, color: shadow.insetRingColor ?? "currentColor" };
      style2.ring = ring.outer || ring.inner ? ring : "none";
    }
    return style2;
  }
  var sizedNames = /* @__PURE__ */ new Map();
  function classesUseSizeClass(className) {
    if (!sized || typeof className !== "string") return false;
    let uses = sizedNames.get(className);
    if (uses === void 0) {
      uses = className.split(/\s+/).some((name) => byName.get(name)?.some((rule) => rule.when?.sizeClass !== void 0) ?? false);
      if (sizedNames.size > 4096) sizedNames.clear();
      sizedNames.set(className, uses);
    }
    return uses;
  }
  function partitionClasses(className, pick2) {
    const picked = [], rest = [];
    for (const name of typeof className === "string" ? className.split(/\s+/).filter(Boolean) : [])
      (byName.get(name)?.some(pick2) ? picked : rest).push(name);
    return [picked.join(" "), rest.join(" ")];
  }
  function changed(full, base) {
    const delta = {};
    for (const [key, value] of Object.entries(full)) if (JSON.stringify(value) !== JSON.stringify(base[key])) delta[key] = value;
    return delta;
  }
  function resolveClasses(className, env = {}) {
    if (className === void 0) return void 0;
    if (typeof className !== "string") throw Error("UI_CLASS: className must be a string");
    if (className.length > 16384) throw Error("UI_CLASS: className exceeds 16384 characters");
    const environment = `${env.appearance ?? "light"}|${env.viewport?.width}|${env.viewport?.height}|${env.sizeClass ?? ""}|${env.direction ?? "ltr"}|${(env.structure ?? []).join(",")}|${env.reducedMotion === true}|${env.data ? JSON.stringify(env.data) : ""}|${env.indeterminate === true}`;
    const key = `${environment}\0${className}`;
    if (cache.has(key)) return copy(cache.get(key));
    const result = cascadeFor(className, env, false);
    if (result.checkable) {
      const on = cascadeFor(className, env, true);
      for (const state of [...states.filter((state2) => state2 !== "disabled"), "children", "every", "backdrop"])
        if (JSON.stringify(on[state]) !== JSON.stringify(result[state]))
          throw Error(`UI_CLASS: ${className}: checked: and ${variantNames[state] ?? state}: style the same part; a state of a checked part is not supported`);
      result.checked = { base: changed(on.base, result.base), disabled: changed(on.disabled ?? on.base, result.disabled ?? result.base) };
    }
    delete result.checkable;
    if (cache.size >= 1024) cache.delete(cache.keys().next().value);
    cache.set(key, copy(result));
    return result;
  }
  function cascadeFor(className, env, checked2) {
    const matched = [];
    let checkable = false;
    const tokens = className.split(/\s+/).filter(Boolean);
    const position = new Map(tokens.map((name, index) => [name, index]));
    for (const name of position.keys()) {
      const rules = byName.get(name);
      if (!rules) throw Error(`UI_CLASS: ${name} was not compiled; use complete utility names in source`);
      for (const rule of rules) {
        if (rule.error) throw Error(`UI_CLASS: ${name}: ${rule.error.message}`);
        if (rule.when?.checked && applies(rule, env, true)) checkable = true;
        if (applies(rule, env, checked2)) matched.push({ ...rule, position: position.get(name) });
      }
    }
    const specific = (rule) => Number(!!rule.when?.state) + Number(!!rule.when?.checked);
    matched.sort((a, b) => specific(a) - specific(b) || a.order - b.order);
    const slots = /* @__PURE__ */ new Map();
    matched.forEach((rule, index) => {
      const variant = JSON.stringify(rule.when ?? {});
      if (!slots.has(variant)) slots.set(variant, []);
      slots.get(variant).push(index);
    });
    const ordered = [...matched];
    for (const indices of slots.values()) {
      const group = indices.map((index) => matched[index]).sort((a, b) => a.position - b.position || a.order - b.order);
      indices.forEach((index, at) => {
        ordered[index] = group[at];
      });
    }
    matched.splice(0, matched.length, ...ordered);
    const own2 = matched.filter((rule) => !rule.when?.children && !rule.when?.every && !rule.when?.part);
    const plain3 = own2.filter((rule) => !rule.when?.state);
    const meta = {};
    const base = cascade(plain3, meta);
    const result = { base, ...meta.pairedLeading ? { pairedLeading: true } : {}, ...meta.release ? { release: meta.release } : {} };
    for (const state of states) {
      const extra = own2.filter((rule) => rule.when?.state === state);
      if (!extra.length) continue;
      const full = cascade([...plain3, ...extra]);
      result[state] = state === "disabled" ? full : changed(full, base);
    }
    for (const part of parts) {
      const rules = matched.filter((rule) => rule.when?.part === part);
      if (!rules.length) continue;
      const key = `${part}Part`, plainPart = rules.filter((rule) => !rule.when.state);
      const rest = cascade(plainPart);
      if (Object.keys(rest).length) base[key] = rest;
      for (const state of ["focusVisible", "active", "hover", "disabled"]) {
        const extra = rules.filter((rule) => rule.when.state === state);
        if (!extra.length) continue;
        const full = cascade([...plainPart, ...extra]);
        if (state === "disabled") (result.disabled ?? (result.disabled = { ...base }))[key] = full;
        else if (JSON.stringify(full) !== JSON.stringify(rest)) (result[state] ?? (result[state] = {}))[key] = full;
      }
      if (result.disabled && !result.disabled[key] && base[key]) result.disabled[key] = base[key];
    }
    const backdrop2 = own2.filter((rule) => rule.when?.state === "backdrop");
    if (backdrop2.length) {
      const plainBackdrop = backdrop2.filter((rule) => !rule.when.presence);
      const backdropBase = cascade(plainBackdrop);
      result.backdrop = { base: backdropBase };
      const motion = Object.assign({}, ...plainBackdrop.map((rule) => rule.motion ?? {}));
      const timing = {
        ...motion.duration !== void 0 ? { durationMs: motion.duration } : {},
        ...motion.delay !== void 0 ? { delayMs: motion.delay } : {},
        ...motion.easing !== void 0 ? { easing: copy(motion.easing) } : {}
      };
      if (Object.keys(timing).length) result.backdrop.timing = timing;
      for (const presence of ["starting", "exit"]) {
        const extra = backdrop2.filter((rule) => rule.when.presence === presence);
        if (extra.length) result.backdrop[presence] = changed(cascade([...plainBackdrop, ...extra]), backdropBase);
      }
    }
    const children = matched.filter((rule) => rule.when?.children);
    if (children.length) result.children = cascade(children);
    const every = matched.filter((rule) => rule.when?.every);
    if (every.length) result.every = cascade(every);
    if (checkable) result.checkable = true;
    return result;
  }

  // src/runtime/errors.js
  function located(error, frame) {
    const base = error instanceof Error ? error : Error(String(error));
    if (!base.uiFrames) {
      base.uiFrames = [];
      base.uiMessage = base.message;
      const site = authorSite(base.stack);
      if (site) base.uiFrames.push(site);
    }
    if (base.uiFrames.length < 12) base.uiFrames.push(frame);
    base.message = [base.uiMessage, ...base.uiFrames.map((line) => `  at ${line}`)].join("\n");
    return base;
  }
  function bridgeError(code, message, data, reason) {
    const error = new Error(message ?? code);
    error.name = "LxBridgeError";
    error.code = code;
    if (reason !== void 0) error.reason = reason;
    if (data !== void 0) error.data = data;
    return error;
  }
  function resourceError(error) {
    if (typeof error?.code === "string") return Object.freeze({
      code: error.code,
      message: String(error.message ?? error.code),
      ...typeof error.reason === "string" ? { reason: error.reason } : {},
      ...error.data !== void 0 ? { data: error.data } : {}
    });
    const text = String(error?.uiMessage ?? error?.message ?? error);
    const match = text.match(/^([A-Z][A-Z0-9_]+): ([\s\S]*)$/);
    return Object.freeze(match ? { code: match[1], message: match[2] } : { code: "UI_ERROR", message: text });
  }
  var sourceMap = null;
  var decoded = null;
  function configureSourceMap(map) {
    sourceMap = map;
    decoded = null;
  }
  var base64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  function decode() {
    const lines = [];
    let source3 = 0, line = 0, column = 0;
    for (const text of sourceMap.mappings.split(";")) {
      const segments = [];
      let generated = 0;
      for (const segment of text.split(",")) {
        if (!segment) continue;
        const values = [];
        let value = 0, shift = 0;
        for (const char of segment) {
          const digit = base64.indexOf(char);
          value += (digit & 31) << shift;
          if (digit & 32) shift += 5;
          else {
            values.push(value & 1 ? -(value >>> 1) : value >>> 1);
            value = 0;
            shift = 0;
          }
        }
        generated += values[0];
        if (values.length < 4) continue;
        source3 += values[1];
        line += values[2];
        column += values[3];
        segments.push([generated, source3, line, column]);
      }
      lines.push(segments);
    }
    return lines;
  }
  function authorSite(stack) {
    if (!sourceMap || typeof stack !== "string") return void 0;
    decoded ?? (decoded = decode());
    for (const match of stack.matchAll(/:(\d+):(\d+)\)?\s*$/gm)) {
      const segments = decoded[Number(match[1]) - 1];
      if (!segments?.length) continue;
      const column = Number(match[2]) - 1;
      let best = segments[0];
      for (const segment of segments) if (segment[0] <= column) best = segment;
      const file = sourceMap.sources[best[1]];
      if (file) return `${file}:${best[2] + 1}:${best[3] + 1 + Math.max(0, column - best[0])}`;
    }
    return void 0;
  }
  function sited(error) {
    if (!(error instanceof Error) || error.uiFrames) return error;
    const site = authorSite(error.stack);
    if (!site) return error;
    error.uiFrames = [site];
    error.uiMessage = error.message;
    error.message = `${error.message}
  at ${site}`;
    return error;
  }

  // src/runtime/stream.js
  function createStream(id, cancel, report2, error) {
    let done = false, failure2, finalValue, iterator = false, bytes = 0;
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
          report2(cause);
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
        failure2 = value;
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
    const handle2 = Object.freeze({
      id,
      result,
      on(kind, fn) {
        if (!Object.prototype.hasOwnProperty.call(listeners, kind) || typeof fn !== "function")
          throw Error("UI_STREAM: invalid listener");
        if (!done) {
          if (listeners[kind].size >= 256) throw Error("UI_LIMIT: too many stream listeners");
          listeners[kind].add(fn);
        }
        return handle2;
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
            if (done) return failure2 ? Promise.reject(failure2) : Promise.resolve({ done: true, value: finalValue });
            if (reads.length >= 256) return Promise.reject(error("UI_LIMIT", "Too many pending stream reads"));
            return new Promise((resolve2, reject2) => reads.push({ resolve: resolve2, reject: reject2 }));
          },
          return() {
            queue.length = 0;
            bytes = 0;
            handle2.cancel();
            return Promise.resolve({ done: true, value: void 0 });
          }
        };
      }
    });
    return {
      handle: handle2,
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
  function createChannel(id, send, close, report2, error) {
    let closed = false, failure2, iterator = false, bytes = 0;
    const queue = [], reads = [];
    const listeners = { data: /* @__PURE__ */ new Set(), close: /* @__PURE__ */ new Set(), error: /* @__PURE__ */ new Set() };
    const invoke = (fn, ...values) => {
      try {
        fn(...values);
      } catch (cause) {
        report2(cause);
      }
    };
    const finish = (cause, failed, discard2 = false) => {
      if (closed) return;
      closed = true;
      failure2 = cause;
      if (cause || discard2) {
        queue.length = 0;
        bytes = 0;
      }
      for (const read of reads.splice(0)) {
        if (cause) read.reject(cause);
        else read.resolve({ done: true, value: void 0 });
      }
      if (cause) for (const fn of [...listeners.error]) invoke(fn, cause);
      const event = Object.freeze(failed === void 0 ? {} : { error: failed });
      for (const fn of [...listeners.close]) invoke(fn, event);
      for (const bucket of Object.values(listeners)) bucket.clear();
    };
    const handle2 = Object.freeze({
      id,
      send(payload2) {
        if (closed) throw error("STREAM_CLOSED", "Channel is closed");
        send(payload2);
      },
      close(failed) {
        if (!closed) close(failed);
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
        return handle2;
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
            if (closed) return failure2 ? Promise.reject(failure2) : Promise.resolve({ done: true, value: void 0 });
            if (reads.length >= 256) return Promise.reject(error("UI_LIMIT", "Too many pending channel reads"));
            return new Promise((resolve, reject) => reads.push({ resolve, reject }));
          },
          return() {
            handle2.close();
            return Promise.resolve({ done: true, value: void 0 });
          }
        };
      }
    });
    return {
      handle: handle2,
      finish: (failed, discard2) => finish(void 0, failed, discard2),
      reject: (cause) => finish(cause, cause),
      push(value) {
        if (closed) return;
        if (reads.length) reads.shift().resolve({ done: false, value });
        else if (iterator || !listeners.data.size) {
          const size = JSON.stringify(value).length * 2;
          if (queue.length >= 256 || bytes + size > 1048576) {
            const limit = error("UI_LIMIT", "Channel consumer queue exceeded");
            close(limit, limit);
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

  // src/runtime/leave-guard.js
  function createLeaveGuards(callHost, report2, fail) {
    const holders = /* @__PURE__ */ new Set(), backHolders = /* @__PURE__ */ new Set();
    let epoch2 = 0, sent = false, sentBack = false, asking = false, tail = Promise.resolve();
    const sync = () => {
      const current2 = epoch2;
      tail = tail.then(async () => {
        if (current2 !== epoch2) return;
        const want = holders.size > 0, wantBack = backHolders.size > 0;
        if (want === sent && wantBack === sentBack) return;
        try {
          await callHost("navigation.setLeaveGuard", wantBack ? { enabled: want, back: true } : { enabled: want });
          if (current2 === epoch2) {
            sent = want;
            sentBack = wantBack;
          }
        } catch (error) {
          if (current2 !== epoch2) return;
          for (const holder of holders) holder.fail(error);
          report2(error);
        }
      });
    };
    const request = (reason) => {
      if (!["back", "home"].includes(reason)) throw Error("UI_LEAVE: invalid reason");
      if (asking) return;
      asking = true;
      const current2 = epoch2, pending3 = [...holders].reverse();
      (async () => {
        try {
          for (const holder of pending3) {
            if (current2 !== epoch2 || !holders.has(holder) || !await holder.ask(reason)) return;
          }
          if (current2 === epoch2) await callHost("navigation.leave", { reason });
        } catch (error) {
          if (current2 === epoch2) report2(error);
        } finally {
          if (current2 === epoch2) asking = false;
        }
      })();
    };
    return {
      request,
      reset() {
        epoch2++;
        for (const holder of holders) holder.cancel();
        holders.clear();
        backHolders.clear();
        sent = false;
        sentBack = false;
        asking = false;
        tail = Promise.resolve();
      },
      resource(state) {
        let snapshot = state.value, answer, enabled = false, mounted = false, registeredEpoch = epoch2;
        const update = (patch) => {
          snapshot = { ...snapshot, ...patch };
          state.set(snapshot);
        };
        const decide = (leave2) => {
          if (typeof leave2 !== "boolean") throw Error("UI_LEAVE: expected boolean answer");
          const resolve = answer;
          answer = void 0;
          if (!resolve) return;
          update({ pending: null });
          resolve(leave2);
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
            update({ error: resourceError(error) });
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
      },
      backHandler() {
        let enabled = false, callback = null, registeredEpoch = epoch2;
        const holder = {
          cancel() {
          },
          ask(reason) {
            if (!enabled || reason !== "back") return Promise.resolve(true);
            const failed = (verb, error) => fail(`UI_HANDLER: onBack ${verb}: ${error instanceof Error ? error.message : String(error)}`);
            try {
              const result = callback?.();
              if (result && typeof result.then === "function") Promise.resolve(result).catch((error) => failed("rejected", error));
            } catch (error) {
              failed("threw", error);
            }
            return Promise.resolve(false);
          },
          fail(error) {
            report2(error);
          }
        };
        return {
          commit(next) {
            callback = next.onBack;
            if (enabled === next.enabled && registeredEpoch === epoch2) return;
            registeredEpoch = epoch2;
            enabled = next.enabled;
            if (enabled) {
              holders.add(holder);
              backHolders.add(holder);
            } else {
              holders.delete(holder);
              backHolders.delete(holder);
            }
            sync();
          },
          dispose() {
            enabled = false;
            callback = null;
            backHolders.delete(holder);
            if (holders.delete(holder)) sync();
          }
        };
      }
    };
  }

  // src/runtime/keys.js
  var made = /* @__PURE__ */ new WeakSet();
  var parents = /* @__PURE__ */ new WeakMap();
  var checked = /* @__PURE__ */ new WeakMap();
  var payloads = /* @__PURE__ */ new WeakMap();
  var positions = /* @__PURE__ */ new WeakMap();
  var latest = null;
  var itemKeys = /* @__PURE__ */ new WeakMap();
  var settled = (keys2) => made.has(keys2) || isFrozen(keys2);
  function keysOf(items, getKey) {
    const known = itemKeys.get(items);
    if (known) return known;
    const frozen2 = isFrozen(items);
    const base = frozen2 && latest && extends_(items, latest.items) ? latest : null;
    if (!base && frozen2 && latest && latest.getKey === getKey && sameKeys(items, latest, getKey)) {
      itemKeys.set(items, latest.keys);
      latest = { ...latest, items };
      return latest.keys;
    }
    let keys2;
    if (base) {
      keys2 = base.keys.slice();
      for (let i = base.items.length; i < items.length; i++) keys2.push(getKey(items[i], i));
    } else keys2 = items.map(getKey);
    if (frozen2) {
      made.add(keys2);
      if (base && base.keys.length) parents.set(keys2, base.keys);
      itemKeys.set(items, keys2);
      latest = { items, keys: keys2, getKey };
    }
    return keys2;
  }
  function sameKeys(items, { items: before, keys: keys2 }, getKey) {
    if (items.length !== before.length || !isFrozen(before)) return false;
    for (let i = 0; i < items.length; i++) {
      if (items[i] !== before[i] && getKey(items[i], i) !== keys2[i]) return false;
    }
    return true;
  }
  function extends_(items, prefix) {
    if (items.length < prefix.length || !isFrozen(prefix)) return false;
    for (let from = extended(items); from; from = extended(from)) if (from === prefix) return true;
    for (let i = 0; i < prefix.length; i++) if (items[i] !== prefix[i]) return false;
    return true;
  }
  function inherited(parent, table, rebuild) {
    const entry = parent && table.get(parent);
    if (!entry) return null;
    return entry.size === parent.length ? entry : rebuild(parent);
  }
  function checkKeys(keys2, check) {
    if (checked.has(keys2)) return;
    const parent = parents.get(keys2);
    const parentSeen = inherited(parent, checked, (keys3) => new Set(keys3));
    const seen = parentSeen ?? /* @__PURE__ */ new Set();
    for (let i = parentSeen ? parent.length : 0; i < keys2.length; i++) {
      check(keys2[i], seen);
      seen.add(keys2[i]);
    }
    if (settled(keys2)) checked.set(keys2, seen);
  }
  function keyPayload(keys2) {
    let encoded = payloads.get(keys2);
    if (encoded) return encoded;
    const parent = parents.get(keys2);
    const base = parent && payloads.get(parent);
    if (base && keys2.length > parent.length) {
      const tail = JSON.stringify(keys2.slice(parent.length));
      encoded = { payload: `${base.payload.slice(0, -1)},${tail.slice(1)}`, bytes: base.bytes + utf8Bytes(tail, limits.itemKeyBytes) - 1 };
    } else {
      const payload2 = JSON.stringify(keys2);
      encoded = { payload: payload2, bytes: utf8Bytes(payload2, limits.itemKeyBytes) };
    }
    if (settled(keys2)) payloads.set(keys2, encoded);
    return encoded;
  }
  var identity = (key) => `key:${JSON.stringify(key)}`;
  function keyPositions(keys2) {
    let map = positions.get(keys2);
    if (map) return map;
    const parent = parents.get(keys2);
    const base = inherited(parent, positions, (keys3) => new Map(keys3.map((key, i) => [identity(key), i])));
    map = base ?? /* @__PURE__ */ new Map();
    for (let i = base ? parent.length : 0; i < keys2.length; i++) map.set(identity(keys2[i]), i);
    if (settled(keys2)) positions.set(keys2, map);
    return map;
  }

  // src/runtime/encode.js
  var structured = /* @__PURE__ */ new Set(["backgroundGradient", "boxShadow", "ring", "animation"]);
  var encodeValue = (key, value) => structured.has(key) && value !== "none" ? JSON.stringify(value) : value;
  var ignorable = new Set(ignorableFields);
  var authored;
  function authoredName(key) {
    if (!authored) {
      authored = /* @__PURE__ */ new Map();
      const camel = (name) => name.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
      for (const name of [...commonProps, ...Object.values(elements).flatMap((element) => Object.keys(element.props))]) authored.set(camel(name), name);
    }
    if (authored.has(key)) return authored.get(key);
    if (Object.prototype.hasOwnProperty.call(style, key)) return `style ${key}`;
    return key.endsWith("Style") ? `${key.slice(0, -5)}: classes` : key;
  }
  function unknownToHost(key, host) {
    if (field[key] <= host.maxField) return false;
    if (ignorable.has(key)) return true;
    const name = authoredName(key);
    throw Error(`UI_COMPAT: ${name} needs wire field ${field[key]} (${key}), but this app's native View${typeof host.version === "string" ? ` (LingXia ${host.version})` : ""} knows fields up to ${host.maxField}; rebuild the app with a LingXia SDK as new as this lxapp's toolchain, or leave ${name} out`);
  }
  function copyTree(node) {
    let children = node.children;
    for (let i = 0; i < children.length; i++) {
      const child = copyTree(node.children[i]);
      if (child === node.children[i]) continue;
      if (children === node.children) children = children.slice();
      children[i] = child;
    }
    if (node.list) return { ...node, children, list: { ...node.list, states: new Map(node.list.states), cache: new Map(node.list.cache), pure: new Set(node.list.pure) } };
    return children === node.children ? node : { ...node, children };
  }
  function retainCells(cells2) {
    for (const [key, cell] of cells2) {
      render.candidate.locals.set(key, cell);
      for (const collector of render.collectors) collector.set(key, cell);
    }
  }
  var initialWindow = (count) => ({ start: 0, end: Math.min(1, count), leading: 0, trailing: Math.max(0, count - 1) * 64 });
  var slices = /* @__PURE__ */ new WeakMap();
  var sliceable = /* @__PURE__ */ new WeakSet();
  var WIRE_SERIAL = -3;
  var WIRE_REUSE = -2;
  var wireSerial = 0;
  var wireHeld = 0;
  function wireAck(serial) {
    wireHeld = serial;
  }
  function wireReset() {
    wireHeld = 0;
  }
  function encodeTree(tree, windows) {
    const { candidate } = render;
    const host = globalThis.__lingxiaViewHost;
    const older = host && Number.isInteger(host.maxField) && host.maxField < maxField ? (key) => unknownToHost(key, host) : () => false;
    const put = (key, value) => {
      if (!older(key)) output.push(field[key], value);
    };
    const serial = ++wireSerial;
    const output = [];
    let count = 0, plotWork = 0, animationBytes = 0, itemKeyBytes = 0, dataBytes = 0;
    const heads = [], handled = [];
    let lists = 0, deepest = 0;
    const paste = (slice, parent, depth) => {
      if (depth + slice.depth > limits.depth || (count += slice.nodes) > limits.nodes) throw Error("UI_LIMIT: candidate too large");
      if ((plotWork += slice.plotWork) > 16384) throw Error("UI_LIMIT: plot geometry budget exceeded");
      if ((animationBytes += slice.animationBytes) > limits.animationBytes) throw Error("UI_ANIMATION: page payload budget exceeded");
      if ((dataBytes += slice.dataBytes) > limits.dataAttributeBytes) throw Error("UI_PROP: data-* page budget exceeded");
      for (let i = 0; i < slice.handlers.length; i++) {
        const entry = slice.handlers[i];
        candidate.callbacks.set(entry[0], entry[1]);
        handled.push(entry);
      }
      const base = output.length, values = slice.values;
      deepest = Math.max(deepest, depth + slice.depth);
      if (slice.serial === wireHeld && wireHeld) {
        output.push(WIRE_REUSE, parent, slice.serial, slice.offset, slice.nodes, 0);
        slice.serial = serial;
        slice.offset = base;
        heads.push(-1);
        return;
      }
      slice.serial = serial;
      slice.offset = base;
      for (let i = 0; i < values.length; i++) output.push(values[i]);
      output[base + 1] = parent;
      for (let i = 0; i < slice.heads.length; i++) {
        const at = slice.heads[i];
        if (at) output[base + at + 1] += base;
        heads.push(base + at);
      }
    };
    const write = (node, parent, depth) => {
      const slice = slices.get(node);
      if (slice) return paste(slice, parent, depth);
      if (!sliceable.has(node)) return writeNode(node, parent, depth);
      const index = output.length, mark = { heads: heads.length, handled: handled.length, lists, deepest, count, plotWork, animationBytes, dataBytes };
      deepest = depth;
      writeNode(node, parent, depth);
      const heads2 = heads.slice(mark.heads);
      if (lists === mark.lists && !heads2.includes(-1)) {
        const values = output.slice(index), at = heads2.map((head) => head - index);
        for (let i = 1; i < at.length; i++) values[at[i] + 1] -= index;
        slices.set(node, {
          values,
          heads: at,
          handlers: handled.slice(mark.handled),
          depth: deepest - depth,
          nodes: count - mark.count,
          plotWork: plotWork - mark.plotWork,
          animationBytes: animationBytes - mark.animationBytes,
          dataBytes: dataBytes - mark.dataBytes,
          serial,
          offset: index
        });
      }
      deepest = Math.max(deepest, mark.deepest);
    };
    const writeNode = (node, parent, depth) => {
      if (depth > limits.depth || ++count > limits.nodes) throw Error("UI_LIMIT: candidate too large");
      deepest = Math.max(deepest, depth);
      const { tag, props, identity: identity2, fields: fields3, states: states2, wire, handlers } = node;
      const index = output.length;
      heads.push(index);
      output.push(tags[tag], parent, props.key ?? "", props.testId ?? "", node.text, 0);
      const start = output.length;
      for (const key in fields3) {
        const value = fields3[key];
        if (key === "animation") {
          animationBytes += utf8Bytes(JSON.stringify(value), 32768);
          if (animationBytes > limits.animationBytes) throw Error("UI_ANIMATION: page payload budget exceeded");
        }
        if (older(key)) continue;
        output.push(field[key], encodeValue(key, value));
      }
      for (const key in states2) {
        const style2 = states2[key], pairs = [];
        for (const name in style2) if (!older(name)) pairs.push(field[name], encodeValue(name, style2[name]));
        if (pairs.length) put(key, JSON.stringify(pairs));
      }
      for (let i = 0; i < wire.length; i++) {
        const key = wire[i][0], value = wire[i][1];
        if (key === "dataAttributes") {
          dataBytes += utf8Bytes(value, limits.dataAttributeBytes);
          if (dataBytes > limits.dataAttributeBytes) throw Error("UI_PROP: data-* page budget exceeded");
        }
        put(key, value);
      }
      for (let i = 0; i < handlers.length; i++) {
        const handler = handlers[i];
        if (older(handler[0])) continue;
        candidate.callbacks.set(handler[1], handler[2]);
        handled.push([handler[1], handler[2]]);
        output.push(field[handler[0]], handler[1]);
      }
      if (node.plotWork) {
        plotWork += node.plotWork;
        if (plotWork > 16384) throw Error("UI_LIMIT: plot geometry budget exceeded");
      }
      if (node.rowIndex !== void 0) put("listRowIndex", node.rowIndex);
      if (node.slot !== void 0) put("childSlot", node.slot);
      let window, seededAnchor = false;
      if (node.list) {
        lists++;
        window = windows[identity2] ?? node.list.window ?? initialWindow(props.count);
        if (!Number.isSafeInteger(window.start) || !Number.isSafeInteger(window.end) || window.start < 0 || window.end < window.start || window.end > Math.max(props.count, node.list.previousCount) || ![window.leading, window.trailing].every((v) => Number.isFinite(v) && v >= 0 && v <= 16384e5)) throw Error("UI_LIST: invalid window");
        if (window.end > props.count) window = initialWindow(props.count);
        const typography = node.list.rowSource ? JSON.stringify(node.typography ?? null) : void 0;
        if (node.list.reusable && node.list.reusable.typography === typography) {
          node.list.cache = new Map(node.list.reusable.rows);
          node.list.pure = new Set(node.list.reusable.rows.keys());
        }
        node.list.reusable = null;
        node.list.typography = typography;
        const target = props.anchor === "end" ? props.count - 1 : Math.min(props.anchor?.index ?? 0, props.count - 1);
        if (node.list.anchorChanged && !props.anchor?.duration && props.count > 0 && (target < window.start || target >= window.end)) {
          node.list.cache = new Map([...node.list.anchorCache].filter(([i, row]) => props.itemKeys === void 0 || row.props.key === `key:${JSON.stringify(props.itemKeys[i])}`));
          seededAnchor = true;
        }
        node.list.anchorChanged = false;
        node.list.anchorCache = null;
        node.list.previousCount = props.count;
        node.list.window = { ...window };
        put("listId", identity2);
        put("listCount", props.count);
        put("listLeading", window.leading);
        put("listTrailing", window.trailing);
        if (props.itemKeys !== void 0) {
          const { payload: payload2, bytes } = keyPayload(props.itemKeys);
          itemKeyBytes += bytes;
          if (itemKeyBytes > limits.itemKeyBytes) throw Error("UI_LIST: itemKeys page budget exceeded");
          put("listItemKeys", payload2);
        }
        if (props.anchor !== void 0) put("listAnchor", JSON.stringify(props.anchor));
        candidate.lists.set(identity2, node.list);
      }
      output[start - 1] = (output.length - start) / 2;
      if (!node.list) {
        const children = node.children;
        for (let i = 0; i < children.length; i++) write(children[i], index, depth + 1);
        return;
      }
      writeRows(node, window, index, depth, write);
      if (seededAnchor) node.list.cache.clear();
    };
    write(tree, -1, 0);
    output.push(WIRE_SERIAL, serial, "", "", "", 0);
    return output;
  }
  function writeRows(node, window, index, depth, write) {
    const visibleKeys = /* @__PURE__ */ new Set();
    for (const i of node.list.cache.keys()) if (i < window.start || i >= window.end) {
      node.list.cache.delete(i);
      node.list.pure.delete(i);
    }
    for (let i = window.start; i < window.end; i++) {
      const cells2 = /* @__PURE__ */ new Map();
      render.collectors.push(cells2);
      try {
        let row = node.list.cache.get(i), key = node.list.states.get(i)?.key;
        if (!row) {
          const { candidate } = render, handles2 = handlesIssued(), declarations = candidate.declarations.size, resources = candidate.resources.size;
          const reads = render.cellReads, contextReads = render.contextReads;
          render.readsView = false;
          ({ row, key } = renderRow(node, i, cells2, depth));
          finishTopLayerRow(row);
          const pure = cells2.size === 0 && !render.readsView && handlesIssued() === handles2 && render.cellReads === reads && render.contextReads === contextReads && candidate.declarations.size === declarations && candidate.resources.size === resources;
          if (pure) node.list.pure.add(i);
          else node.list.pure.delete(i);
        } else {
          row = copyTree(row);
          retainCells(node.list.states.get(i).locals);
        }
        if (visibleKeys.has(key)) throw Error("UI_KEY: duplicate list row");
        visibleKeys.add(key);
        sliceable.add(row);
        write(row, index, depth + 1);
        node.list.cache.set(i, row);
        node.list.states.set(i, { key, locals: cells2 });
      } finally {
        render.collectors.pop();
      }
    }
    for (const [i, state] of node.list.states) {
      if (state.locals.size) retainCells(state.locals);
      else if (i < window.start || i >= window.end) node.list.states.delete(i);
    }
  }
  function renderRow(node, i, cells2, depth) {
    const since = now();
    try {
      return renderRowTimed(node, i, cells2, depth);
    } finally {
      spent("rows", since);
    }
  }
  function renderRowTimed(node, i, cells2, depth) {
    const { props, identity: identity2 } = node;
    const suppliedKey = props.itemKeys?.[i];
    const previous = render.scope, outerContexts = render.contexts;
    render.contexts = node.list.contexts;
    render.scope = suppliedKey === void 0 ? `${identity2}/row:${i}` : `${identity2}/row/key:${JSON.stringify(suppliedKey)}`;
    try {
      const value = props.row(i);
      render.contexts = outerContexts;
      const declared2 = isNode(value) ? readNode(value)[1].key : void 0;
      if (declared2 !== void 0 && typeof declared2 !== "string") throw Error("UI_KEY: expected string");
      if (suppliedKey === void 0 && declared2 !== void 0 && cells2.size > 0)
        throw Error("UI_LIST: keyed row factory hooks require itemKeys; alternatively move hooks into a keyed component");
      if (suppliedKey !== void 0 && declared2 !== void 0 && suppliedKey !== declared2) throw Error("UI_LIST: row key does not match itemKeys");
      const rowKey = suppliedKey ?? declared2;
      const key = rowKey === void 0 ? `index:${i}` : `key:${JSON.stringify(rowKey)}`;
      render.scope = `${identity2}/row/${key}`;
      const fields3 = {};
      if (props.height) {
        const height = props.height(i);
        if (!Number.isFinite(height) || height < 0 || height > 16384) throw Error("UI_LIST: invalid row height");
        fields3.height = height;
      }
      const content = resolveNode(value, render.scope, depth + 2, {
        typography: node.typography,
        sibling: { index: i, count: props.count },
        direction: node.list.direction,
        contexts: node.list.contexts
      });
      if (content?.fragment) throw Error("UI_LIST: a row returns one node; wrap its fragment in <view>");
      const row = {
        tag: "column",
        element: "view",
        props: { key },
        fields: fields3,
        states: {},
        wire: [],
        handlers: [],
        identity: render.scope,
        rowIndex: i,
        text: "",
        children: content === null ? [] : [content, ...liftedFrom(content, "row")]
      };
      return { row, key };
    } finally {
      render.scope = previous;
      render.contexts = outerContexts;
    }
  }

  // src/runtime/context.js
  var unstamped = /* @__PURE__ */ new WeakMap();
  var unnamed = 0;
  function contextId(context) {
    if (typeof context.__lxid === "string") return context.__lxid;
    let id = unstamped.get(context);
    if (id === void 0) unstamped.set(context, id = `context#${++unnamed}`);
    return id;
  }
  function lookup(chain, id) {
    while (chain && chain.id !== id) chain = chain.next;
    return chain;
  }
  function plain(value, budget) {
    if (value === null || typeof value !== "object" || --budget.left < 0 || brand in value || cellOf(value)) return true;
    if (Array.isArray(value)) return value.every((each) => plain(each, budget));
    const proto = Object.getPrototypeOf(value);
    if (proto !== Object.prototype && proto !== null) return false;
    for (const descriptor of Object.values(Object.getOwnPropertyDescriptors(value)))
      if ("value" in descriptor && !plain(descriptor.value, budget)) return false;
    return true;
  }
  function checkValue(value) {
    if (!plain(value, { left: 256 })) throw Error("UI_CONTEXT: value must be plain data, functions or handles");
  }
  function createContext(...args) {
    if (args.length !== 1) throw Error("UI_CONTEXT: createContext takes a default value");
    if (render.active) throw Error("UI_CONTEXT: createContext belongs at module top level");
    checkValue(args[0]);
    const context = {};
    const Provider = () => {
      throw Error("UI_CONTEXT: a Provider renders only as an element");
    };
    Object.defineProperty(Provider, "__lxContext", { value: context });
    Object.defineProperty(Provider, "__lxid", { get: () => `${contextId(context)}.Provider` });
    Object.defineProperty(context, "Provider", { value: Provider, enumerable: true });
    Object.defineProperty(context, "defaultValue", { value: args[0] });
    return context;
  }
  function isProvider(tag) {
    return Boolean(tag.__lxContext);
  }
  function provide(tag, props, children, chain, resolve) {
    for (const key in props) if (key !== "value" && key !== "key") throw Error(`UI_CONTEXT: a Provider takes only value, not ${key}`);
    if (!("value" in props)) throw Error("UI_CONTEXT: a Provider needs a value");
    checkValue(props.value);
    const child = children.length === 1 ? children[0] : void 0;
    if (children.length > 1 || child != null && child !== false && !isNode(child)) throw Error("UI_CONTEXT: a Provider wraps one element");
    const result = resolve(child, { id: contextId(tag.__lxContext), value: props.value, next: chain });
    if (result?.fragment) throw Error("UI_CONTEXT: a Provider wraps one element");
    return result;
  }

  // src/runtime/clock.js
  var steps = { second: 1e3, minute: 6e4 };
  var cells = Object.fromEntries(Object.entries(steps).map(([name, step]) => [name, { value: null, clockStep: step }]));
  var now2 = null;
  var floored = (ms, step) => Math.floor(ms / step) * step;
  var current = (cell) => page.updates.has(cell) ? page.updates.get(cell) : cell.value;
  function noteClock(cell) {
    if (cell.clockStep && !(render.clockStep > 0 && render.clockStep <= cell.clockStep)) render.clockStep = cell.clockStep;
  }
  function useNow(granularity) {
    const cell = cells[granularity];
    if (!cell) throw Error('UI_PROP: useNow takes "second" or "minute"');
    if (!render.active) throw Error("UI_SCOPE: useNow is called during render");
    if (now2 === null) setNow(Date.now());
    noteRead(cell);
    noteClock(cell);
    return current(cell);
  }
  function setNow(ms) {
    if (typeof ms !== "number" || !Number.isFinite(ms)) return false;
    now2 = ms;
    let moved = false;
    for (const cell of Object.values(cells)) {
      const next = floored(ms, cell.clockStep);
      if (current(cell) === next) continue;
      const shown = cell.value !== null && clockStep() > 0 && cell.clockStep >= clockStep();
      if (!shown) cell.value = next;
      else {
        page.updates.set(cell, next);
        moved = true;
      }
    }
    return moved;
  }
  var clockStep = () => render.clockStep ?? 0;
  function commitClock() {
    for (const cell of Object.values(cells)) if (page.updates.has(cell)) cell.value = page.updates.get(cell);
  }
  function resetClock() {
    now2 = null;
    for (const cell of Object.values(cells)) cell.value = null;
  }

  // src/runtime/memo.js
  var compareBudget = 256;
  var stamps = /* @__PURE__ */ new WeakMap();
  function viewStamp(view) {
    let stamp = stamps.get(view);
    if (!stamp) stamps.set(view, stamp = { actions: view?.actions, channels: view?.channels });
    return stamp;
  }
  var sameView = (a, b) => a.actions === b.actions && a.channels === b.channels;
  var envValues = /* @__PURE__ */ new WeakMap();
  function envValue(view, key) {
    let values = envValues.get(view);
    if (!values) envValues.set(view, values = /* @__PURE__ */ new Map());
    let value = values.get(key);
    if (value === void 0) {
      const env = envOf(view);
      values.set(key, value = JSON.stringify((key === "*" ? env : env?.[key]) ?? null));
    }
    return value;
  }
  function sameEnv(fields3, view) {
    for (const [key, value] of fields3) if (envValue(view, key) !== value) return false;
    return true;
  }
  function same(a, b, old, budget) {
    if (Object.is(a, b)) return true;
    if (typeof a !== "object" || typeof b !== "object" || a === null || b === null || --budget.left < 0) return false;
    const nodeA = brand in a, nodeB = brand in b;
    if (nodeA || nodeB) {
      if (!nodeA || !nodeB) return false;
      const x = currentArena()[a[brand][1]], y = old[b[brand][1]];
      return Boolean(x && y) && x[0] === y[0] && sameFields(x[1], y[1], old, budget) && same(x[2], y[2], old, budget);
    }
    if (Array.isArray(a)) {
      if (!Array.isArray(b) || a.length !== b.length) return false;
      for (let i = 0; i < a.length; i++) if (!same(a[i], b[i], old, budget)) return false;
      return true;
    }
    if (Object.getPrototypeOf(a) !== Object.prototype || Object.getPrototypeOf(b) !== Object.prototype) return false;
    return sameFields(a, b, old, budget);
  }
  function sameFields(a, b, old, budget) {
    let count = 0;
    for (const key in a) {
      count++;
      if (!Object.prototype.hasOwnProperty.call(b, key) || !same(a[key], b[key], old, budget)) return false;
    }
    for (const key in b) count--;
    return count === 0;
  }
  var contextKeys = ["tag", "direction", "inControl", "inText", "inLink", "inBlockLink"];
  function sameContext(a, b, old) {
    for (let i = 0; i < contextKeys.length; i++) if (a[contextKeys[i]] !== b[contextKeys[i]]) return false;
    const budget = { left: compareBudget };
    return same(a.typography, b.typography, old, budget) && same(a.extra, b.extra, old, budget) && same(a.every, b.every, old, budget) && same(a.sibling, b.sibling, old, budget) && same(a.popovers, b.popovers, old, budget);
  }
  function sameContexts(reads, chain, old, budget) {
    for (const [id, read] of reads) {
      const entry = lookup(chain, id);
      if (!same(entry ? entry.value : read.context.defaultValue, read.value, old, budget)) return false;
    }
    return true;
  }
  function sameChain(a, b, old, budget) {
    for (; a || b; a = a.next, b = b.next) if (!a || !b || a.id !== b.id || !same(a.value, b.value, old, budget)) return false;
    return true;
  }
  function passContexts(reads, chain, outer) {
    for (const [id, read] of reads) if (!outer.contextReads.has(id) && lookup(chain, id) === lookup(outer.contexts, id)) outer.contextReads.set(id, read);
  }
  var topLayerSize = () => {
    const top = render.candidate.topLayer;
    return top ? top.entries.length + top.popovers.size + top.invokers.length : 0;
  };
  function recall(scope, tag, props, children, parent) {
    const since = now();
    try {
      return recallTimed(scope, tag, props, children, parent);
    } finally {
      spent("recall", since);
    }
  }
  function recallTimed(scope, tag, props, children, parent) {
    const record = render.recall?.get(scope);
    if (!record || record.tag !== tag || tag.__lxVolatile || !sameView(record.view, viewStamp(render.view)) || !sameEnv(record.env, render.view)) return void 0;
    for (const cell of record.reads) if (page.updates.has(cell)) return void 0;
    for (const cell of record.cells.values()) if (page.updates.has(cell)) return void 0;
    const budget = { left: compareBudget };
    if (!sameFields(props, record.props, record.arena, budget) || !same(children, record.children, record.arena, budget) || !sameContext(parent, record.parent, record.arena) || !sameContexts(record.contextReads, parent.contexts, record.arena, budget) || record.lists && !sameChain(parent.contexts, record.parent.contexts, record.arena, budget)) return void 0;
    const fresh2 = { ...record, props, children, parent, arena: currentArena(), result: record.lists ? reuse(record.result) : record.result };
    keep(fresh2);
    const frame = render.frames[render.frames.length - 1];
    if (frame) {
      frame.nested.push(fresh2);
      for (const cell of record.reads) frame.reads.add(cell);
      for (const key of record.env.keys()) frame.env.add(key);
      passContexts(record.contextReads, parent.contexts, frame);
    }
    for (const cell of record.reads) noteClock(cell);
    for (const [key, cell] of record.cells) {
      render.candidate.locals.set(key, cell);
      for (const collector of render.collectors) collector.set(key, cell);
    }
    if (record.handles) skipHandle();
    if (record.lists) render.lists++;
    return fresh2.result;
  }
  function reuse(node) {
    let children = node.children;
    for (let i = 0; i < children.length; i++) {
      const child = reuse(node.children[i]);
      if (child === node.children[i]) continue;
      if (children === node.children) children = children.slice();
      children[i] = child;
    }
    if (!node.list) return children === node.children ? node : { ...node, children };
    const list = page.committed?.lists.get(node.identity) ?? node.list;
    const { pure } = list, cache2 = /* @__PURE__ */ new Map();
    for (const [i, row] of list.cache) if (pure.has(i)) cache2.set(i, row);
    return { ...node, children, list: {
      ...list,
      states: new Map(list.states),
      cache: cache2,
      pure: new Set(cache2.keys()),
      reusable: null,
      anchorChanged: false,
      anchorCache: null
    } };
  }
  function keep(record) {
    render.candidate.memo.set(record.scope, record);
    for (const [key, entry] of record.closures) render.candidate.closures?.set(key, entry);
    for (const nested of record.nested) keep(nested);
  }
  function enter(scope, contexts) {
    const frame = { scope, contexts, contextReads: /* @__PURE__ */ new Map(), cells: /* @__PURE__ */ new Map(), reads: /* @__PURE__ */ new Set(), env: /* @__PURE__ */ new Set(), view: false, nested: [], closures: [], handles: handlesIssued(), top: topLayerSize(), lists: render.lists };
    render.frames.push(frame);
    render.collectors.push(frame.cells);
    return frame;
  }
  function noteRead(cell) {
    render.cellReads++;
    const frame = render.frames[render.frames.length - 1];
    if (frame) frame.reads.add(cell);
  }
  function noteContext(id, context, value) {
    render.contextReads++;
    const frame = render.frames[render.frames.length - 1];
    if (frame && !frame.contextReads.has(id)) frame.contextReads.set(id, { context, value });
  }
  function noteEnv(keys2) {
    const frame = render.frames[render.frames.length - 1];
    if (!frame) return;
    if (typeof keys2 === "string") frame.env.add(keys2);
    else for (let i = 0; i < keys2.length; i++) frame.env.add(keys2[i]);
  }
  function noteView() {
    const frame = render.frames[render.frames.length - 1];
    if (frame) frame.view = true;
  }
  function leave(frame) {
    render.frames.splice(render.frames.lastIndexOf(frame), 1);
    render.collectors.splice(render.collectors.lastIndexOf(frame.cells), 1);
    const outer = render.frames[render.frames.length - 1];
    if (outer) {
      for (const cell of frame.reads) outer.reads.add(cell);
      for (const key of frame.env) outer.env.add(key);
      outer.view || (outer.view = frame.view);
      passContexts(frame.contextReads, frame.contexts, outer);
    }
  }
  function remember(frame, tag, props, children, parent, result) {
    if (frame.view || result?.fragment || topLayerSize() !== frame.top || !render.candidate.memo) return;
    const record = {
      scope: frame.scope,
      tag,
      props,
      children,
      parent,
      arena: currentArena(),
      result,
      cells: frame.cells,
      reads: frame.reads,
      contextReads: frame.contextReads,
      nested: frame.nested,
      closures: frame.closures,
      view: viewStamp(render.view),
      env: new Map([...frame.env.has("*") ? ["*"] : frame.env].map((key) => [key, envValue(render.view, key)])),
      handles: handlesIssued() !== frame.handles,
      lists: render.lists !== frame.lists
    };
    render.candidate.memo.set(frame.scope, record);
    render.frames[render.frames.length - 1]?.nested.push(record);
    if (result && !record.lists) sliceable.add(result);
  }

  // src/runtime/toplayer.js
  var panelMark = Symbol("top-layer panel");
  var dragClose = Symbol("drag close");
  var scrimMark = Symbol("drag scrim");
  var presenceStyle = Symbol("backdrop presence");
  var backdropOwner = Symbol("backdrop owner");
  var popoverScope = Symbol("popover scope");
  var popoverTags = /* @__PURE__ */ new Set(["view"]);
  var closers = /* @__PURE__ */ new Set(["any", "closerequest", "none"]);
  var layerZ = 9e3;
  var uncontrolled = /* @__PURE__ */ new Set();
  var anchors = /* @__PURE__ */ new Map();
  var opened = /* @__PURE__ */ new Map();
  var refused = /* @__PURE__ */ new Set();
  var resting = /* @__PURE__ */ new Map();
  var invoker = null;
  var sequence = 0;
  var submenus = /* @__PURE__ */ new Map();
  function resetTopLayer() {
    uncontrolled = /* @__PURE__ */ new Set();
    anchors = /* @__PURE__ */ new Map();
    opened = /* @__PURE__ */ new Map();
    refused = /* @__PURE__ */ new Set();
    resting = /* @__PURE__ */ new Map();
    invoker = null;
    sequence = 0;
    submenus = /* @__PURE__ */ new Map();
  }
  function collected() {
    var _a;
    return (_a = render.candidate).topLayer ?? (_a.topLayer = { entries: [], popovers: /* @__PURE__ */ new Map(), invokers: [], interests: [] });
  }
  function admitted(identity2) {
    const top = collected();
    if (top.entries.length >= limits.topLayer) {
      if (!refused.has(identity2)) {
        refused.add(identity2);
        report(Error(`UI_BUDGET: more than ${limits.topLayer} dialogs and popovers open; ${identity2} stays closed`));
      }
      return false;
    }
    refused.delete(identity2);
    if (!opened.has(identity2)) opened.set(identity2, ++sequence);
    return true;
  }
  function placement(props, env) {
    const set = { ...resolveClasses(props.className, env)?.base, ...props.style };
    const margins = {};
    for (const side of ["Top", "Bottom", "Left", "Right"]) {
      const inline = side === "Left" || side === "Right";
      if (set[`margin${side}`] === void 0 && !(inline && (set.marginInlineStart !== void 0 || set.marginInlineEnd !== void 0)))
        margins[`margin${side}`] = "auto";
    }
    return margins;
  }
  var rtlOf = (parent, env) => (parent.direction ?? env?.direction) === "rtl";
  function layerWith(style2, identity2, env, ...children) {
    return h("view", { style: { position: "absolute", top: 0, left: 0, right: 0, bottom: env?.keyboard?.height ?? 0, zIndex: layerZ + opened.get(identity2) % 1e3, ...style2 } }, ...children);
  }
  var sides2 = ["Top", "Right", "Bottom", "Left"];
  function barInsets(env) {
    const insets = env?.insets;
    if (!insets) return null;
    return {
      Top: insets.top ?? 0,
      Right: insets.right ?? 0,
      Left: insets.left ?? 0,
      Bottom: Math.max(0, (insets.bottom ?? 0) - (env.keyboard?.height ?? 0))
    };
  }
  var zero = (value) => value === 0 || Array.isArray(value) && value[1] === 0 && value[2] === 0;
  function inlineField(set, prefix, side, rtl) {
    const logical2 = `${prefix}InlineStart`, other = `${prefix}InlineEnd`;
    const mapped = side === "Left" !== rtl ? logical2 : other;
    return set[mapped] !== void 0 ? mapped : `${prefix}${side}`;
  }
  function docked(set, env, rtl) {
    const viewport = env?.viewport;
    const full = (size, max, extent) => set[size] === "100%" && (set[max] === void 0 || typeof set[max] === "number" && extent !== void 0 && set[max] >= extent);
    const reaches = {
      Top: full("height", "maxHeight", viewport?.height),
      Bottom: full("height", "maxHeight", viewport?.height),
      Left: full("width", "maxWidth", viewport?.width),
      Right: full("width", "maxWidth", viewport?.width)
    };
    const inline = (side) => side === "Left" || side === "Right";
    const inset = (side) => set[side.toLowerCase()] ?? (inline(side) ? set[side === "Left" !== rtl ? "start" : "end"] : void 0);
    const meets = (side) => zero(set[inline(side) ? inlineField(set, "margin", side, rtl) : `margin${side}`]) || set.position === "absolute" && zero(inset(side));
    const spans2 = (side) => inline(side) ? set.height === "100%" || set.position === "absolute" && zero(inset("Top")) && zero(inset("Bottom")) : set.width === "100%" || set.position === "absolute" && zero(inset("Left")) && zero(inset("Right"));
    return new Set(sides2.filter((side) => reaches[side] || meets(side) && spans2(side)));
  }
  function dock(props, env, rtl) {
    const insets = barInsets(env);
    if (!insets) return { layer: {}, panel: {} };
    const set = { ...resolveClasses(props.className, { ...env, direction: rtl ? "rtl" : "ltr" })?.base, ...props.style };
    const edges = docked(set, env, rtl);
    const layer = {}, panel = {};
    for (const side of sides2) {
      if (!edges.has(side)) {
        layer[`padding${side}`] = insets[side];
        continue;
      }
      if (!insets[side]) continue;
      const field2 = side === "Left" || side === "Right" ? inlineField(set, "padding", side, rtl) : `padding${side}`;
      const own2 = set[field2] ?? set.padding ?? 0;
      if (typeof own2 === "number") panel[field2] = own2 + insets[side];
      else if (Array.isArray(own2)) panel[field2] = [own2[0], own2[1], own2[2] + insets[side]];
    }
    return { layer, panel };
  }
  var backdropTiming = { durationMs: 200, delayMs: 0, easing: [0, 0, 0.58, 1] };
  function backdropOf(classes) {
    const own2 = classes?.base?.transition;
    const timing = { ...own2 && typeof own2 === "object" ? { durationMs: own2.durationMs, delayMs: own2.delayMs, easing: own2.easing } : backdropTiming, ...classes?.backdrop?.timing };
    const fades = (name) => classes?.[name] && Object.keys(classes[name]).length ? { opacity: 0 } : {};
    return {
      style: { opacity: 1, backgroundColor: "scrim", transition: { properties: ["opacity"], ...timing }, ...classes?.backdrop?.base },
      presence: { starting: { ...fades("starting"), ...classes?.backdrop?.starting }, exit: { ...fades("exit"), ...classes?.backdrop?.exit } }
    };
  }
  function expand(tag, props, children, identity2, parent) {
    if (props[panelMark]) return void 0;
    if (tag === "dialog") return dialog(props, children, identity2, parent);
    if (popoverTags.has(tag) && props.popover !== void 0 && props.popover !== false) return popover(tag, props, children, identity2, parent);
    return void 0;
  }
  function common(props, parent, what) {
    if (parent.inControl) throw Error(`UI_BUTTON: a ${what} cannot sit inside a pressable control`);
    if (props.open !== void 0 && typeof props.open !== "boolean") throw Error("UI_PROP: open must be boolean");
  }
  function dialog(props, children, identity2, parent) {
    common(props, parent, "<dialog>");
    const { open: open2, onClose, closedby = "closerequest", ...rest } = props;
    if (onClose !== void 0 && typeof onClose !== "function") throw Error("UI_PROP: onClose");
    if (!closers.has(closedby)) throw Error('UI_PROP: closedby must be "any", "closerequest" or "none"');
    const opening = !opened.has(identity2);
    if (!open2 || !admitted(identity2)) return null;
    const bound = rest.drag !== void 0 && rest.bind !== void 0 ? cellOf(rest.bind) : void 0;
    if (opening && bound && typeof bound.cell.value === "number") resting.set(identity2, { ...bound, index: bound.cell.value });
    const close = () => onClose?.();
    collected().entries.push({ identity: identity2, kind: "dialog", sequence: opened.get(identity2), close: closedby === "none" ? null : close });
    noteEnv("*");
    const env = envOf(render.view);
    const backdrop2 = backdropOf(resolveClasses(props.className, env));
    const testId = props["data-testid"];
    const docking = dock(rest, env, rtlOf(parent, env));
    return {
      node: layerWith(
        docking.layer,
        identity2,
        env,
        h("button", {
          key: "backdrop",
          "aria-hidden": true,
          "data-testid": testId && `${testId}-backdrop`,
          onClick: closedby === "any" ? close : () => {
          },
          ...rest.drag !== void 0 ? { [scrimMark]: true } : {},
          [presenceStyle]: backdrop2.presence,
          style: { position: "absolute", top: 0, left: 0, right: 0, bottom: 0, ...backdrop2.style }
        }),
        h("dialog", { ...rest, key: "panel", [panelMark]: true, [backdropOwner]: true, [popoverScope]: parent.popovers ?? [], ...closedby === "none" ? {} : { [dragClose]: close }, style: { ...placement(rest, env), ...rest.style, ...docking.panel } }, ...children)
      ),
      finish: (result) => {
        result.wire.push(["topLayer", 1]);
      }
    };
  }
  function popover(tag, props, children, identity2, parent) {
    common(props, parent, "popover");
    const { popover: type, id, open: open2, onToggle, onCloseRequest, closeAfter, anchor: anchorProp, ...rest } = props;
    const kind = type === true || type === "" || type === "auto" ? "auto" : type === "manual" || type === "hint" ? type : null;
    if (!kind) throw Error('UI_PROP: popover is "auto", "manual" or "hint"');
    if (typeof id !== "string" || !id.length || id.length > 256) throw Error("UI_ID: a popover needs an id of 1..256 characters");
    if (onToggle !== void 0 && typeof onToggle !== "function") throw Error("UI_PROP: onToggle");
    if (onCloseRequest !== void 0 && typeof onCloseRequest !== "function") throw Error("UI_PROP: onCloseRequest takes a function");
    if (anchorProp !== void 0 && (typeof anchorProp !== "string" || !anchorProp.length || anchorProp.length > 256 || /\s/.test(anchorProp)))
      throw Error("UI_ID: anchor names an element id");
    const [, anchorId, anchorPart] = anchorProp === void 0 ? [] : /^(.+?)(?::(caret|selection))?$/.exec(anchorProp);
    if (closeAfter !== void 0 && (kind !== "manual" || !Number.isInteger(closeAfter) || closeAfter < 1 || closeAfter > 6e4))
      throw Error('UI_PROP: closeAfter takes 1..60000 ms on a popover="manual"');
    const top = collected();
    if (top.popovers.has(id)) throw Error(`UI_ID: two popovers have the id "${id}"`);
    const controlled = open2 !== void 0;
    const shown = controlled ? open2 : uncontrolled.has(id);
    const scope = parent.popovers ?? [];
    const entry = { kind, controlled, onToggle, open: false, ancestors: scope, anchorId };
    top.popovers.set(id, entry);
    if (!shown || !admitted(identity2)) {
      anchors.delete(id);
      submenus.delete(id);
      return null;
    }
    entry.open = true;
    if (!anchors.has(id)) anchors.set(id, invoker?.id === id ? invoker.identity : null);
    const anchor = anchorId ?? anchors.get(id);
    top.entries.push({ identity: identity2, kind, sequence: opened.get(identity2), close: kind === "manual" ? null : () => {
      if (onCloseRequest?.() !== false) toggle(id, false);
    } });
    noteEnv("*");
    const env = envOf(render.view);
    const docking = anchor ? { layer: {}, panel: {} } : dock(rest, env, rtlOf(parent, env));
    const lit = kind === "auto" && !submenus.has(id);
    const classes = lit ? resolveClasses(rest.className, env) : void 0;
    const backdrop2 = classes?.backdrop ? backdropOf(classes) : null;
    const style2 = anchor ? { ...rest.style, position: "absolute", top: 0, left: 0 } : { ...placement(rest, env), ...rest.style, ...docking.panel };
    const testId = props["data-testid"];
    const node = layerWith(
      docking.layer,
      identity2,
      env,
      lit && h("button", {
        key: "backdrop",
        "aria-hidden": true,
        "data-testid": testId && `${testId}-backdrop`,
        onClick: () => toggle(id, false),
        ...backdrop2 ? { [presenceStyle]: backdrop2.presence } : {},
        style: { position: "absolute", top: 0, left: 0, right: 0, bottom: 0, ...backdrop2 ? { ...backdrop2.style, backgroundColor: "transparent", ...classes.backdrop.base } : { backgroundColor: "transparent" } }
      }),
      h(tag, { ...rest, id, key: "panel", [panelMark]: true, ...lit ? { [backdropOwner]: true } : {}, [popoverScope]: [...scope, id], style: style2 }, ...children)
    );
    return {
      node,
      finish: (result) => {
        result.wire.push(["topLayer", kind === "manual" ? 2 : 1]);
        if (kind === "hint") result.wire.push(["hint", 1]);
        if (closeAfter !== void 0) result.wire.push(["closeAfter", closeAfter]);
        const panel = result.children.find((child) => child.props?.key === "panel");
        if (anchor && panel) panel.wire.push(["anchorTo", anchorName(id) + (anchorPart ? `:${anchorPart}` : "")]);
      }
    };
  }
  var anchorName = (id) => `--lx-popover-${id}`;
  function toggle(id, next, from) {
    const top = page.committed?.topLayer;
    const entry = top?.popovers.get(id);
    if (!entry || entry.open === next) return false;
    if (next && entry.kind !== "manual") {
      const kept2 = /* @__PURE__ */ new Set([...entry.ancestors, ...from?.scope ?? [], ...menuChain(from?.menu)]);
      for (const [other, state] of top.popovers)
        if (other !== id && state.open && (state.kind === "hint" || entry.kind === "auto" && state.kind === "auto") && !kept2.has(other))
          toggle(other, false);
    }
    if (next) {
      invoker = from ? { id, identity: from.identity } : null;
      if (from?.menu !== void 0) submenus.set(id, from.menu);
      else submenus.delete(id);
    } else {
      for (const [child, parent] of [...submenus]) if (parent === id) toggle(child, false);
      submenus.delete(id);
    }
    entry.open = next;
    if (!entry.controlled) {
      if (next) uncontrolled.add(id);
      else uncontrolled.delete(id);
      page.invalidated = true;
    }
    entry.onToggle?.(next);
    return true;
  }
  function menuChain(id) {
    const chain = [];
    for (let at = id; at !== void 0 && !chain.includes(at); at = submenus.get(at)) chain.push(at);
    return chain;
  }
  function closeAfterExpired(id) {
    const entry = page.committed?.topLayer?.popovers.get(id);
    return entry?.kind === "manual" && entry.open ? toggle(id, false) : false;
  }
  function closePassing() {
    const top = page.committed?.topLayer;
    let changed2 = false;
    for (const [id, state] of [...top?.popovers ?? []].reverse())
      if (state.open && state.kind !== "manual") changed2 = toggle(id, false) || changed2;
    return changed2;
  }
  function ownedPopovers() {
    const top = render.candidate?.topLayer ?? page.committed?.topLayer;
    return top ? [...top.popovers].filter(([id, entry]) => entry.open && (entry.anchorId || anchors.get(id))).map(([id]) => id) : [];
  }
  function closeOrphans(ids) {
    const top = render.candidate?.topLayer ?? page.committed?.topLayer;
    for (const id of Array.isArray(ids) ? ids : []) {
      const entry = top?.popovers.get(id);
      if (!entry?.open) continue;
      entry.open = false;
      submenus.delete(id);
      if (!entry.controlled) {
        uncontrolled.delete(id);
        page.invalidated = true;
      }
      entry.onToggle?.(false);
    }
  }
  function dismissHints(keep2) {
    const top = page.committed?.topLayer;
    if (!top || !Array.isArray(keep2)) return false;
    let changed2 = false;
    for (const [id, state] of top.popovers) if (state.kind === "hint" && state.open && !keep2.includes(id)) changed2 = toggle(id, false) || changed2;
    return changed2;
  }
  function invokerHandler(props, identity2, scope) {
    const { popoverTarget: target, onClick, role, keepOpen } = props;
    if (keepOpen !== void 0 && typeof keepOpen !== "boolean") throw Error("UI_PROP: keepOpen is a boolean");
    if (target !== void 0) {
      if (typeof target !== "string" || !target.length) throw Error("UI_ID: popoverTarget names a popover id");
      collected().invokers.push({ target, identity: identity2, scope });
    }
    if (role === "menuitem" && !scope.length) throw Error('UI_PROP: role="menuitem" belongs inside a popover menu');
    if (target === void 0 && (role !== "menuitem" || keepOpen)) return onClick;
    return (event) => {
      onClick?.(event);
      const menu = role === "menuitem" ? scope[scope.length - 1] : void 0;
      if (target !== void 0) toggle(target, !page.committed?.topLayer?.popovers.get(target)?.open, { identity: identity2, scope, menu });
      else for (const id of menuChain(menu)) toggle(id, false);
    };
  }
  function interestTrigger(props, identity2, scope) {
    const target = props.interestFor;
    if (target === void 0) return;
    if (typeof target !== "string" || !target.length || target.length > 256 || /\s/.test(target)) throw Error("UI_ID: interestFor names a popover id");
    collected().interests.push({ target, identity: identity2, scope });
  }
  function interest(target, nth, show4) {
    const entry = page.committed?.topLayer?.interests.filter((candidate) => candidate.target === target)[nth];
    return entry ? toggle(target, show4, { identity: entry.identity, scope: entry.scope }) : false;
  }
  function finishTopLayer(tree) {
    var _a;
    const top = render.candidate.topLayer;
    for (const identity2 of [...opened.keys()]) {
      if (top?.entries.some((entry) => entry.identity === identity2)) continue;
      opened.delete(identity2);
      const rest = resting.get(identity2);
      if (rest) {
        resting.delete(identity2);
        ((_a = render.candidate).dragResets ?? (_a.dragResets = [])).push(rest);
      }
    }
    if (top) wireInvokers(top, tree);
  }
  function finishTopLayerRow(row) {
    const top = render.candidate?.topLayer;
    if (top) wireInvokers(top, row);
  }
  function wireInvokers(top, tree) {
    const invokers = new Map(top.invokers.map((entry) => [entry.identity, entry]));
    const interests = new Map(top.interests.map((entry) => [entry.identity, entry]));
    const named = /* @__PURE__ */ new Map();
    for (const [id, entry] of top.popovers) if (entry.open && entry.anchorId) named.set(entry.anchorId, [...named.get(entry.anchorId) ?? [], id]);
    const walk = (node) => {
      const names3 = [];
      const anchored = named.size && typeof node.props?.id === "string" ? named.get(node.props.id) : void 0;
      if (anchored !== void 0) for (const id of anchored) names3.push(anchorName(id));
      const interest2 = interests.get(node.identity);
      if (interest2) {
        const target = top.popovers.get(interest2.target);
        if (!target) throw Error(`UI_ID: interestFor "${interest2.target}" names no popover on this page`);
        if (target.open && node.props["aria-describedby"] === void 0) node.wire.push(["ariaDescribedBy", interest2.target]);
        if (!invokers.has(node.identity) && target.open && !target.anchorId && anchors.get(interest2.target) === node.identity)
          names3.push(anchorName(interest2.target));
      }
      const entry = invokers.get(node.identity);
      if (entry && (node.element === "button" || node.element === "link")) {
        const target = top.popovers.get(entry.target);
        if (!target) throw Error(`UI_ID: popoverTarget "${entry.target}" names no popover on this page`);
        if (node.props.accessibilityState?.expanded === void 0) node.wire.push(["stateExpanded", +target.open]);
        if (target.open && !target.anchorId && anchors.get(entry.target) === node.identity) names3.push(anchorName(entry.target));
      }
      if (names3.length) node.wire.push(["anchorName", [...new Set(names3)].join("\n")]);
      for (const child of node.children ?? []) walk(child);
    };
    walk(tree);
  }
  function closeTopmost() {
    const entries2 = (page.committed?.topLayer?.entries ?? []).filter((entry) => entry.kind !== "manual");
    const top = [...entries2].sort((a, b) => b.sequence - a.sequence)[0];
    if (!top?.close) return false;
    top.close();
    return true;
  }

  // src/runtime/overlay.js
  var timers = [];
  var nextToken = 1;
  var wakes = /* @__PURE__ */ new Map();
  function overlayTimer(token) {
    const wake = wakes.get(token);
    if (!wake) return false;
    wakes.delete(token);
    return wake() === true;
  }
  function escapeOverlay() {
    return closeTopmost();
  }
  function requestWake(ms, fire) {
    const token = nextToken++;
    wakes.set(token, fire);
    timers.push([token, Math.round(ms)]);
  }
  function takeOverlayTimers() {
    const result = timers;
    timers = [];
    return JSON.stringify(result);
  }
  function resetOverlays() {
    timers = [];
    wakes = /* @__PURE__ */ new Map();
  }

  // src/runtime/snap.js
  var CEILING_MS = 400;
  var settled2 = /* @__PURE__ */ new Map();
  function resetSnaps() {
    settled2 = /* @__PURE__ */ new Map();
  }
  var clock2 = () => Date.now();
  function controlledSnap(key, index) {
    if (settled2.size === 0) return index;
    const record = settled2.get(key);
    if (!record) return index;
    if (index === record.index || index !== record.from) {
      settled2.delete(key);
      return index;
    }
    if (!record.sent) {
      record.sent = true;
      if (!open(record)) render.candidate.invalidateAfterCommit = true;
      return record.index;
    }
    if (open(record)) return record.index;
    settled2.delete(key);
    return index;
  }
  function open(record) {
    if (record.ids.length === 0 || record.rejected || clock2() >= record.deadline) return false;
    if (record.ids.some(actionPending)) return true;
    return render.view?.data === record.data;
  }
  function snapSettled(key, index, current2, heard) {
    if (index === current2) {
      settled2.delete(key);
      return;
    }
    const first = actionSerial();
    try {
      heard?.();
    } finally {
      const ids = [];
      for (let serial = first; serial < actionSerial(); serial++) ids.push(serial);
      const record = { index, from: current2, ids, rejected: false, sent: false, data: page.committed?.view?.data, deadline: clock2() + CEILING_MS };
      settled2.set(key, record);
      page.invalidated = true;
      if (ids.length) requestWake(CEILING_MS, () => {
        if (settled2.get(key) !== record) return false;
        page.invalidated = true;
        return true;
      });
    }
  }
  function snapActionSettled(serial, ok) {
    if (settled2.size === 0) return;
    for (const record of settled2.values()) {
      if (!record.ids.includes(serial)) continue;
      if (!ok) record.rejected = true;
      if (!record.ids.some(actionPending)) record.data = page.committed?.view?.data;
      page.invalidated = true;
    }
  }

  // src/runtime/actions.js
  var actionPage = "";
  var probes = /* @__PURE__ */ new Set(["then", "toJSON", "constructor", "prototype", "__proto__", "toString", "valueOf", "inspect", "nodeType", "tagName", "$$typeof", "asymmetricMatch", "_isMockFunction", "@@__IMMUTABLE_ITERABLE__@@", "@@__IMMUTABLE_RECORD__@@"]);
  var catalog = (built, arrived) => new Proxy(Object.freeze(built), {
    get(target, key) {
      if (!arrived || typeof key === "symbol" || has(target, key) || probes.has(key)) return target[key];
      throw Error(`UI_ACTION: \`${key}\` is not an action of ${actionPage || "this page"}: the build did not see it`);
    }
  });
  function setActionPage(page2) {
    if (typeof page2 === "string") actionPage = page2;
  }
  var actions = catalog(/* @__PURE__ */ Object.create(null), false);
  var actionEpoch = 0;
  var nextRequest = 1;
  var requests = [];
  var actionErrors = [];
  var pendingActions = /* @__PURE__ */ new Map();
  var activeChannels = /* @__PURE__ */ new Map();
  var actionModes = /* @__PURE__ */ new WeakMap();
  var actionNames = /* @__PURE__ */ new WeakMap();
  var closedChannels = Object.freeze({ open() {
    throw Error("UI_CHANNEL: bridge not ready");
  } });
  var channels = closedChannels;
  function report(error) {
    if (actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
  }
  var handlerFailure = "";
  var loud = true;
  function setLoudFailures(on) {
    loud = on === true;
  }
  function loudFailures() {
    return loud;
  }
  function failHandler(message) {
    if (loud) handlerFailure || (handlerFailure = message.slice(0, 2048));
    else report(message);
  }
  function takeHandlerFailure() {
    const result = handlerFailure;
    handlerFailure = "";
    return result;
  }
  function reportHandlerFailure(site, error) {
    if (requests.length >= 512) return;
    const isError = error instanceof Error;
    requests.push({
      kind: "failure",
      prop: site ? String(site[0]) : "",
      testId: site && typeof site[2] === "string" ? site[2] : null,
      action: typeof error?.action === "string" ? error.action : null,
      name: isError ? error.name : "Error",
      message: (isError ? error.uiMessage ?? error.message : String(error)).slice(0, 2048)
    });
  }
  var leaveGuards = createLeaveGuards(callLeaveHost, report, (message) => failHandler(message));
  function currentActions() {
    return { actions, channels };
  }
  function actionInfo(action) {
    const name = actionNames.get(action);
    return name !== void 0 && has(actions, name) && actions[name] === action ? { name, mode: actionModes.get(action) } : void 0;
  }
  function cancelActions() {
    actionEpoch++;
    leaveGuards.reset();
    sentTitle = void 0;
    const pending3 = [...pendingActions.values()];
    pendingActions.clear();
    requests = [];
    actions = catalog(/* @__PURE__ */ Object.create(null), false);
    const oldChannels = [...activeChannels.values()];
    activeChannels.clear();
    channels = closedChannels;
    for (const { reject } of pending3) reject(bridgeError("E_ABORT", "Page reset"));
    for (const entry of oldChannels) {
      const error = bridgeError("E_ABORT", "Page reset");
      entry.reject(error);
      entry.channel.reject(error);
    }
  }
  function resetActions() {
    cancelActions();
    actionErrors = [];
    handlerFailure = "";
  }
  function admit(what) {
    if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256) throw Error(`UI_LIMIT: too many pending ${what}`);
  }
  var ActionPromise = class extends Promise {
    static get [Symbol.species]() {
      return Promise;
    }
    then(onFulfilled, onRejected) {
      this.handled = true;
      return super.then(onFulfilled, onRejected);
    }
  };
  var kindOf2 = (value) => value === null ? "null" : typeof value === "number" ? String(value) : typeof value !== "object" ? typeof value : Array.isArray(value) ? "array" : value.constructor?.name ?? "object";
  function payload(value, refuse, path, depth, budget, ancestors) {
    if (depth > 64) throw Error("UI_LOCAL: value too deep");
    if ((budget.left -= typeof value === "string" ? value.length + 2 : 8) < 0) throw Error(`UI_LIMIT: ${budget.what} payload too large`);
    if (value === null || typeof value === "string" || typeof value === "boolean") return value;
    if (typeof value === "number" && Number.isFinite(value)) return value;
    const array = Array.isArray(value);
    const plain3 = !array && value && typeof value === "object" && Object.getPrototypeOf(value) === Object.prototype;
    if (array || plain3) {
      const here = path ? path.startsWith("[") ? `argument${path}` : `argument.${path}` : "argument";
      if (ancestors.has(value)) {
        const code = budget.what === "action" ? "UI_ACTION" : "UI_CHANNEL";
        throw Error(`${code}: ${budget.who} payload field ${here} refers back to ${ancestors.get(value)}; send a value without cycles`);
      }
      ancestors.set(value, here);
      try {
        if (array) return Object.freeze(Array.from(value, (v, i) => v === void 0 ? null : payload(v, refuse, `${path}[${i}]`, depth + 1, budget, ancestors)));
        const out = {};
        for (const [k, v] of Object.entries(value)) {
          if (v === void 0) continue;
          budget.left -= k.length + 3;
          const nextPath = /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(k) ? path ? `${path}.${k}` : k : `${path}[${JSON.stringify(k)}]`;
          out[k] = payload(v, refuse, nextPath, depth + 1, budget, ancestors);
        }
        return Object.freeze(out);
      } finally {
        ancestors.delete(value);
      }
    }
    throw refuse(path, kindOf2(value));
  }
  var bounded = (value, what, who) => payload(
    value,
    (path, kind) => Error(`${what === "action" ? "UI_ACTION" : "UI_CHANNEL"}: ${who} payload ${path ? `field ${path}` : "value"} is not JSON (${kind})`),
    "",
    0,
    { left: 65536, what, who },
    /* @__PURE__ */ new Map()
  );
  var show2 = (value) => {
    if (typeof value === "number") return String(value);
    try {
      return (JSON.stringify(value) ?? typeof value).slice(0, 40);
    } catch {
      return typeof value;
    }
  };
  function configureActions(metadata) {
    outsideRender("configure actions");
    const names3 = metadata?.__names, modes = metadata?.__modes;
    if (!Array.isArray(names3) || names3.length > 256 || !modes || typeof modes !== "object" || names3.some((name) => typeof name !== "string" || !name || name.length > 256 || !has(modes, name) || !["call", "stream"].includes(modes[name])) || new Set(names3).size !== names3.length)
      throw Error(`UI_ACTION: action catalog takes { __names: 0..256 unique non-empty strings (<= 256 chars), __modes: Record<name, 'call'|'stream'> }; got ${show2(metadata)}`);
    cancelActions();
    const epoch2 = actionEpoch, built = /* @__PURE__ */ Object.create(null);
    const reportLive = (cause) => {
      if (epoch2 === actionEpoch) report(cause);
    };
    for (const name of names3) built[name] = (...args) => {
      outsideRender("action");
      if (epoch2 !== actionEpoch) throw bridgeError("E_ABORT", "Stale page action");
      if (args.length > 1) throw Error("UI_ACTION: expected at most one JSON payload");
      admit("actions");
      const params = args.length && args[0] !== void 0 ? bounded(args[0], "action", name) : void 0;
      const id = `ui-action:${nextRequest++}`;
      const encoded = { kind: "req", id, method: name, params };
      if (modes[name] === "stream") {
        const stream = createStream(id, (cause) => {
          if (!pendingActions.has(id)) return;
          if (requests.length >= 512) throw Error("UI_LIMIT: too many pending cancellations");
          requests.push({ kind: "cancel", id });
          pendingActions.delete(id);
          stream.reject(cause ?? bridgeError("E_ABORT", "Stream canceled"));
        }, reportLive, bridgeError);
        pendingActions.set(id, { ...stream, sequence: -1 });
        requests.push(encoded);
        return stream.handle;
      }
      let settle2;
      const promise = new ActionPromise((resolve, reject) => {
        settle2 = { resolve, reject };
      });
      pendingActions.set(id, {
        resolve: settle2.resolve,
        reject(cause) {
          if (cause instanceof Error && cause.action === void 0) cause.action = name;
          settle2.reject(cause);
          Promise.resolve().then(() => {
            if (!promise.handled && cause?.code !== "E_ABORT") {
              reportLive(`Unhandled rejection of action '${name}': ${cause?.message ?? String(cause)}`);
            }
          });
        }
      });
      Promise.prototype.then.call(promise, void 0, () => {
      });
      requests.push(encoded);
      return promise;
    };
    for (const name of names3) {
      actionModes.set(built[name], modes[name]);
      actionNames.set(built[name], name);
    }
    actions = catalog(built, true);
    channels = Object.freeze({
      open(topic, params) {
        outsideRender("channel");
        if (epoch2 !== actionEpoch) throw bridgeError("E_ABORT", "Stale page channel");
        if (typeof topic !== "string" || !has(built, topic) || topic.includes(".") || modes[topic] !== "call")
          throw Error("UI_CHANNEL: expected a page channel handler");
        admit("channels");
        const id = `ui-channel:${nextRequest++}`;
        const encoded = { kind: "ch.open", id, topic, params: params === void 0 ? void 0 : bounded(params, "channel", topic) };
        let outgoing = 0;
        const channel = createChannel(id, (payload2) => {
          outsideRender("channel send");
          const entry = activeChannels.get(id);
          if (epoch2 !== actionEpoch || !entry?.open) throw bridgeError("STREAM_CLOSED", "Channel is closed");
          if (requests.length >= 256) throw Error("UI_LIMIT: too many pending channel frames");
          requests.push({ kind: "ch.data", id, seq: outgoing, payload: bounded(payload2, "channel", topic) });
          outgoing++;
        }, (failed, cause) => {
          outsideRender("channel close");
          if (!activeChannels.has(id)) return;
          const wire = failed === void 0 ? void 0 : closeError(failed);
          if (requests.length >= 512) throw Error("UI_LIMIT: too many pending cancellations");
          requests.push(wire === void 0 ? { kind: "ch.close", id } : { kind: "ch.close", id, error: wire });
          activeChannels.delete(id);
          if (cause) channel.reject(cause);
          else channel.finish(failed, true);
        }, reportLive, bridgeError);
        const promise = new Promise((resolve, reject) => activeChannels.set(id, { channel, resolve, reject, open: false, sequence: -1 }));
        promise.catch(() => {
        });
        requests.push(encoded);
        return promise;
      }
    });
  }
  function callLeaveHost(method, params) {
    outsideRender("leave control");
    if (!["navigation.setLeaveGuard", "navigation.leave", "navigation.setTitle"].includes(method)) throw Error("UI_LEAVE: invalid control");
    admit("actions");
    const id = `ui-action:${nextRequest++}`;
    const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
    promise.catch(() => {
    });
    requests.push({ kind: "req", id, method: `host.${method}`, params });
    return promise;
  }
  function requestNavigation(to) {
    outsideRender("link");
    admit("actions");
    const id = `ui-action:${nextRequest++}`;
    const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
    promise.catch(report);
    requests.push({ kind: "req", id, method: "nav.go", params: bounded(to, "link") });
  }
  var sentTitle;
  function syncPageTitle(title) {
    const next = typeof title === "string" ? title : null;
    if (next === sentTitle || sentTitle === void 0 && next === null) return;
    sentTitle = next;
    try {
      callLeaveHost("navigation.setTitle", { title: next }).catch(report);
    } catch (error) {
      report(error);
    }
  }
  function requestLeave(reason) {
    outsideRender("leave request");
    leaveGuards.request(reason);
  }
  function takeViewEvents() {
    const result = requests;
    requests = [];
    return result;
  }
  var frameCaps = { req: (m) => m.method.startsWith("host.") ? "host" : "page", "ch.open": () => "page" };
  function takeActionRequests() {
    return takeViewEvents().map((message) => {
      const frame = { v: 2, ...message };
      if (frameCaps[message.kind]) frame.cap = frameCaps[message.kind](message);
      return JSON.stringify(frame);
    });
  }
  var automationResults = [];
  var automationError = (call, error) => automationResults.push({ call, error: {
    code: typeof error?.code === "string" ? error.code : "UI_ACTION",
    message: String(error?.message ?? error)
  } });
  function automationAction(call, name, payload2) {
    const action = typeof name === "string" && has(actions, name) ? actions[name] : void 0;
    if (!action) return automationError(call, { code: "UI_ACTION", message: `no page action ${JSON.stringify(name)}` });
    if (actionModes.get(action) !== "call") return automationError(call, { code: "UI_ACTION", message: `${name} streams; automation calls only call actions` });
    try {
      (payload2 === void 0 ? action() : action(payload2)).then(
        (value) => automationResults.push({ call, value: value === void 0 ? null : value }),
        (error) => automationError(call, error)
      );
    } catch (error) {
      automationError(call, error);
    }
  }
  function takeAutomationResults() {
    const result = automationResults;
    automationResults = [];
    return JSON.stringify(result);
  }
  function takeActionErrors() {
    const result = actionErrors;
    actionErrors = [];
    return result;
  }
  function actionSerial() {
    return nextRequest;
  }
  function actionPending(serial) {
    return pendingActions.has(`ui-action:${serial}`);
  }
  function hasActionEpoch(epoch2) {
    return epoch2 === actionEpoch;
  }
  function actionEpochNow() {
    return actionEpoch;
  }
  function closeError(failed) {
    const { code, message, reason, data } = failed ?? {};
    if (typeof code !== "string" || code.length > 256 || message !== void 0 && (typeof message !== "string" || message.length > 2048) || reason !== void 0 && (typeof reason !== "string" || reason.length > 256))
      throw Error("UI_CHANNEL: close takes { code: string, message?: string, reason?: string, data? }");
    return { code, message: message ?? code, ...reason === void 0 ? {} : { reason }, ...data === void 0 ? {} : { data: bounded(data, "channel", "close") } };
  }
  var failure = ({ error }) => bridgeError(
    typeof error?.code === "string" ? error.code : "E_INTERNAL",
    typeof error?.message === "string" ? error.message : void 0,
    error?.data === void 0 ? void 0 : immutable(error.data),
    typeof error?.reason === "string" ? error.reason : void 0
  );
  function receiveActionResponse(frame) {
    outsideRender("response");
    if (frame?.v === 2 && ["ch.ack", "ch.data", "ch.close"].includes(frame.kind)) return receiveChannelFrame(frame);
    if (frame?.v !== 2 || !["res", "event"].includes(frame.kind) || typeof frame.id !== "string" || frame.kind === "res" && typeof frame.ok !== "boolean" || frame.kind === "event" && (!Number.isSafeInteger(frame.seq) || frame.seq < 0))
      throw Error(`UI_ACTION: response takes { v: 2, kind: 'res'|'event', id: string, ok: boolean | seq: integer >= 0 }; got ${show2(frame)}`);
    const pending3 = pendingActions.get(frame.id);
    if (!pending3) return false;
    if (frame.kind === "event") {
      if (!pending3.push) throw Error("UI_STREAM: event for unary action");
      if (frame.seq <= pending3.sequence || pending3.sequence === -1 && frame.seq !== 0)
        throw Error("UI_STREAM: invalid sequence");
      const value2 = immutable(frame.payload ?? null);
      pending3.sequence = frame.seq;
      pending3.push(value2);
      return true;
    }
    const value = frame.ok ? immutable(frame.result ?? null) : failure(frame);
    pendingActions.delete(frame.id);
    if (frame.ok) pending3.resolve(value);
    else pending3.reject(value);
    snapActionSettled(Number(frame.id.slice("ui-action:".length)), frame.ok);
    return true;
  }
  function receiveChannelFrame(frame) {
    if (typeof frame.id !== "string" || frame.kind === "ch.ack" && typeof frame.ok !== "boolean" || frame.kind === "ch.data" && (!Number.isSafeInteger(frame.seq) || frame.seq < 0) || frame.kind === "ch.close" && frame.error !== void 0 && typeof frame.error?.code !== "string")
      throw Error(`UI_CHANNEL: frame takes { v: 2, kind: 'ch.ack'|'ch.data'|'ch.close', id: string, ok?: boolean, seq?: integer >= 0, error?: { code: string, message?: string, reason?: string, data? } }; got ${show2(frame)}`);
    const entry = activeChannels.get(frame.id);
    if (!entry) return false;
    if (frame.kind === "ch.ack") {
      if (entry.open) throw Error("UI_CHANNEL: duplicate acknowledgement");
      if (frame.ok) {
        entry.open = true;
        entry.resolve(entry.channel.handle);
      } else {
        const cause = failure(frame);
        activeChannels.delete(frame.id);
        entry.reject(cause);
        entry.channel.reject(cause);
      }
      return true;
    }
    if (!entry.open) throw Error("UI_CHANNEL: frame before acknowledgement");
    if (frame.kind === "ch.close") {
      activeChannels.delete(frame.id);
      entry.channel.finish(frame.error === void 0 ? void 0 : failure(frame));
    } else {
      if (frame.seq <= entry.sequence || entry.sequence === -1 && frame.seq !== 0)
        throw Error("UI_CHANNEL: invalid sequence");
      const value = immutable(frame.payload ?? null);
      entry.sequence = frame.seq;
      entry.channel.push(value);
    }
    return true;
  }

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
    const fail = (error) => {
      stream = void 0;
      update({ status: "failed", error: resourceError(error) });
    };
    return {
      get state() {
        return snapshot;
      },
      cancel,
      start(...params) {
        if (!mounted) return;
        cancel();
        const current2 = ++ticket;
        update({ status: "streaming", error: null, result: null, data: null });
        try {
          if (!definition.action) throw Error("UI_STREAM: bridge not ready");
          stream = definition.action(...params);
          stream.on("data", (value) => {
            if (!mounted || current2 !== ticket) return;
            update({ data: value ?? null });
            try {
              definition.onData?.(value);
            } catch (error) {
              cancel();
              fail(error);
            }
          });
          stream.result.then((value) => {
            if (!mounted || current2 !== ticket) return;
            stream = void 0;
            update({ status: "completed", result: value ?? null });
          }, (error) => {
            if (!mounted || current2 !== ticket) return;
            if (error.code === "E_ABORT") {
              stream = void 0;
              update({ status: "canceled", error: resourceError(error) });
            } else fail(error);
          });
        } catch (error) {
          fail(error);
        }
      },
      commit(next) {
        if (definition && definition.action !== next.action) {
          cancel();
          update({ status: "idle", error: null, result: null, data: null });
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
      const current2 = ++ticket;
      update({ pending: true, stale: snapshot.value !== null, error: null });
      try {
        const value = await definition.action(...definition.params === void 0 ? [] : [definition.params]);
        if (mounted && current2 === ticket) update({ pending: false, stale: false, value, error: null });
      } catch (error) {
        if (mounted && current2 === ticket) update({ pending: false, stale: snapshot.value !== null, error: resourceError(error) });
      }
    };
    return {
      get state() {
        return snapshot;
      },
      reload,
      commit(next) {
        const changed2 = !definition || definition.action !== next.action || definition.key !== next.key;
        definition = next;
        mounted = true;
        if (changed2) {
          ticket++;
          if (next.action) void reload();
          else update({ pending: false, stale: false, value: null, error: null });
        }
      },
      dispose() {
        mounted = false;
        ticket++;
      }
    };
  }
  function actionResource(state) {
    let definition, mounted = false, ticket = 0, snapshot = state.value;
    const update = (patch) => {
      snapshot = { ...snapshot, ...patch };
      state.set(snapshot);
    };
    return {
      get state() {
        return snapshot;
      },
      async run(...params) {
        if (params.length > 1) throw Error("UI_ACTION: expected at most one JSON payload");
        if (!mounted || !definition.action) return Object.freeze({ ok: false, error: Object.freeze({ code: "UI_ACTION", message: "the component is no longer mounted" }) });
        const current2 = ++ticket;
        update({ pending: true, error: null });
        try {
          const value = await definition.action(...params);
          if (mounted && current2 === ticket) update({ pending: false });
          return Object.freeze({ ok: true, value });
        } catch (error) {
          const failure2 = resourceError(error);
          if (mounted && current2 === ticket) update({ pending: false, error: failure2 });
          return Object.freeze({ ok: false, error: failure2 });
        }
      },
      commit(next) {
        definition = next;
        mounted = true;
      },
      dispose() {
        mounted = false;
        ticket++;
      }
    };
  }

  // src/runtime/channel-resource.js
  function channelResource(state, report2) {
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
      if (!mounted || !definition.topic || snapshot.connecting || snapshot.connected) return;
      const current2 = ++ticket;
      update({ connecting: true, connected: false, error: null });
      try {
        const opened2 = await definition.channels.open(definition.topic, definition.params);
        if (!mounted || current2 !== ticket) {
          opened2.close();
          return;
        }
        channel = opened2;
        update({ connecting: false, connected: true });
        if (definition.onData) opened2.on("data", (value) => {
          if (mounted && current2 === ticket) definition.onData(value);
        });
        opened2.on("error", (error) => {
          if (mounted && current2 === ticket) update({ error: resourceError(error) });
        });
        opened2.on("close", ({ error }) => {
          if (mounted && current2 === ticket) {
            channel = void 0;
            update({ connecting: false, connected: false, ...error ? { error: resourceError(error) } : {} });
          }
        });
      } catch (error) {
        if (mounted && current2 === ticket) {
          update({ connecting: false, connected: false, error: resourceError(error) });
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
        const changed2 = !definition || definition.topic !== next.topic || definition.key !== next.key || definition.channels !== next.channels;
        definition = next;
        if (changed2) {
          try {
            close();
          } catch (error) {
            report2(error);
          }
          mounted = true;
          connect().catch(report2);
        }
      },
      dispose() {
        mounted = false;
        try {
          close();
        } catch (error) {
          report2(error);
        }
      }
    };
  }

  // src/runtime/hooks.js
  var SIZE_CLASS_CONTEXT_ID = "@lingxia/view:SizeClassScope";
  var SizeClassContext = createContext(null);
  Object.defineProperty(SizeClassContext, "__lxid", { value: SIZE_CLASS_CONTEXT_ID });
  function SizeClassScope({ value, children }) {
    if (value !== "compact" && value !== "regular")
      throw Error('UI_PROP: SizeClassScope value must be "compact" or "regular"');
    return h(SizeClassContext.Provider, { value }, children);
  }
  function scopedSizeClass(contexts = render.contexts, track = true) {
    const entry = lookup(contexts, SIZE_CLASS_CONTEXT_ID);
    if (!entry) return void 0;
    if (track) noteContext(SIZE_CLASS_CONTEXT_ID, SizeClassContext, entry.value);
    return entry.value;
  }
  var bindings = /* @__PURE__ */ new WeakMap();
  function cellOf(binding) {
    return bindings.get(binding);
  }
  function local(name, initial, resource = "", order) {
    if (!render.active || render.scope === null || order === void 0 && (typeof name !== "string" || !name || name.length > 256))
      throw Error("UI_SCOPE: invalid local state");
    const { candidate, scope } = render;
    const key = JSON.stringify(order !== void 0 ? [scope, order] : resource ? [scope, resource, name] : [scope, name]);
    if (!resource && candidate.declarations.has(key)) throw Error(`UI_LOCAL: duplicate declaration ${JSON.stringify(name)} in ${scope}`);
    candidate.declarations.add(key);
    const cell = candidate.locals.get(key) ?? render.lookupLocals?.get(key) ?? page.locals.get(key) ?? { value: immutable(initial, 0, true) };
    candidate.locals.set(key, cell);
    for (const collector of render.collectors) collector.set(key, cell);
    if (cell.binding) return cell.binding;
    const binding = cell.binding = Object.freeze({
      get value() {
        if (render.active) noteRead(cell);
        return page.updates.has(cell) ? page.updates.get(cell) : cell.value;
      },
      set(value) {
        outsideRender("state mutation");
        if (page.locals.get(key) !== cell && render.candidate?.locals.get(key) !== cell)
          throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
        if (typeof value === "function") value = value(page.updates.has(cell) ? page.updates.get(cell) : cell.value);
        page.updates.set(cell, immutable(value, 0, true));
      },
      focus() {
        outsideRender("focus");
        if (page.locals.get(key) !== cell) throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
        cell.focus = (cell.focus ?? 0) + 1;
        page.updates.set(cell, page.updates.has(cell) ? page.updates.get(cell) : cell.value);
      },
      showPicker() {
        outsideRender("showPicker");
        if (page.locals.get(key) !== cell) throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
        cell.picker = (cell.picker ?? 0) + 1;
        page.updates.set(cell, page.updates.has(cell) ? page.updates.get(cell) : cell.value);
      },
      setSelection(start, end) {
        outsideRender("setSelection");
        const value = editorValue(key, cell, name, scope, "setSelection");
        selectionRange(value, start, end, "setSelection");
        cell.selection = [(cell.selection?.[0] ?? 0) + 1, start, end];
        page.updates.set(cell, value);
      },
      replaceRange(start, end, text) {
        outsideRender("replaceRange");
        const value = editorValue(key, cell, name, scope, "replaceRange");
        selectionRange(value, start, end, "replaceRange");
        if (typeof text !== "string") throw Error("UI_LOCAL: replaceRange takes (start, end, text) with text a string");
        const at = start + text.length;
        cell.selection = [(cell.selection?.[0] ?? 0) + 1, at, at];
        page.updates.set(cell, immutable(value.slice(0, start) + text + value.slice(end), 0, true));
      }
    });
    bindings.set(binding, { key, cell });
    return binding;
  }
  function editorValue(key, cell, name, scope, call) {
    if (page.locals.get(key) !== cell) throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
    const value = page.updates.has(cell) ? page.updates.get(cell) : cell.value;
    if (typeof value !== "string") throw Error(`UI_LOCAL: ${call} belongs to a text cell bound to an editor`);
    return value;
  }
  function selectionRange(value, start, end, call) {
    const splits = (at) => at > 0 && at < value.length && /[\uD800-\uDBFF]/.test(value[at - 1]) && /[\uDC00-\uDFFF]/.test(value[at]);
    if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || start > end || end > value.length || splits(start) || splits(end))
      throw Error(`UI_LOCAL: ${call} takes UTF-16 offsets 0 <= start <= end <= ${value.length} that split no character; got ${start}, ${end}`);
  }
  function useLocal(...args) {
    var _a;
    if (args.length === 2) return local(args[0], args[1]);
    if (args.length !== 1) throw Error("UI_LOCAL: useLocal takes (initial) or (name, initial)");
    if (!render.active || render.scope === null) throw Error("UI_SCOPE: invalid local state");
    const counts = (_a = render.candidate).localOrder ?? (_a.localOrder = /* @__PURE__ */ new Map());
    const order = counts.get(render.scope) ?? 0;
    counts.set(render.scope, order + 1);
    return local(`#${order}`, args[0], "", order);
  }
  function hashId(text) {
    let a = 2166136261, b = 2538058380;
    for (let i = 0; i < text.length; i++) {
      const c = text.charCodeAt(i);
      a = Math.imul(a ^ c, 16777619) >>> 0;
      b = Math.imul(b ^ c, 16777619) >>> 0;
    }
    return a.toString(36) + b.toString(36);
  }
  function useId(site) {
    var _a;
    if (!render.active || render.scope === null) throw Error("UI_SCOPE: useId outside render");
    const counts = (_a = render.candidate).idOrder ?? (_a.idOrder = /* @__PURE__ */ new Map());
    const base = `${render.scope}\0${site ?? "#"}`;
    const seen = counts.get(base) ?? 0;
    counts.set(base, seen + 1);
    return `lx-${hashId(seen ? `${base}\0${seen}` : base)}`;
  }
  function hasLocalUpdates() {
    return page.updates.size > 0 || page.invalidated === true;
  }
  function useView() {
    if (!render.active) throw Error("UI_SCOPE: useView outside render");
    render.readsView = true;
    noteView();
    const scoped = scopedEnv();
    if (!scoped) return render.view;
    const scopedView = Object.freeze({ ...render.view, env: scoped.env });
    rawEnvs.set(scopedView, scoped.base);
    return scopedView;
  }
  function useEnv() {
    if (!render.active) throw Error("UI_SCOPE: useEnv outside render");
    return fieldTracked(scopedEnv()?.env ?? render.view.env);
  }
  var tracked = /* @__PURE__ */ new WeakMap();
  function fieldTracked(env) {
    if (!env || typeof env !== "object") return env;
    let proxy = tracked.get(env);
    if (!proxy) tracked.set(env, proxy = new Proxy(env, {
      get(target, key, receiver) {
        if (typeof key === "string") noteEnv(key);
        return Reflect.get(target, key, receiver);
      },
      has(target, key) {
        if (typeof key === "string") noteEnv(key);
        return Reflect.has(target, key);
      },
      ownKeys(target) {
        noteEnv("*");
        return Reflect.ownKeys(target);
      }
    }));
    return proxy;
  }
  function scopedEnv() {
    const scoped = scopedSizeClass();
    const raw = envOf(render.view);
    if (!scoped || !raw || raw.sizeClass === scoped) return void 0;
    const base = Object.freeze({ ...raw, sizeClass: scoped });
    const env = rawEnvs.has(render.view) ? new Proxy(base, {
      get(target, key, receiver) {
        if (key !== "sizeClass" && hostContextKeys.has(key)) hostContextRead = true;
        if (appearanceKeys.has(key)) appearanceRead = true;
        return Reflect.get(target, key, receiver);
      }
    }) : base;
    return { env, base };
  }
  function pageAction(action, code, mode) {
    if (action === void 0 && !Object.keys(render.view.actions).length) return void 0;
    const info = typeof action === "function" ? actionInfo(action) : void 0;
    if (!info || info.mode !== mode || info.name.includes("."))
      throw Error(`${code}: expected a ${mode === "stream" ? "streaming" : "unary"} action from view.actions`);
    return { action, name: info.name };
  }
  function own(name, initial, kind, code, create, definition) {
    const binding = local(name, initial, kind);
    const { key, cell } = bindings.get(binding);
    const state = { get value() {
      return binding.value;
    }, set(value) {
      if (page.locals.get(key) === cell || render.candidate?.locals.get(key) === cell) binding.set(value);
    } };
    const { candidate } = render;
    if (candidate.resourceNames.has(cell)) throw Error(`${code}: duplicate resource name`);
    candidate.resourceNames.add(cell);
    const resource = page.resources.get(cell) ?? create(state);
    candidate.resources.set(cell, { resource, definition });
    return resource;
  }
  var guard = (what, fn) => (...args) => {
    outsideRender(what);
    return fn(...args);
  };
  function useRequest(name, action, ...params) {
    if (params.length > 1) throw Error("UI_REQUEST: expected at most one JSON payload");
    const target = pageAction(action, "UI_REQUEST", "call");
    const payload2 = params[0] === void 0 ? void 0 : immutable(params[0]);
    const resource = own(
      name,
      { pending: false, stale: false, value: null, error: null },
      "request",
      "UI_REQUEST",
      requestResource,
      { action: target?.action, params: payload2, key: JSON.stringify(payload2) }
    );
    return Object.freeze({
      get pending() {
        return resource.state.pending;
      },
      get stale() {
        return resource.state.stale;
      },
      get value() {
        return resource.state.value;
      },
      get error() {
        return resource.state.error;
      },
      reload: guard("request reload", resource.reload)
    });
  }
  function useAction(...args) {
    var _a;
    if (args.length < 1 || args.length > 2) throw Error("UI_ACTION: useAction takes (action) or (name, action)");
    if (!render.active || render.scope === null) throw Error("UI_SCOPE: useAction outside render");
    let name = args[0];
    if (args.length === 1) {
      const counts = (_a = render.candidate).actionOrder ?? (_a.actionOrder = /* @__PURE__ */ new Map());
      const order = counts.get(render.scope) ?? 0;
      counts.set(render.scope, order + 1);
      name = `#${order}`;
    }
    const target = pageAction(args[args.length - 1], "UI_ACTION", "call");
    const resource = own(name, { pending: false, error: null }, "action", "UI_ACTION", actionResource, { action: target?.action });
    return Object.freeze({
      get pending() {
        return resource.state.pending;
      },
      get error() {
        return resource.state.error;
      },
      run: guard("action run", resource.run)
    });
  }
  function useStream(name, action, options2 = {}) {
    if (!options2 || typeof options2 !== "object" || options2.onData !== void 0 && typeof options2.onData !== "function")
      throw Error("UI_STREAM: onData must be a function");
    const target = pageAction(action, "UI_STREAM", "stream");
    const resource = own(
      name,
      { status: "idle", error: null, result: null, data: null },
      "stream",
      "UI_STREAM",
      streamResource,
      { action: target?.action, onData: options2.onData }
    );
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
      get data() {
        return resource.state.data;
      },
      start: guard("stream mutation", resource.start),
      cancel: guard("stream mutation", resource.cancel)
    });
  }
  function useChannel(name, action, options2) {
    if (!options2 || typeof options2 !== "object" || !("params" in options2) || options2.onData !== void 0 && typeof options2.onData !== "function")
      throw Error("UI_CHANNEL: expected { params, onData? }");
    const target = pageAction(action, "UI_CHANNEL", "call");
    const params = options2.params === void 0 ? void 0 : immutable(options2.params);
    const resource = own(
      name,
      { connecting: false, connected: false, error: null },
      "channel",
      "UI_CHANNEL",
      (state) => channelResource(state, report),
      { topic: target?.name, params, key: JSON.stringify(params), onData: options2.onData, channels: render.view.channels }
    );
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
      connect: guard("channel resource mutation", resource.connect),
      close: guard("channel resource mutation", resource.close),
      send: guard("channel resource mutation", resource.send)
    });
  }
  function useLeaveGuard(name, options2) {
    if (typeof options2?.enabled !== "boolean") throw Error("UI_LEAVE: expected { enabled: boolean }");
    const resource = own(name, { pending: null, requests: 0, error: null }, "leave", "UI_LEAVE", leaveGuards.resource, { enabled: options2.enabled });
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
      answer: guard("leave mutation", resource.answer),
      request: guard("leave mutation", resource.request)
    });
  }
  function useBackHandler(enabled, onBack) {
    var _a;
    if (!render.active || render.scope === null) throw Error("UI_SCOPE: useBackHandler outside render");
    if (typeof enabled !== "boolean") throw Error("UI_BACK: enabled must be a boolean");
    if (typeof onBack !== "function") throw Error("UI_BACK: onBack must be a function");
    const counts = (_a = render.candidate).backOrder ?? (_a.backOrder = /* @__PURE__ */ new Map());
    const order = counts.get(render.scope) ?? 0;
    counts.set(render.scope, order + 1);
    own(`#${order}`, null, "back", "UI_BACK", leaveGuards.backHandler, { enabled, onBack });
  }
  var hostContextKeys = /* @__PURE__ */ new Set(["sizeClass", "canDock"]);
  var hostContextRead = false;
  var appearanceKeys = /* @__PURE__ */ new Set(["appearance", "highContrast"]);
  var appearanceRead = false;
  var rawEnvs = /* @__PURE__ */ new WeakMap();
  var lastData;
  var lastRevision = null;
  function syncData(from, revision, ops) {
    if (lastData === void 0 || lastRevision !== from) return false;
    try {
      lastData = applyPatch(lastData, ops);
    } catch {
      lastRevision = null;
      return false;
    }
    lastRevision = revision;
    return true;
  }
  function withLogicalEdges(edges, rtl) {
    if (!edges || typeof edges !== "object") return edges;
    const inlineStart = rtl ? edges.right : edges.left;
    const inlineEnd = rtl ? edges.left : edges.right;
    if (edges.inlineStart === inlineStart && edges.inlineEnd === inlineEnd) return edges;
    return Object.freeze({
      ...edges,
      inlineStart,
      inlineEnd
    });
  }
  function normalizeEnv(env) {
    if (!env || typeof env !== "object") return env;
    const rtl = env.direction === "rtl";
    const insets = env.insets ? withLogicalEdges(env.insets, rtl) : void 0;
    const safeArea = env.safeArea ? withLogicalEdges(env.safeArea, rtl) : void 0;
    if (insets === env.insets && safeArea === env.safeArea && env.hinge !== void 0) return env;
    return {
      ...env,
      ...insets ? { insets } : {},
      ...safeArea ? { safeArea } : {},
      ...env.hinge === void 0 ? { hinge: null } : {}
    };
  }
  function ensureNormalizedView(view) {
    if (!view || typeof view !== "object" || rawEnvs.has(view) || !view.env || typeof view.env !== "object") return view;
    const env = normalizeEnv(view.env);
    return env === view.env ? view : { ...view, env };
  }
  function createView(data, env, revision) {
    const { actions: actions2, channels: channels2 } = currentActions();
    hostContextRead = false;
    const withCapsule = env && typeof env === "object" && !Object.hasOwn(env, "capsule") ? { ...env, capsule: null } : env;
    const frozen2 = immutable(normalizeEnv(withCapsule));
    const watched = frozen2 && typeof frozen2 === "object" ? new Proxy(frozen2, {
      get(target, key, receiver) {
        if (hostContextKeys.has(key)) hostContextRead = true;
        if (appearanceKeys.has(key)) appearanceRead = true;
        return Reflect.get(target, key, receiver);
      }
    }) : frozen2;
    const view = Object.freeze({ data: data === void 0 ? lastData : shared(data, lastData), env: watched, actions: actions2, channels: channels2 });
    if (data !== void 0) lastRevision = typeof revision === "number" ? revision : null;
    lastData = view.data;
    rawEnvs.set(view, frozen2);
    return view;
  }
  var envOf = (view) => (view && rawEnvs.get(view)) ?? view?.env;
  var readsHostContext = () => hostContextRead;
  var envReads = () => (hostContextRead || classesReadSizeClass() ? 1 : 0) | (appearanceRead || classesReadTheme() ? 2 : 0) | (clockStep() === 1e3 ? 4 : clockStep() ? 8 : 0);
  function resetEnvReads() {
    appearanceRead = false;
  }

  // src/runtime/vector-path.js
  var arity = { m: 2, l: 2, h: 1, v: 1, c: 6, s: 4, q: 4, t: 2, a: 7, z: 0 };
  function pathSegments(d) {
    let count = 0;
    for (const [, command, args] of d.matchAll(/([MmLlHhVvCcSsQqTtAaZz])([^MmLlHhVvCcSsQqTtAaZz]*)/g)) {
      const per = arity[command.toLowerCase()];
      const numbers = args.match(/[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g)?.length ?? 0;
      count += per ? Math.max(1, Math.floor(numbers / per)) : 1;
    }
    return count;
  }
  function pathSegmentsProblem(segments) {
    if (segments <= limits.vectorPathSegments) return void 0;
    return `UI_VECTOR: a path has ${segments} segments; the limit is ${limits.vectorPathSegments} per path: split it into several paths`;
  }

  // src/runtime/validate.js
  function label(value, code, required = false) {
    if (value === void 0 && !required) return;
    if (typeof value !== "string" || !value.trim() || utf8Bytes(value, 1025) > 1024 || value.includes("\0"))
      throw Error(`${code}: ${required ? "aria-label is required" : "invalid aria-label"}`);
  }
  var finiteWithin = (value, bound) => typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= bound;
  var hexColor = (value) => typeof value === "string" && /^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(value);
  var pathKeys = ["key", "d", "fill", "stroke", "strokeWidth", "strokeDasharray", "strokeDashoffset", "strokeLinecap", "strokeLinejoin", "transform", "vectorEffect", "pathLength"];
  var gradientKeys = ["type", "x1", "y1", "x2", "y2", "cx", "cy", "r", "fx", "fy", "fr", "from", "units", "spread", "stops"];
  var plain2 = (value) => value && typeof value === "object" && !Array.isArray(value);
  var show3 = (value) => {
    if (typeof value === "number") return String(value);
    try {
      return (JSON.stringify(value) ?? typeof value).slice(0, 40);
    } catch {
      return typeof value;
    }
  };
  function vectorGradient(id, gradient) {
    const invalid2 = () => {
      throw Error(`UI_VECTOR: invalid gradient ${id}`);
    };
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(id) || !plain2(gradient) || Object.keys(gradient).some((k) => !gradientKeys.includes(k)) || !["linear", "radial", "conic"].includes(gradient.type) || { linear: ["cx", "cy", "r", "fx", "fy", "fr", "from"], radial: ["x1", "y1", "x2", "y2", "from"], conic: ["x1", "y1", "x2", "y2", "r", "fx", "fy", "fr"] }[gradient.type].some((k) => gradient[k] !== void 0) || gradient.from !== void 0 && !finiteWithin(gradient.from, 36e3) || ["x1", "y1", "x2", "y2", "cx", "cy", "fx", "fy"].some((k) => gradient[k] !== void 0 && !finiteWithin(gradient[k], 16384)) || ["r", "fr"].some((k) => gradient[k] !== void 0 && (!finiteWithin(gradient[k], 16384) || gradient[k] < 0)) || gradient.units !== void 0 && !["objectBoundingBox", "userSpaceOnUse"].includes(gradient.units) || gradient.spread !== void 0 && !["pad", "reflect", "repeat"].includes(gradient.spread) || !Array.isArray(gradient.stops) || !gradient.stops.length || gradient.stops.length > 16 || gradient.stops.some((stop) => !plain2(stop) || Object.keys(stop).some((k) => !["offset", "color", "opacity"].includes(k)) || !finiteWithin(stop.offset, 1) || stop.offset < 0 || !(hexColor(stop.color) || isColorName(stop.color)) || stop.opacity !== void 0 && (!finiteWithin(stop.opacity, 1) || stop.opacity < 0))) invalid2();
  }
  function vector(props, fit) {
    const box = props.viewBox ?? [0, 0, 24, 24];
    if (!Array.isArray(box) || box.length !== 4 || box.some((v) => !finiteWithin(v, 16384)) || box[2] < 1 / 64 || box[3] < 1 / 64)
      throw Error(`UI_VECTOR: viewBox takes [x, y, width, height] with finite numbers within \xB116384 and width/height >= 1/64; got ${show3(box)}`);
    if (!Array.isArray(props.paths) || !props.paths.length || props.paths.length > 32)
      throw Error(`UI_VECTOR: paths takes 1..32 path objects; got ${show3(props.paths)}`);
    const gradients = props.gradients ?? {};
    if (!plain2(gradients) || Object.keys(gradients).length > 8) throw Error("UI_VECTOR: at most 8 gradients");
    for (const [id, gradient] of Object.entries(gradients)) vectorGradient(id, gradient);
    const keys2 = /* @__PURE__ */ new Set();
    for (const path of props.paths) {
      if (!plain2(path) || Object.keys(path).some((k) => !pathKeys.includes(k)) || typeof path.d !== "string" || !path.d.length || path.d.length > limits.vectorPathBytes)
        throw Error(`UI_VECTOR: path takes an object with non-empty d (<= ${limits.vectorPathBytes} chars) and valid path props; got ${show3(path)}`);
      const segments = path.d.length > limits.vectorPathSegments && pathSegmentsProblem(pathSegments(path.d));
      if (segments) throw Error(segments);
      for (const key of ["fill", "stroke"]) {
        const value = path[key];
        if (value === void 0 || value === "none" || value === "currentColor" || hexColor(value) || isColorName(value)) continue;
        const id = typeof value === "string" && value.match(/^url\(#(.+)\)$/)?.[1];
        if (!id) throw Error(`UI_VECTOR: ${key} takes 'none', 'currentColor', a hex color, a theme role or url(#id); got ${show3(value)}`);
        if (!has(gradients, id)) throw Error(`UI_VECTOR: unknown gradient ${id}`);
      }
      if (path.vectorEffect !== void 0 && !["none", "non-scaling-stroke"].includes(path.vectorEffect)) throw Error("UI_VECTOR: invalid vector effect");
      if (path.key !== void 0 && (typeof path.key !== "string" || !path.key || path.key.length > 64)) throw Error("UI_VECTOR: a path key is 1..64 characters");
      if (path.key !== void 0 && keys2.has(path.key)) throw Error("UI_VECTOR: path keys are unique");
      if (path.key !== void 0) keys2.add(path.key);
      if (path.pathLength !== void 0 && (!finiteWithin(path.pathLength, 16384) || path.pathLength <= 0)) throw Error("UI_VECTOR: invalid pathLength");
      if (path.strokeLinecap !== void 0 && !["butt", "round", "square"].includes(path.strokeLinecap)) throw Error("UI_VECTOR: invalid stroke-linecap");
      if (path.strokeLinejoin !== void 0 && !["miter", "round", "bevel"].includes(path.strokeLinejoin)) throw Error("UI_VECTOR: invalid stroke-linejoin");
      if (path.strokeDashoffset !== void 0 && !finiteWithin(path.strokeDashoffset, 16384)) throw Error("UI_VECTOR: invalid dash offset");
      if (path.transform !== void 0 && (typeof path.transform !== "string" || path.transform.length > 512 || !/^\s*(?:(?:matrix|translate|scale|rotate|skewX|skewY)\s*\([^()]*\)[\s,]*)+$/.test(path.transform)))
        throw Error(`UI_VECTOR: transform takes SVG transform functions (matrix/translate/scale/rotate/skewX/skewY) up to 512 chars; got ${show3(path.transform)}`);
      if (path.strokeDasharray !== void 0 && (!Array.isArray(path.strokeDasharray) || path.strokeDasharray.length > 16 || path.strokeDasharray.some((v) => typeof v !== "number" || !Number.isFinite(v) || v < 1 / 64 || v > 1024)))
        throw Error(`UI_VECTOR: strokeDasharray takes up to 16 numbers from 1/64 to 1024; got ${show3(path.strokeDasharray)}`);
      if (path.strokeWidth !== void 0 && (typeof path.strokeWidth !== "number" || !Number.isFinite(path.strokeWidth) || path.strokeWidth < 0 || path.strokeWidth > 1024))
        throw Error(`UI_VECTOR: strokeWidth takes a number from 0 to 1024; got ${show3(path.strokeWidth)}`);
    }
    const source3 = JSON.stringify({ viewBox: box, paths: props.paths, ...Object.keys(gradients).length ? { gradients } : {}, ...fit === void 0 ? {} : { fit } });
    if (source3.length > limits.vectorSourceBytes) throw Error("UI_VECTOR: source too large");
    return source3;
  }
  function plot(props) {
    if (["smooth", "area"].some((name) => props[name] !== void 0 && typeof props[name] !== "boolean") || props.kind !== "line" && (props.smooth || props.area))
      throw Error("UI_PLOT: smooth and area must be booleans on line plots");
    label(props.accessibilityLabel, "UI_PLOT", true);
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
    if (props.onSelect !== void 0 && typeof props.onSelect !== "function") throw Error("UI_PLOT: onSelect must be a function");
    if (props.selectedIndex !== void 0 && (!Number.isInteger(props.selectedIndex) || props.selectedIndex < -1 || props.selectedIndex >= props.values.length))
      throw Error("UI_PLOT: invalid selected index");
    if (props.colors !== void 0) {
      if (!Array.isArray(props.colors) || !props.colors.length || props.colors.length > 16) throw Error("UI_PLOT: expected 1..16 colors");
      for (const color of props.colors) if (!(hexColor(color) && color.length !== 4 && color.length !== 5) && seriesProblem(color)) throw Error(seriesProblem(color));
    }
  }
  function sliderValue({ min, max, step }, value) {
    value = Math.max(min, Math.min(max, value));
    const snapped = Math.max(min, Math.min(max, min + Math.floor((value - min) / step + 0.5) * step));
    return Math.abs(max - value) < Math.abs(snapped - value) ? max : snapped;
  }
  function slider(props) {
    for (const [name, fallback] of [["min", 0], ["max", 100], ["step", 1]]) if (props[name] === void 0) props[name] = fallback;
    const { min, max, step, value } = props;
    if (![min, max, step, ...value === void 0 ? [] : [value]].every((v) => typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= 1e9) || max <= min || step <= 0 || step > max - min || (max - min) / step > 1e6 || min + step === min || max - step === max)
      throw Error(`UI_SLIDER: range takes finite min < max within \xB11e9 and step > 0 (<= max - min, at most 1e6 steps); got min=${show3(min)}, max=${show3(max)}, step=${show3(step)}${value === void 0 ? "" : `, value=${show3(value)}`}`);
    label(props.accessibilityLabel, "UI_SLIDER", true);
    if (value !== void 0) props.value = sliderValue(props, value);
  }
  function sliderDefault(props) {
    const value = props.defaultValue;
    if (typeof value !== "number" || !Number.isFinite(value) || Math.abs(value) > 1e9) throw Error("UI_SLIDER: defaultValue must be a finite number");
    return sliderValue(props, value);
  }
  function sliderSecondary({ min, max, secondaryValue: value }) {
    if (typeof value !== "number" || !Number.isFinite(value)) throw Error("UI_SLIDER: secondaryValue must be a finite number");
    return Math.max(min, Math.min(max, value));
  }
  function image(props) {
    if (typeof props.src !== "string" || !props.src.length || props.src.length > limits.sourceBytes || props.src.includes("\0")) throw Error("UI_IMAGE: invalid source");
    if (props.alt !== void 0 && (typeof props.alt !== "string" || utf8Bytes(props.alt, 1024) > 1024 || props.alt.includes("\0") || props.alt.length > 0 && !props.alt.trim())) throw Error("UI_IMAGE: invalid alternative text");
    if (props.retryKey !== void 0 && (typeof props.retryKey !== "string" || utf8Bytes(props.retryKey, 257) > 256 || props.retryKey.includes("\0")))
      throw Error("UI_IMAGE: invalid retry key");
    if (props.placeholder !== void 0 && (typeof props.placeholder !== "string" || !props.placeholder.length || props.placeholder.length > limits.sourceBytes || props.placeholder.includes("\0")))
      throw Error("UI_IMAGE: invalid placeholder source");
    for (const name of ["onLoad", "onError"]) if (props[name] !== void 0 && typeof props[name] !== "function") throw Error("UI_IMAGE: invalid callback");
  }
  var source = (value) => typeof value === "string" && value.length > 0 && value.length <= limits.sourceBytes && !value.includes("\0");
  function video(props) {
    if (props.src !== void 0 && !source(props.src)) throw Error("UI_VIDEO: invalid source");
    if (props.poster !== void 0 && !source(props.poster)) throw Error("UI_VIDEO: invalid poster");
    if (props.sources !== void 0) {
      const labels = /* @__PURE__ */ new Set();
      if (!Array.isArray(props.sources) || props.sources.length < 1 || props.sources.length > 8) throw Error("UI_VIDEO: sources takes 1..8 { src, label } entries");
      for (const entry of props.sources) {
        if (!entry || !source(entry.src)) throw Error("UI_VIDEO: each of sources needs a valid src");
        if (typeof entry.label !== "string" || !/^[^\s].{0,31}$/.test(entry.label) || entry.label === "auto" || labels.has(entry.label))
          throw Error('UI_VIDEO: each of sources needs a unique label of 1..32 characters other than "auto"');
        labels.add(entry.label);
      }
    }
    for (const name of ["autoPlay", "paused", "muted", "loop"]) if (props[name] !== void 0 && typeof props[name] !== "boolean") throw Error(`UI_VIDEO: ${name} takes a boolean`);
    if (props.playbackRate !== void 0 && !(typeof props.playbackRate === "number" && props.playbackRate >= 0.25 && props.playbackRate <= 4)) throw Error("UI_VIDEO: playbackRate takes a number in 0.25..4");
    if (props.volume !== void 0 && !(typeof props.volume === "number" && props.volume >= 0 && props.volume <= 1)) throw Error("UI_VIDEO: volume takes a number in 0..1");
    for (const [name, value] of Object.entries(props)) if (/^on[A-Z]/.test(name) && value !== void 0 && typeof value !== "function") throw Error(`UI_VIDEO: ${name} takes a function`);
  }
  function webview(props) {
    if (props.src !== void 0 && !source(props.src)) throw Error("UI_WEBVIEW: invalid source");
    label(props.accessibilityLabel, "UI_WEBVIEW", true);
    if (props.handlesDrags !== void 0 && typeof props.handlesDrags !== "boolean") throw Error("UI_WEBVIEW: handlesDrags takes a boolean");
    for (const name of ["onFullscreenChange", "onLoad", "onMessage", "onBlocked", "onError"])
      if (props[name] !== void 0 && typeof props[name] !== "function") throw Error(`UI_WEBVIEW: ${name} takes a function`);
  }
  var SHORT_KEY = Math.floor((limits.identifierBytes - 6) / 6);
  var SURROGATE = /[\ud800-\udfff]/;
  function checkKey(key, seen) {
    if (typeof key !== "string" || seen.has(key) || (key.length > SHORT_KEY || SURROGATE.test(key)) && (utf8Bytes(key, limits.identifierBytes) > limits.identifierBytes || utf8Bytes(`key:${JSON.stringify(key)}`, limits.identifierBytes) > limits.identifierBytes))
      throw Error("UI_LIST: invalid or duplicate itemKey");
  }
  function listKeys(props) {
    if (!Number.isSafeInteger(props.count) || props.count < 0 || props.count > limits.listRows || typeof props.row !== "function" || props.height !== void 0 && typeof props.height !== "function") throw Error("UI_LIST: invalid row description");
    if (props.itemKeys !== void 0) {
      if (!Array.isArray(props.itemKeys) || props.itemKeys.length !== props.count) throw Error("UI_LIST: itemKeys must match count");
      if (!settled(props.itemKeys)) props.itemKeys = Array.from(props.itemKeys);
      checkKeys(props.itemKeys, checkKey);
      if (keyPayload(props.itemKeys).bytes > limits.itemKeyBytes) throw Error("UI_LIST: itemKeys budget exceeded");
    }
    if (props.anchor !== void 0 && props.anchor !== "end") {
      const a = props.anchor;
      if (!a || typeof a !== "object" || Object.keys(a).some((k) => !["index", "align", "seq", "duration"].includes(k)) || !Number.isSafeInteger(a.index) || a.index < 0 || !["start", "end"].includes(a.align) || a.seq !== void 0 && (!Number.isSafeInteger(a.seq) || a.seq < 0) || a.duration !== void 0 && (!Number.isFinite(a.duration) || a.duration < 0 || a.duration > 6e4)) throw Error("UI_LIST: invalid anchor");
      props.anchor = {
        index: a.index,
        align: a.align,
        ...a.seq === void 0 ? {} : { seq: a.seq },
        ...a.duration ? { duration: a.duration } : {}
      };
    }
  }
  var iso = { date: /^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/, time: /^([01]\d|2[0-3]):[0-5]\d(:[0-5]\d)?$/ };
  var inputTypes = { text: "text", password: "text", email: "email", number: "decimal", tel: "phone", search: "search", date: "date", time: "time" };
  var inputModes = { text: "text", numeric: "number", decimal: "decimal", tel: "phone", email: "email" };
  function editorKind(tag, props) {
    const { type, inputMode } = props;
    if (type !== void 0 && (tag !== "input" || !Object.hasOwn(inputTypes, type))) {
      const hint = tag === "input" ? "" : "; set inputMode on <textarea>";
      throw Error(`UI_INPUT: type ${JSON.stringify(type)} is not one of ${Object.keys(inputTypes).join(", ")}${hint}`);
    }
    if (inputMode !== void 0 && !Object.hasOwn(inputModes, inputMode))
      throw Error(`UI_INPUT: inputMode ${JSON.stringify(inputMode)} is not one of ${Object.keys(inputModes).join(", ")}`);
    if (inputMode !== void 0 && ["number", "search", "date", "time"].includes(type) && !(type === "number" && ["numeric", "decimal"].includes(inputMode)))
      throw Error(`UI_INPUT: type="${type}" picks its own keyboard; drop inputMode`);
    const keyboard = inputMode !== void 0 ? inputModes[inputMode] : type !== void 0 ? inputTypes[type] : void 0;
    return { keyboard, secure: type === "password" };
  }
  function editor(tag, props, keyboard) {
    const out = [];
    if (props.maxLength !== void 0) {
      if (!Number.isInteger(props.maxLength) || props.maxLength < 1 || props.maxLength > 65536) throw Error("UI_INPUT: maxLength is an integer 1..65536");
      out.push(["maxLength", props.maxLength]);
    }
    for (const [name, key] of [["autoComplete", "autoComplete"], ["enterKeyHint", "enterKeyHint"]]) {
      if (props[name] === void 0) continue;
      if (!fieldOptions[key].includes(props[name])) throw Error(`UI_INPUT: ${name} must be one of ${fieldOptions[key].join(", ")}`);
      out.push([key, props[name]]);
    }
    if (props.autoCapitalize !== void 0) {
      const value = { off: "none", on: "sentences" }[props.autoCapitalize] ?? props.autoCapitalize;
      if (!fieldOptions.autoCapitalize.includes(value)) throw Error("UI_INPUT: autoCapitalize");
      out.push(["autoCapitalize", value]);
    }
    if (props.spellCheck !== void 0) {
      if (typeof props.spellCheck !== "boolean") throw Error("UI_INPUT: spellCheck must be boolean");
      out.push(["spellCheck", +props.spellCheck]);
    }
    if (props.readOnly !== void 0) {
      if (typeof props.readOnly !== "boolean") throw Error("UI_INPUT: readOnly must be boolean");
      if (props.readOnly) out.push(["readOnly", 1]);
    }
    const bounded2 = ["min", "max", "step"].filter((name) => props[name] !== void 0);
    if (bounded2.length) {
      if (tag !== "input" || !["number", "date", "time"].includes(props.type)) throw Error('UI_INPUT: min, max and step need type="number", "date" or "time"');
      const kind = props.type;
      const valid = (value) => kind === "number" ? typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 1e15 : typeof value === "string" && iso[kind].test(value);
      for (const name of ["min", "max"]) if (props[name] !== void 0 && !valid(props[name])) throw Error(`UI_INPUT: ${name} must be a ${kind === "number" ? "number" : `${kind} in ISO form`}`);
      if (props.min !== void 0 && props.max !== void 0 && props.min > props.max) throw Error("UI_INPUT: min exceeds max");
      if (props.step !== void 0 && (kind === "date" || typeof props.step !== "number" || !(props.step > 0) || props.step > 1e15 || kind === "time" && (!Number.isInteger(props.step) || props.step > 86400)))
        throw Error('UI_INPUT: step is a positive number on type="number", or whole seconds up to a day on type="time"');
      for (const [name, key] of [["min", "inputMin"], ["max", "inputMax"], ["step", "inputStep"]]) if (props[name] !== void 0) out.push([key, props[name]]);
    }
    return out;
  }

  // src/runtime/range.js
  var thumbMark = Symbol("slider thumb");
  var drawn = (key) => key === "color" || key.startsWith("padding");
  var tall = (key) => key === "height" || key === "minHeight" || key === "maxHeight";
  var keys = (rule) => Object.keys(rule.style ?? {});
  var pick = (style2, test) => Object.fromEntries(Object.entries(style2 ?? {}).filter(([key]) => test(key)));
  var over = { position: "absolute", top: 0, left: 0, right: 0 };
  function rangeSlider(props) {
    const {
      values,
      thumbLabels,
      onChange,
      onChangeEnd,
      onPressChange,
      className,
      style: style2,
      min,
      max,
      step,
      disabled,
      key,
      id,
      role,
      "data-testid": testId,
      "aria-label": ariaLabel,
      accessibilityLabel = ariaLabel,
      ...rest
    } = props;
    for (const name of ["value", "bind", "defaultValue", "secondaryValue", "aria-valuetext"]) if (rest[name] !== void 0) throw Error(`UI_SLIDER: ${name} belongs to a slider with one value`);
    if (role !== void 0) throw Error("UI_SLIDER: a slider with two values is a group of two sliders; it takes no role");
    if (!Array.isArray(values) || values.length !== 2 || !values.every((value) => typeof value === "number" && Number.isFinite(value)) || values[0] > values[1])
      throw Error("UI_SLIDER: values is [lower, upper], two numbers in order");
    if (!Array.isArray(thumbLabels) || thumbLabels.length !== 2 || !thumbLabels.every((label2) => typeof label2 === "string" && label2.trim()))
      throw Error('UI_A11Y: a slider with two values needs a name for each thumb; add thumbLabels={["From", "To"]}');
    if (typeof accessibilityLabel !== "string" || !accessibilityLabel.trim()) throw Error('UI_A11Y: <slider> needs an accessible name; add aria-label="\u2026"');
    for (const [name, handler] of [["onChange", onChange], ["onChangeEnd", onChangeEnd]])
      if (handler !== void 0 && typeof handler !== "function") throw Error(`UI_SLIDER: ${name} takes a function`);
    const snap = (value) => {
      const one = { min, max, step, value, accessibilityLabel };
      slider(one);
      return one.value;
    };
    const [lower, upper] = [snap(values[0]), snap(values[1])];
    const [own2, placed] = partitionClasses(className, (rule) => rule.when?.part || rule.when?.state || keys(rule).some(drawn));
    const [height] = partitionClasses(placed, (rule) => keys(rule).some(tall));
    const thumb = (at, label2, value, other, extra, ownStyle) => h("slider", {
      ...rest,
      key: at,
      min,
      max,
      step,
      disabled,
      value,
      "aria-label": label2,
      "data-testid": testId && `${testId}-${at}`,
      className: [own2, extra].filter(Boolean).join(" ") || void 0,
      style: ownStyle,
      [thumbMark]: { other, upper: at === "upper" },
      onChange: onChange && ((next) => onChange(at === "upper" ? [lower, next] : [next, upper])),
      onChangeEnd: onChangeEnd && ((next) => onChangeEnd(at === "upper" ? [lower, next] : [next, upper])),
      onPressChange
    });
    const sized2 = pick(style2, (key2) => drawn(key2) || tall(key2));
    const lowerThumb = thumb("lower", thumbLabels[0], lower, upper, height, sized2);
    const upperThumb = thumb("upper", thumbLabels[1], upper, lower, height, { ...sized2, ...over });
    return {
      node: h(
        "view",
        { key, id, role: "group", "aria-label": accessibilityLabel, "data-testid": testId, className: placed || void 0, style: pick(style2, (key2) => !drawn(key2)) },
        lowerThumb,
        upperThumb
      ),
      finish() {
      }
    };
  }

  // src/runtime/video.js
  var fields = [
    "paused",
    "ended",
    "buffering",
    "seeking",
    "readyState",
    "currentTime",
    "duration",
    "buffered",
    "videoWidth",
    "videoHeight",
    "playbackRate",
    "volume",
    "muted",
    "quality",
    "qualities",
    "error",
    "fullscreen"
  ];
  var empty = Object.freeze({
    paused: true,
    ended: false,
    buffering: false,
    seeking: false,
    readyState: "empty",
    currentTime: 0,
    duration: null,
    buffered: 0,
    videoWidth: 0,
    videoHeight: 0,
    playbackRate: 1,
    volume: 1,
    muted: false,
    quality: "auto",
    qualities: Object.freeze([]),
    error: null,
    fullscreen: false
  });
  var players = /* @__PURE__ */ new WeakMap();
  var readers = /* @__PURE__ */ new Map();
  var NAME = /^[A-Za-z0-9_.-]{1,64}$/;
  var fullscreenName = null;
  var latest2 = /* @__PURE__ */ new Map();
  var fullscreenPlayer = () => fullscreenName;
  function resetVideos() {
    fullscreenName = null;
    latest2.clear();
    syncOrientation();
  }
  var orientation = "";
  function syncOrientation() {
    const { videoWidth: w = 0, videoHeight: h2 = 0 } = fullscreenName !== null && latest2.get(fullscreenName) || {};
    const next = fullscreenName === null ? "" : w === h2 && w > 0 ? "free" : h2 > w && w > 0 ? "portrait" : "landscape";
    if (next === orientation) return;
    orientation = next;
    globalThis.__lingxiaVideoOrientation?.(next);
  }
  function fullscreenHandler(name, handler) {
    var _a;
    const handlers = (_a = render.candidate).fullscreenHandlers ?? (_a.fullscreenHandlers = /* @__PURE__ */ new Map());
    if (!handlers.has(name)) handlers.set(name, []);
    handlers.get(name).push(handler);
  }
  function setFullscreen(name) {
    if (fullscreenName === name) return;
    const touched = [fullscreenName, name].filter((n) => n !== null);
    fullscreenName = name;
    page.invalidated = true;
    syncOrientation();
    for (const each of touched) for (const reader of readers.get(each) ?? []) {
      const { key, cell } = cellOf(reader.binding);
      if (page.locals.get(key) === cell) reader.binding.set({ ...reader.binding.value, fullscreen: fullscreenName === each });
    }
    for (const each of touched) for (const handler of page.committed?.fullscreenHandlers?.get(each) ?? []) {
      try {
        handler(Object.freeze({ fullscreen: fullscreenName === each }));
      } catch (error) {
        report(error);
      }
    }
  }
  var playerName = (handle2) => players.get(handle2);
  function useVideo(name) {
    if (typeof name !== "string" || !NAME.test(name)) throw Error('UI_VIDEO: useVideo takes a name of 1..64 letters, digits, "_", "." or "-"');
    const binding = local(name, latest2.get(name) ?? empty, "video");
    const { cell } = cellOf(binding);
    let reader = cell.videoReader;
    if (!reader) {
      reader = cell.videoReader = { binding, read: /* @__PURE__ */ new Set() };
      if (!readers.has(name)) readers.set(name, /* @__PURE__ */ new Set());
      readers.get(name).add(reader);
    }
    reader.read.clear();
    const handle2 = {};
    for (const field2 of fields) Object.defineProperty(handle2, field2, { enumerable: true, get() {
      if (render.active) reader.read.add(field2);
      return field2 === "fullscreen" ? fullscreenName === name : binding.value[field2];
    } });
    const command = (op, value) => {
      outsideRender("video command");
      const failure2 = globalThis.__lingxiaVideoCommand?.(`name:${name}`, op, typeof value === "number" ? value : 0, typeof value === "string" ? value : "") ?? "VIDEO_NO_PLAYER: this host plays no video";
      if (!failure2) return Promise.resolve();
      const [code] = failure2.split(":", 1);
      return Promise.reject(Object.assign(Error(failure2), { code, recoverable: code === "VIDEO_NETWORK" }));
    };
    const number = (what, value, min, max) => {
      if (typeof value !== "number" || !Number.isFinite(value) || value < min || value > max) throw Error(`UI_VIDEO: ${what} takes a number in ${min}..${max}`);
      return value;
    };
    Object.assign(handle2, {
      play: () => command("play"),
      pause: () => command("pause"),
      seek: (seconds) => command("seek", number("seek", seconds, 0, 1e7)),
      setMuted: (muted) => {
        if (typeof muted !== "boolean") throw Error("UI_VIDEO: setMuted takes a boolean");
        return command("muted", +muted);
      },
      setVolume: (volume) => command("volume", number("setVolume", volume, 0, 1)),
      setPlaybackRate: (rate) => command("rate", number("setPlaybackRate", rate, 0.25, 4)),
      setQuality: (quality) => {
        if (typeof quality !== "string" || !quality) throw Error('UI_VIDEO: setQuality takes "auto" or a label of sources');
        return command("quality", quality);
      },
      retry: () => command("retry"),
      requestFullscreen: () => {
        outsideRender("video command");
        setFullscreen(name);
        return Promise.resolve();
      },
      exitFullscreen: () => {
        outsideRender("video command");
        if (fullscreenName === name) setFullscreen(null);
        return Promise.resolve();
      }
    });
    Object.freeze(handle2);
    players.set(handle2, name);
    return handle2;
  }
  var same2 = (a, b) => Object.is(a, b) || typeof a === "object" && typeof b === "object" && a !== null && b !== null && JSON.stringify(a) === JSON.stringify(b);
  function videoStatus(name, status) {
    const next = Object.freeze({ ...status, fullscreen: fullscreenName === name, qualities: Object.freeze([...status.qualities ?? []]), error: status.error ? Object.freeze({ ...status.error }) : null });
    latest2.set(name, next);
    if (name === fullscreenName) syncOrientation();
    const named = readers.get(name);
    for (const reader of named ?? []) {
      const { key, cell } = cellOf(reader.binding);
      if (page.locals.get(key) !== cell) {
        named.delete(reader);
        continue;
      }
      const current2 = reader.binding.value;
      if ([...reader.read].some((field2) => !same2(current2[field2], next[field2]))) reader.binding.set(next);
      else cell.value = next;
    }
  }

  // src/runtime/box.js
  var handles = /* @__PURE__ */ new WeakMap();
  var written = /* @__PURE__ */ new WeakMap();
  var boxBinding = (handle2) => written.get(handle2);
  function useBox(name) {
    if (typeof name !== "string" || !name.length || name.length > 64) throw Error("UI_PROP: useBox takes a name of 1..64 characters");
    const binding = local(name, null, "box");
    const { cell } = cellOf(binding);
    let handle2 = handles.get(cell);
    if (!handle2) {
      handle2 = Object.freeze({ get value() {
        return binding.value;
      } });
      handles.set(cell, handle2);
      written.set(handle2, binding);
    }
    return handle2;
  }

  // src/runtime/fullscreen.js
  function shownInDialog(name) {
    var _a;
    ((_a = render.candidate).fullscreenDrawn ?? (_a.fullscreenDrawn = /* @__PURE__ */ new Set())).add(name);
  }
  function fullscreenDrawn() {
    const name = fullscreenPlayer();
    return name !== null && render.candidate.fullscreenDrawn?.has(name) === true;
  }
  function resolveBare(resolve) {
    const kept2 = render.candidate.fullscreenDrawn;
    render.candidate.fullscreenDrawn = new Set(kept2);
    try {
      return resolve();
    } finally {
      render.candidate.fullscreenDrawn = kept2;
    }
  }
  function BareFullscreen({ name }) {
    const player = useVideo(name);
    return h(
      "dialog",
      {
        open: true,
        closedby: "closerequest",
        onClose: () => player.exitFullscreen(),
        "aria-label": "Full screen video",
        "data-testid": "lx-fullscreen-video",
        style: {
          width: "100%",
          height: "100%",
          maxWidth: "100%",
          maxHeight: "100%",
          marginTop: 0,
          marginBottom: 0,
          marginLeft: 0,
          marginRight: 0,
          paddingTop: 0,
          paddingBottom: 0,
          paddingLeft: 0,
          paddingRight: 0,
          borderRadius: 0,
          backgroundColor: "#000000"
        }
      },
      h("video", { player, style: { width: "100%", flexGrow: 1, objectFit: "contain" } })
    );
  }
  function fullscreenFallback() {
    const name = fullscreenPlayer();
    return name === null || fullscreenDrawn() ? null : h(BareFullscreen, { name, key: `lx-fullscreen:${name}` });
  }

  // src/generated/words.js
  var source2 = "en";
  var words = {
    "ar": {
      "@view.cancel": (a, f) => "\u0625\u0644\u063A\u0627\u0621",
      "@view.done": (a, f) => "\u062A\u0645",
      "@view.error.failed": (a, f) => "\u0644\u0645 \u062A\u0646\u062C\u062D \u0627\u0644\u0639\u0645\u0644\u064A\u0629. \u062D\u0627\u0648\u0644 \u0645\u0631\u0629 \u0623\u062E\u0631\u0649.",
      "@view.error.limit": (a, f) => "\u0644\u0642\u062F \u0628\u0644\u063A\u062A \u0627\u0644\u062D\u062F \u0627\u0644\u0645\u0633\u0645\u0648\u062D. \u062D\u0627\u0648\u0644 \u0645\u0631\u0629 \u0623\u062E\u0631\u0649 \u0644\u0627\u062D\u0642\u064B\u0627.",
      "@view.error.network": (a, f) => "\u062A\u0639\u0630\u0651\u0631 \u0627\u0644\u0627\u062A\u0635\u0627\u0644. \u062A\u062D\u0642\u0642 \u0645\u0646 \u0627\u062A\u0635\u0627\u0644\u0643 \u0648\u062D\u0627\u0648\u0644 \u0645\u0631\u0629 \u0623\u062E\u0631\u0649.",
      "@view.error.notFound": (a, f) => "\u0644\u0645 \u064A\u0639\u062F \u0647\u0630\u0627 \u0645\u062A\u0627\u062D\u064B\u0627.",
      "@view.error.permission": (a, f) => "\u064A\u0644\u0632\u0645 \u0627\u0644\u062D\u0635\u0648\u0644 \u0639\u0644\u0649 \u0625\u0630\u0646\u0643. \u064A\u0645\u0643\u0646\u0643 \u0645\u0646\u062D\u0647 \u0645\u0646 \u0627\u0644\u0625\u0639\u062F\u0627\u062F\u0627\u062A.",
      "@view.error.timeout": (a, f) => "\u064A\u0633\u062A\u063A\u0631\u0642 \u0630\u0644\u0643 \u0648\u0642\u062A\u064B\u0627 \u0637\u0648\u064A\u0644\u0627\u064B. \u062D\u0627\u0648\u0644 \u0645\u0631\u0629 \u0623\u062E\u0631\u0649.",
      "@view.error.unsupported": (a, f) => "\u063A\u064A\u0631 \u0645\u062A\u0627\u062D \u0639\u0644\u0649 \u0647\u0630\u0627 \u0627\u0644\u062C\u0647\u0627\u0632.",
      "@view.hour": (a, f) => "\u0627\u0644\u0633\u0627\u0639\u0629",
      "@view.minute": (a, f) => "\u0627\u0644\u062F\u0642\u064A\u0642\u0629",
      "@view.nextMonth": (a, f) => "\u0627\u0644\u0634\u0647\u0631 \u0627\u0644\u062A\u0627\u0644\u064A",
      "@view.options": (a, f) => "\u062E\u064A\u0627\u0631\u0627\u062A",
      "@view.period": (a, f) => "\u0635/\u0645",
      "@view.previousMonth": (a, f) => "\u0627\u0644\u0634\u0647\u0631 \u0627\u0644\u0633\u0627\u0628\u0642",
      "@view.second": (a, f) => "\u062B\u0627\u0646\u064A\u0629"
    },
    "de": {
      "@view.cancel": (a, f) => "Abbrechen",
      "@view.done": (a, f) => "Fertig",
      "@view.error.failed": (a, f) => "Das hat nicht geklappt. Versuche es noch einmal.",
      "@view.error.limit": (a, f) => "Du hast ein Limit erreicht. Versuche es sp\xE4ter noch einmal.",
      "@view.error.network": (a, f) => "Keine Verbindung. Pr\xFCfe deine Verbindung und versuche es noch einmal.",
      "@view.error.notFound": (a, f) => "Das ist nicht mehr verf\xFCgbar.",
      "@view.error.permission": (a, f) => "Daf\xFCr ist deine Erlaubnis n\xF6tig. Du kannst sie in den Einstellungen erteilen.",
      "@view.error.timeout": (a, f) => "Das dauert zu lange. Versuche es noch einmal.",
      "@view.error.unsupported": (a, f) => "Auf diesem Ger\xE4t nicht verf\xFCgbar.",
      "@view.hour": (a, f) => "Stunde",
      "@view.minute": (a, f) => "Minute",
      "@view.nextMonth": (a, f) => "N\xE4chster Monat",
      "@view.options": (a, f) => "Optionen",
      "@view.period": (a, f) => "AM/PM",
      "@view.previousMonth": (a, f) => "Vorheriger Monat",
      "@view.second": (a, f) => "Sekunde"
    },
    "en": {
      "@view.cancel": (a, f) => "Cancel",
      "@view.done": (a, f) => "Done",
      "@view.error.failed": (a, f) => "That didn't work. Try again.",
      "@view.error.limit": (a, f) => "You've reached a limit. Try again later.",
      "@view.error.network": (a, f) => "Can't connect. Check your connection and try again.",
      "@view.error.notFound": (a, f) => "This is no longer available.",
      "@view.error.permission": (a, f) => "This needs your permission. You can allow it in Settings.",
      "@view.error.timeout": (a, f) => "This is taking too long. Try again.",
      "@view.error.unsupported": (a, f) => "This isn't available on this device.",
      "@view.hour": (a, f) => "Hour",
      "@view.minute": (a, f) => "Minute",
      "@view.nextMonth": (a, f) => "Next month",
      "@view.options": (a, f) => "Options",
      "@view.period": (a, f) => "AM/PM",
      "@view.previousMonth": (a, f) => "Previous month",
      "@view.second": (a, f) => "Second"
    },
    "es": {
      "@view.cancel": (a, f) => "Cancelar",
      "@view.done": (a, f) => "Listo",
      "@view.error.failed": (a, f) => "No ha funcionado. Int\xE9ntalo de nuevo.",
      "@view.error.limit": (a, f) => "Has alcanzado un l\xEDmite. Int\xE9ntalo m\xE1s tarde.",
      "@view.error.network": (a, f) => "No se puede conectar. Comprueba tu conexi\xF3n e int\xE9ntalo de nuevo.",
      "@view.error.notFound": (a, f) => "Ya no est\xE1 disponible.",
      "@view.error.permission": (a, f) => "Se necesita tu permiso. Puedes concederlo en Ajustes.",
      "@view.error.timeout": (a, f) => "Est\xE1 tardando demasiado. Int\xE9ntalo de nuevo.",
      "@view.error.unsupported": (a, f) => "No est\xE1 disponible en este dispositivo.",
      "@view.hour": (a, f) => "Hora",
      "@view.minute": (a, f) => "Minuto",
      "@view.nextMonth": (a, f) => "Mes siguiente",
      "@view.options": (a, f) => "Opciones",
      "@view.period": (a, f) => "a. m./p. m.",
      "@view.previousMonth": (a, f) => "Mes anterior",
      "@view.second": (a, f) => "Segundo"
    },
    "fr": {
      "@view.cancel": (a, f) => "Annuler",
      "@view.done": (a, f) => "OK",
      "@view.error.failed": (a, f) => "Cela n'a pas fonctionn\xE9. R\xE9essayez.",
      "@view.error.limit": (a, f) => "Vous avez atteint une limite. R\xE9essayez plus tard.",
      "@view.error.network": (a, f) => "Connexion impossible. V\xE9rifiez votre connexion et r\xE9essayez.",
      "@view.error.notFound": (a, f) => "Ce contenu n'est plus disponible.",
      "@view.error.permission": (a, f) => "Votre autorisation est n\xE9cessaire. Vous pouvez l'accorder dans les R\xE9glages.",
      "@view.error.timeout": (a, f) => "Cela prend trop de temps. R\xE9essayez.",
      "@view.error.unsupported": (a, f) => "Non disponible sur cet appareil.",
      "@view.hour": (a, f) => "Heure",
      "@view.minute": (a, f) => "Minute",
      "@view.nextMonth": (a, f) => "Mois suivant",
      "@view.options": (a, f) => "Options",
      "@view.period": (a, f) => "AM/PM",
      "@view.previousMonth": (a, f) => "Mois pr\xE9c\xE9dent",
      "@view.second": (a, f) => "Seconde"
    },
    "ja": {
      "@view.cancel": (a, f) => "\u30AD\u30E3\u30F3\u30BB\u30EB",
      "@view.done": (a, f) => "\u5B8C\u4E86",
      "@view.error.failed": (a, f) => "\u3046\u307E\u304F\u3044\u304D\u307E\u305B\u3093\u3067\u3057\u305F\u3002\u3082\u3046\u4E00\u5EA6\u304A\u8A66\u3057\u304F\u3060\u3055\u3044\u3002",
      "@view.error.limit": (a, f) => "\u4E0A\u9650\u306B\u9054\u3057\u307E\u3057\u305F\u3002\u3057\u3070\u3089\u304F\u3057\u3066\u304B\u3089\u304A\u8A66\u3057\u304F\u3060\u3055\u3044\u3002",
      "@view.error.network": (a, f) => "\u63A5\u7D9A\u3067\u304D\u307E\u305B\u3093\u3002\u30CD\u30C3\u30C8\u30EF\u30FC\u30AF\u3092\u78BA\u8A8D\u3057\u3066\u3001\u3082\u3046\u4E00\u5EA6\u304A\u8A66\u3057\u304F\u3060\u3055\u3044\u3002",
      "@view.error.notFound": (a, f) => "\u3053\u306E\u30B3\u30F3\u30C6\u30F3\u30C4\u306F\u5229\u7528\u3067\u304D\u306A\u304F\u306A\u308A\u307E\u3057\u305F\u3002",
      "@view.error.permission": (a, f) => "\u8A31\u53EF\u304C\u5FC5\u8981\u3067\u3059\u3002\u300C\u8A2D\u5B9A\u300D\u3067\u8A31\u53EF\u3067\u304D\u307E\u3059\u3002",
      "@view.error.timeout": (a, f) => "\u6642\u9593\u304C\u304B\u304B\u308A\u3059\u304E\u3066\u3044\u307E\u3059\u3002\u3082\u3046\u4E00\u5EA6\u304A\u8A66\u3057\u304F\u3060\u3055\u3044\u3002",
      "@view.error.unsupported": (a, f) => "\u3053\u306E\u30C7\u30D0\u30A4\u30B9\u3067\u306F\u3054\u5229\u7528\u3044\u305F\u3060\u3051\u307E\u305B\u3093\u3002",
      "@view.hour": (a, f) => "\u6642",
      "@view.minute": (a, f) => "\u5206",
      "@view.nextMonth": (a, f) => "\u6B21\u306E\u6708",
      "@view.options": (a, f) => "\u30AA\u30D7\u30B7\u30E7\u30F3",
      "@view.period": (a, f) => "\u5348\u524D/\u5348\u5F8C",
      "@view.previousMonth": (a, f) => "\u524D\u306E\u6708",
      "@view.second": (a, f) => "\u79D2"
    },
    "ko": {
      "@view.cancel": (a, f) => "\uCDE8\uC18C",
      "@view.done": (a, f) => "\uC644\uB8CC",
      "@view.error.failed": (a, f) => "\uC791\uC5C5\uC744 \uC644\uB8CC\uD558\uC9C0 \uBABB\uD588\uC2B5\uB2C8\uB2E4. \uB2E4\uC2DC \uC2DC\uB3C4\uD558\uC138\uC694.",
      "@view.error.limit": (a, f) => "\uD55C\uB3C4\uC5D0 \uB3C4\uB2EC\uD588\uC2B5\uB2C8\uB2E4. \uB098\uC911\uC5D0 \uB2E4\uC2DC \uC2DC\uB3C4\uD558\uC138\uC694.",
      "@view.error.network": (a, f) => "\uC5F0\uACB0\uD560 \uC218 \uC5C6\uC2B5\uB2C8\uB2E4. \uB124\uD2B8\uC6CC\uD06C\uB97C \uD655\uC778\uD55C \uD6C4 \uB2E4\uC2DC \uC2DC\uB3C4\uD558\uC138\uC694.",
      "@view.error.notFound": (a, f) => "\uB354 \uC774\uC0C1 \uC0AC\uC6A9\uD560 \uC218 \uC5C6\uC2B5\uB2C8\uB2E4.",
      "@view.error.permission": (a, f) => "\uAD8C\uD55C\uC774 \uD544\uC694\uD569\uB2C8\uB2E4. \uC124\uC815\uC5D0\uC11C \uD5C8\uC6A9\uD560 \uC218 \uC788\uC2B5\uB2C8\uB2E4.",
      "@view.error.timeout": (a, f) => "\uC2DC\uAC04\uC774 \uB108\uBB34 \uC624\uB798 \uAC78\uB9BD\uB2C8\uB2E4. \uB2E4\uC2DC \uC2DC\uB3C4\uD558\uC138\uC694.",
      "@view.error.unsupported": (a, f) => "\uC774 \uAE30\uAE30\uC5D0\uC11C\uB294 \uC0AC\uC6A9\uD560 \uC218 \uC5C6\uC2B5\uB2C8\uB2E4.",
      "@view.hour": (a, f) => "\uC2DC",
      "@view.minute": (a, f) => "\uBD84",
      "@view.nextMonth": (a, f) => "\uB2E4\uC74C \uB2EC",
      "@view.options": (a, f) => "\uC635\uC158",
      "@view.period": (a, f) => "\uC624\uC804/\uC624\uD6C4",
      "@view.previousMonth": (a, f) => "\uC774\uC804 \uB2EC",
      "@view.second": (a, f) => "\uCD08"
    },
    "pt": {
      "@view.cancel": (a, f) => "Cancelar",
      "@view.done": (a, f) => "OK",
      "@view.error.failed": (a, f) => "N\xE3o funcionou. Tente novamente.",
      "@view.error.limit": (a, f) => "Voc\xEA atingiu um limite. Tente novamente mais tarde.",
      "@view.error.network": (a, f) => "N\xE3o foi poss\xEDvel conectar. Verifique sua conex\xE3o e tente novamente.",
      "@view.error.notFound": (a, f) => "Isto n\xE3o est\xE1 mais dispon\xEDvel.",
      "@view.error.permission": (a, f) => "\xC9 necess\xE1ria sua permiss\xE3o. Voc\xEA pode conced\xEA-la em Ajustes.",
      "@view.error.timeout": (a, f) => "Est\xE1 demorando demais. Tente novamente.",
      "@view.error.unsupported": (a, f) => "N\xE3o dispon\xEDvel neste dispositivo.",
      "@view.hour": (a, f) => "Hora",
      "@view.minute": (a, f) => "Minuto",
      "@view.nextMonth": (a, f) => "Pr\xF3ximo m\xEAs",
      "@view.options": (a, f) => "Op\xE7\xF5es",
      "@view.period": (a, f) => "AM/PM",
      "@view.previousMonth": (a, f) => "M\xEAs anterior",
      "@view.second": (a, f) => "Segundo"
    },
    "ru": {
      "@view.cancel": (a, f) => "\u041E\u0442\u043C\u0435\u043D\u0430",
      "@view.done": (a, f) => "\u0413\u043E\u0442\u043E\u0432\u043E",
      "@view.error.failed": (a, f) => "\u041D\u0435 \u043F\u043E\u043B\u0443\u0447\u0438\u043B\u043E\u0441\u044C. \u041F\u043E\u043F\u0440\u043E\u0431\u0443\u0439\u0442\u0435 \u0435\u0449\u0451 \u0440\u0430\u0437.",
      "@view.error.limit": (a, f) => "\u0414\u043E\u0441\u0442\u0438\u0433\u043D\u0443\u0442 \u043B\u0438\u043C\u0438\u0442. \u041F\u043E\u043F\u0440\u043E\u0431\u0443\u0439\u0442\u0435 \u043F\u043E\u0437\u0436\u0435.",
      "@view.error.network": (a, f) => "\u041D\u0435\u0442 \u043F\u043E\u0434\u043A\u043B\u044E\u0447\u0435\u043D\u0438\u044F. \u041F\u0440\u043E\u0432\u0435\u0440\u044C\u0442\u0435 \u0441\u043E\u0435\u0434\u0438\u043D\u0435\u043D\u0438\u0435 \u0438 \u043F\u043E\u043F\u0440\u043E\u0431\u0443\u0439\u0442\u0435 \u0435\u0449\u0451 \u0440\u0430\u0437.",
      "@view.error.notFound": (a, f) => "\u042D\u0442\u043E \u0431\u043E\u043B\u044C\u0448\u0435 \u043D\u0435\u0434\u043E\u0441\u0442\u0443\u043F\u043D\u043E.",
      "@view.error.permission": (a, f) => "\u0422\u0440\u0435\u0431\u0443\u0435\u0442\u0441\u044F \u0432\u0430\u0448\u0435 \u0440\u0430\u0437\u0440\u0435\u0448\u0435\u043D\u0438\u0435. \u0415\u0433\u043E \u043C\u043E\u0436\u043D\u043E \u0434\u0430\u0442\u044C \u0432 \u043D\u0430\u0441\u0442\u0440\u043E\u0439\u043A\u0430\u0445.",
      "@view.error.timeout": (a, f) => "\u042D\u0442\u043E \u0437\u0430\u043D\u0438\u043C\u0430\u0435\u0442 \u0441\u043B\u0438\u0448\u043A\u043E\u043C \u043C\u043D\u043E\u0433\u043E \u0432\u0440\u0435\u043C\u0435\u043D\u0438. \u041F\u043E\u043F\u0440\u043E\u0431\u0443\u0439\u0442\u0435 \u0435\u0449\u0451 \u0440\u0430\u0437.",
      "@view.error.unsupported": (a, f) => "\u041D\u0435\u0434\u043E\u0441\u0442\u0443\u043F\u043D\u043E \u043D\u0430 \u044D\u0442\u043E\u043C \u0443\u0441\u0442\u0440\u043E\u0439\u0441\u0442\u0432\u0435.",
      "@view.hour": (a, f) => "\u0427\u0430\u0441",
      "@view.minute": (a, f) => "\u041C\u0438\u043D\u0443\u0442\u0430",
      "@view.nextMonth": (a, f) => "\u0421\u043B\u0435\u0434\u0443\u044E\u0449\u0438\u0439 \u043C\u0435\u0441\u044F\u0446",
      "@view.options": (a, f) => "\u041F\u0430\u0440\u0430\u043C\u0435\u0442\u0440\u044B",
      "@view.period": (a, f) => "AM/PM",
      "@view.previousMonth": (a, f) => "\u041F\u0440\u0435\u0434\u044B\u0434\u0443\u0449\u0438\u0439 \u043C\u0435\u0441\u044F\u0446",
      "@view.second": (a, f) => "\u0421\u0435\u043A\u0443\u043D\u0434\u0430"
    },
    "zh": {
      "@view.cancel": (a, f) => "\u53D6\u6D88",
      "@view.done": (a, f) => "\u5B8C\u6210",
      "@view.error.failed": (a, f) => "\u64CD\u4F5C\u6CA1\u6709\u6210\u529F\uFF0C\u8BF7\u91CD\u8BD5\u3002",
      "@view.error.limit": (a, f) => "\u5DF2\u8FBE\u5230\u4F7F\u7528\u4E0A\u9650\uFF0C\u8BF7\u7A0D\u540E\u518D\u8BD5\u3002",
      "@view.error.network": (a, f) => "\u65E0\u6CD5\u8FDE\u63A5\uFF0C\u8BF7\u68C0\u67E5\u7F51\u7EDC\u540E\u91CD\u8BD5\u3002",
      "@view.error.notFound": (a, f) => "\u5185\u5BB9\u5DF2\u4E0D\u5B58\u5728\u3002",
      "@view.error.permission": (a, f) => "\u9700\u8981\u4F60\u7684\u6388\u6743\uFF0C\u53EF\u4EE5\u5728\u201C\u8BBE\u7F6E\u201D\u4E2D\u5F00\u542F\u3002",
      "@view.error.timeout": (a, f) => "\u7B49\u5F85\u65F6\u95F4\u8FC7\u957F\uFF0C\u8BF7\u91CD\u8BD5\u3002",
      "@view.error.unsupported": (a, f) => "\u6B64\u8BBE\u5907\u4E0D\u652F\u6301\u8BE5\u529F\u80FD\u3002",
      "@view.hour": (a, f) => "\u65F6",
      "@view.minute": (a, f) => "\u5206",
      "@view.nextMonth": (a, f) => "\u4E0B\u4E2A\u6708",
      "@view.options": (a, f) => "\u9009\u9879",
      "@view.period": (a, f) => "\u4E0A\u5348/\u4E0B\u5348",
      "@view.previousMonth": (a, f) => "\u4E0A\u4E2A\u6708",
      "@view.second": (a, f) => "\u79D2"
    }
  };

  // src/runtime/i18n.js
  var catalogues = null;
  function registerCatalogues(compiled) {
    catalogues = compiled;
    formatters.clear();
  }
  function registerLanguageCatalogue(tag, messages, formatter2, source3, declared2) {
    catalogues ?? (catalogues = { source: source3, declared: declared2, languages: {}, formatter: formatter2 });
    catalogues.languages[tag] = messages;
    formatters.delete(tag);
  }
  function catalogueFor(language) {
    const tags2 = catalogues.declared ?? Object.keys(catalogues.languages);
    const lower = String(language ?? "").toLowerCase();
    const exact = tags2.find((tag2) => tag2.toLowerCase() === lower);
    const primary = lower.split("-")[0];
    const tag = exact ?? tags2.find((tag2) => tag2.toLowerCase().split("-")[0] === primary) ?? catalogues.source;
    return { tag, messages: catalogues.languages[tag] };
  }
  var join = (text) => Array.isArray(text) ? text.join("") : String(text);
  function lxappText(key, language, args) {
    if (!catalogues) return void 0;
    const { tag, messages } = catalogueFor(language);
    const message = messages?.[key];
    return message ? join(message(args ?? {}, formatter(tag))) : void 0;
  }
  var formatters = /* @__PURE__ */ new Map();
  function formatter(tag) {
    let f = formatters.get(tag);
    if (!f) formatters.set(tag, f = catalogues.formatter(tag));
    return f;
  }

  // src/runtime/words.js
  function word(key, language) {
    const own2 = lxappText(key, language);
    if (own2 !== void 0) return own2;
    const table = words[String(language ?? "").split(/[-_]/)[0].toLowerCase()] ?? words[source2];
    return (table[key] ?? words[source2][key])({}, null);
  }

  // src/runtime/calendar.js
  var names2 = {
    en: {
      months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
      days: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
      longDays: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
      title: (month, year) => `${month} ${year}`,
      full: (day, month, year, weekday2) => `${weekday2}, ${month} ${day}, ${year}`,
      am: "AM",
      pm: "PM"
    },
    zh: {
      months: ["1\u6708", "2\u6708", "3\u6708", "4\u6708", "5\u6708", "6\u6708", "7\u6708", "8\u6708", "9\u6708", "10\u6708", "11\u6708", "12\u6708"],
      days: ["\u65E5", "\u4E00", "\u4E8C", "\u4E09", "\u56DB", "\u4E94", "\u516D"],
      longDays: ["\u661F\u671F\u65E5", "\u661F\u671F\u4E00", "\u661F\u671F\u4E8C", "\u661F\u671F\u4E09", "\u661F\u671F\u56DB", "\u661F\u671F\u4E94", "\u661F\u671F\u516D"],
      title: (month, year) => `${year}\u5E74${month}`,
      full: (day, month, year, weekday2) => `${year}\u5E74${month}${day}\u65E5${weekday2}`,
      am: "\u4E0A\u5348",
      pm: "\u4E0B\u5348"
    },
    ja: {
      months: ["1\u6708", "2\u6708", "3\u6708", "4\u6708", "5\u6708", "6\u6708", "7\u6708", "8\u6708", "9\u6708", "10\u6708", "11\u6708", "12\u6708"],
      days: ["\u65E5", "\u6708", "\u706B", "\u6C34", "\u6728", "\u91D1", "\u571F"],
      longDays: ["\u65E5\u66DC\u65E5", "\u6708\u66DC\u65E5", "\u706B\u66DC\u65E5", "\u6C34\u66DC\u65E5", "\u6728\u66DC\u65E5", "\u91D1\u66DC\u65E5", "\u571F\u66DC\u65E5"],
      title: (month, year) => `${year}\u5E74${month}`,
      full: (day, month, year, weekday2) => `${year}\u5E74${month}${day}\u65E5 ${weekday2}`,
      am: "\u5348\u524D",
      pm: "\u5348\u5F8C"
    },
    ko: {
      months: ["1\uC6D4", "2\uC6D4", "3\uC6D4", "4\uC6D4", "5\uC6D4", "6\uC6D4", "7\uC6D4", "8\uC6D4", "9\uC6D4", "10\uC6D4", "11\uC6D4", "12\uC6D4"],
      days: ["\uC77C", "\uC6D4", "\uD654", "\uC218", "\uBAA9", "\uAE08", "\uD1A0"],
      longDays: ["\uC77C\uC694\uC77C", "\uC6D4\uC694\uC77C", "\uD654\uC694\uC77C", "\uC218\uC694\uC77C", "\uBAA9\uC694\uC77C", "\uAE08\uC694\uC77C", "\uD1A0\uC694\uC77C"],
      title: (month, year) => `${year}\uB144 ${month}`,
      full: (day, month, year, weekday2) => `${year}\uB144 ${month} ${day}\uC77C ${weekday2}`,
      am: "\uC624\uC804",
      pm: "\uC624\uD6C4"
    },
    de: {
      months: ["Januar", "Februar", "M\xE4rz", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
      days: ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
      longDays: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
      title: (month, year) => `${month} ${year}`,
      full: (day, month, year, weekday2) => `${weekday2}, ${day}. ${month} ${year}`,
      am: "AM",
      pm: "PM"
    },
    fr: {
      months: ["janvier", "f\xE9vrier", "mars", "avril", "mai", "juin", "juillet", "ao\xFBt", "septembre", "octobre", "novembre", "d\xE9cembre"],
      days: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
      longDays: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
      title: (month, year) => `${month} ${year}`,
      full: (day, month, year, weekday2) => `${weekday2} ${day} ${month} ${year}`,
      am: "AM",
      pm: "PM"
    },
    es: {
      months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
      days: ["dom", "lun", "mar", "mi\xE9", "jue", "vie", "s\xE1b"],
      longDays: ["domingo", "lunes", "martes", "mi\xE9rcoles", "jueves", "viernes", "s\xE1bado"],
      title: (month, year) => `${month} de ${year}`,
      full: (day, month, year, weekday2) => `${weekday2}, ${day} de ${month} de ${year}`,
      am: "a. m.",
      pm: "p. m."
    },
    pt: {
      months: ["janeiro", "fevereiro", "mar\xE7o", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
      days: ["dom.", "seg.", "ter.", "qua.", "qui.", "sex.", "s\xE1b."],
      longDays: ["domingo", "segunda-feira", "ter\xE7a-feira", "quarta-feira", "quinta-feira", "sexta-feira", "s\xE1bado"],
      title: (month, year) => `${month} de ${year}`,
      full: (day, month, year, weekday2) => `${weekday2}, ${day} de ${month} de ${year}`,
      am: "AM",
      pm: "PM"
    },
    ru: {
      months: ["\u044F\u043D\u0432\u0430\u0440\u044C", "\u0444\u0435\u0432\u0440\u0430\u043B\u044C", "\u043C\u0430\u0440\u0442", "\u0430\u043F\u0440\u0435\u043B\u044C", "\u043C\u0430\u0439", "\u0438\u044E\u043D\u044C", "\u0438\u044E\u043B\u044C", "\u0430\u0432\u0433\u0443\u0441\u0442", "\u0441\u0435\u043D\u0442\u044F\u0431\u0440\u044C", "\u043E\u043A\u0442\u044F\u0431\u0440\u044C", "\u043D\u043E\u044F\u0431\u0440\u044C", "\u0434\u0435\u043A\u0430\u0431\u0440\u044C"],
      days: ["\u0432\u0441", "\u043F\u043D", "\u0432\u0442", "\u0441\u0440", "\u0447\u0442", "\u043F\u0442", "\u0441\u0431"],
      longDays: ["\u0432\u043E\u0441\u043A\u0440\u0435\u0441\u0435\u043D\u044C\u0435", "\u043F\u043E\u043D\u0435\u0434\u0435\u043B\u044C\u043D\u0438\u043A", "\u0432\u0442\u043E\u0440\u043D\u0438\u043A", "\u0441\u0440\u0435\u0434\u0430", "\u0447\u0435\u0442\u0432\u0435\u0440\u0433", "\u043F\u044F\u0442\u043D\u0438\u0446\u0430", "\u0441\u0443\u0431\u0431\u043E\u0442\u0430"],
      title: (month, year) => `${month} ${year}`,
      full: (day, month, year, weekday2) => `${weekday2}, ${day} ${month} ${year}`,
      am: "AM",
      pm: "PM"
    },
    ar: {
      months: ["\u064A\u0646\u0627\u064A\u0631", "\u0641\u0628\u0631\u0627\u064A\u0631", "\u0645\u0627\u0631\u0633", "\u0623\u0628\u0631\u064A\u0644", "\u0645\u0627\u064A\u0648", "\u064A\u0648\u0646\u064A\u0648", "\u064A\u0648\u0644\u064A\u0648", "\u0623\u063A\u0633\u0637\u0633", "\u0633\u0628\u062A\u0645\u0628\u0631", "\u0623\u0643\u062A\u0648\u0628\u0631", "\u0646\u0648\u0641\u0645\u0628\u0631", "\u062F\u064A\u0633\u0645\u0628\u0631"],
      days: ["\u0623\u062D\u062F", "\u0625\u062B\u0646\u064A\u0646", "\u062B\u0644\u0627\u062B\u0627\u0621", "\u0623\u0631\u0628\u0639\u0627\u0621", "\u062E\u0645\u064A\u0633", "\u062C\u0645\u0639\u0629", "\u0633\u0628\u062A"],
      longDays: ["\u0627\u0644\u0623\u062D\u062F", "\u0627\u0644\u0625\u062B\u0646\u064A\u0646", "\u0627\u0644\u062B\u0644\u0627\u062B\u0627\u0621", "\u0627\u0644\u0623\u0631\u0628\u0639\u0627\u0621", "\u0627\u0644\u062E\u0645\u064A\u0633", "\u0627\u0644\u062C\u0645\u0639\u0629", "\u0627\u0644\u0633\u0628\u062A"],
      title: (month, year) => `${month} ${year}`,
      full: (day, month, year, weekday2) => `${weekday2}\u060C ${day} ${month} ${year}`,
      am: "\u0635",
      pm: "\u0645"
    }
  };
  function clockText({ hour, minute, second }, words2, hourCycle) {
    const rest = `:${pad(minute)}${second === void 0 ? "" : `:${pad(second)}`}`;
    if (hourCycle !== "h12") return `${pad(hour)}${rest}`;
    return `${hour % 12 || 12}${rest} ${hour < 12 ? words2.am : words2.pm}`;
  }
  function fieldText(kind, value, { seconds, language, hourCycle }) {
    if (kind === "date") {
      const format = dateFormat(language);
      const date = parseDate(value);
      const sizeText2 = [...new Set(Array.from({ length: 12 }, (_, month) => format({ year: 2e3, month: month + 1, day: 10 }).replace(/\d/g, "0")))].join("\n");
      return { displayValue: date ? format(date) : value, sizeText: sizeText2 };
    }
    const words2 = wordsFor(language);
    const shown = parseTime(value);
    const widest = (second) => [words2.am, words2.pm].map((period) => clockText({ hour: 12, minute: 0, second }, { am: period, pm: period }, hourCycle).replace(/\d/g, "0"));
    const sizeText = [...new Set(widest(seconds || shown?.second !== void 0 ? 0 : void 0))].join("\n");
    return { displayValue: shown ? clockText(shown, words2, hourCycle) : value, sizeText };
  }
  var shortMonths = {
    en: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    fr: ["janv.", "f\xE9vr.", "mars", "avr.", "mai", "juin", "juil.", "ao\xFBt", "sept.", "oct.", "nov.", "d\xE9c."],
    es: ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic"],
    pt: ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."],
    ru: ["\u044F\u043D\u0432.", "\u0444\u0435\u0432\u0440.", "\u043C\u0430\u0440.", "\u0430\u043F\u0440.", "\u043C\u0430\u044F", "\u0438\u044E\u043D.", "\u0438\u044E\u043B.", "\u0430\u0432\u0433.", "\u0441\u0435\u043D\u0442.", "\u043E\u043A\u0442.", "\u043D\u043E\u044F\u0431.", "\u0434\u0435\u043A."]
  };
  var monthFirst = /* @__PURE__ */ new Set(["US", "CA", "PH"]);
  function dateFormat(language) {
    const parts2 = String(language ?? "").split(/[-_]/);
    const lang = parts2[0].toLowerCase();
    const region = parts2.slice(1).find((part) => /^[A-Za-z]{2}$|^\d{3}$/.test(part))?.toUpperCase();
    switch (lang) {
      case "zh":
        return ({ year, month, day }) => `${year}\u5E74${month}\u6708${day}\u65E5`;
      case "ja":
        return ({ year, month, day }) => `${year}/${pad(month)}/${pad(day)}`;
      case "ko":
        return ({ year, month, day }) => `${year}. ${month}. ${day}.`;
      case "de":
        return ({ year, month, day }) => `${pad(day)}.${pad(month)}.${year}`;
      case "fr":
        return ({ year, month, day }) => `${day} ${shortMonths.fr[month - 1]} ${year}`;
      case "es":
        return ({ year, month, day }) => `${day} ${shortMonths.es[month - 1]} ${year}`;
      case "pt":
        return ({ year, month, day }) => `${day} de ${shortMonths.pt[month - 1]} de ${year}`;
      case "ru":
        return ({ year, month, day }) => `${day} ${shortMonths.ru[month - 1]} ${year} \u0433.`;
      case "ar":
        return ({ year, month, day }) => `${day}\u200F/${month}\u200F/${year}`;
      default:
        return lang === "en" && region !== void 0 && !monthFirst.has(region) ? ({ year, month, day }) => `${day} ${shortMonths.en[month - 1]} ${year}` : ({ year, month, day }) => `${shortMonths.en[month - 1]} ${day}, ${year}`;
    }
  }
  function wordsFor(language) {
    const table = names2[String(language ?? "").split(/[-_]/)[0].toLowerCase()] ?? names2.en;
    const w = (key) => word(`@view.${key}`, language);
    return {
      ...table,
      cancel: w("cancel"),
      done: w("done"),
      previous: w("previousMonth"),
      next: w("nextMonth"),
      hour: w("hour"),
      minute: w("minute"),
      second: w("second"),
      period: w("period"),
      options: w("options")
    };
  }
  var pad = (value, width = 2) => String(value).padStart(width, "0");
  var isLeap = (year) => year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
  var daysIn = (year, month) => [31, isLeap(year) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
  function weekday(year, month, day) {
    const t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    const y = month < 3 ? year - 1 : year;
    return (y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) + t[month - 1] + day) % 7;
  }
  var iso2 = ({ year, month, day }) => `${pad(year, 4)}-${pad(month)}-${pad(day)}`;
  function parseDate(value) {
    const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value ?? "");
    if (!match) return null;
    const [year, month, day] = match.slice(1).map(Number);
    return month >= 1 && month <= 12 && day >= 1 && day <= daysIn(year, month) ? { year, month, day } : null;
  }
  function parseTime(value) {
    const match = /^(\d{2}):(\d{2})(?::(\d{2}))?$/.exec(value ?? "");
    if (!match) return null;
    const [hour, minute] = match.slice(1, 3).map(Number);
    if (hour > 23 || minute > 59) return null;
    if (match[3] === void 0) return { hour, minute };
    const second = Number(match[3]);
    return second < 60 ? { hour, minute, second } : null;
  }
  function addDays({ year, month, day }, days) {
    let ordinal = toOrdinal(year, month, day) + days;
    return fromOrdinal(ordinal);
  }
  function addMonths({ year, month, day }, months) {
    const index = year * 12 + (month - 1) + months;
    const y = Math.floor(index / 12), m = index - y * 12 + 1;
    return { year: y, month: m, day: Math.min(day, daysIn(y, m)) };
  }
  function toOrdinal(year, month, day) {
    const y = month <= 2 ? year - 1 : year, m = month <= 2 ? month + 9 : month - 3;
    return 365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) + Math.floor((153 * m + 2) / 5) + day - 1;
  }
  function fromOrdinal(ordinal) {
    let y = Math.floor((1e4 * ordinal + 14780) / 3652425);
    let rest = ordinal - (365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400));
    if (rest < 0) {
      y -= 1;
      rest = ordinal - (365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400));
    }
    const mi = Math.floor((100 * rest + 52) / 3060);
    const month = (mi + 2) % 12 + 1, year = y + Math.floor((mi + 2) / 12), day = rest - Math.floor((mi * 306 + 5) / 10) + 1;
    return { year, month, day };
  }
  var compare = (a, b) => a.year - b.year || a.month - b.month || a.day - b.day;
  function monthGrid({ year, month }, firstWeekday = 7) {
    const lead = (weekday(year, month, 1) - firstWeekday % 7 + 7) % 7;
    const start = addDays({ year, month, day: 1 }, -lead);
    return Array.from({ length: 42 }, (_, index) => {
      const date = addDays(start, index);
      return { date, inMonth: date.month === month && date.year === year };
    });
  }
  var weekdayOrder = (firstWeekday) => Array.from({ length: 7 }, (_, index) => (firstWeekday + index) % 7);
  function moveDate(date, key, rtl = false) {
    switch (key) {
      case "ArrowLeft":
        return addDays(date, rtl ? 1 : -1);
      case "ArrowRight":
        return addDays(date, rtl ? -1 : 1);
      case "ArrowUp":
        return addDays(date, -7);
      case "ArrowDown":
        return addDays(date, 7);
      case "PageUp":
        return addMonths(date, -1);
      case "PageDown":
        return addMonths(date, 1);
      default:
        return null;
    }
  }
  var within = (date, min, max) => (!min || compare(date, min) >= 0) && (!max || compare(date, max) <= 0);
  var clampDate = (date, min, max) => min && compare(date, min) < 0 ? min : max && compare(date, max) > 0 ? max : date;
  function hourValues(cycle) {
    return cycle === "h12" ? [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] : Array.from({ length: 24 }, (_, hour) => hour);
  }
  function minuteValues(step) {
    const every = Number.isFinite(step) && step >= 60 ? Math.max(1, Math.round(step / 60)) : 1;
    return Array.from({ length: Math.ceil(60 / every) }, (_, index) => index * every).filter((minute) => minute < 60);
  }
  function secondValues(step) {
    if (!Number.isFinite(step) || step <= 0 || step >= 60) return null;
    const every = Math.max(1, Math.round(step));
    return Array.from({ length: Math.ceil(60 / every) }, (_, index) => index * every).filter((second) => second < 60);
  }
  var timeIso = ({ hour, minute, second }) => `${pad(hour)}:${pad(minute)}${second === void 0 ? "" : `:${pad(second)}`}`;

  // src/runtime/webview.js
  var fields2 = ["url", "title", "error", "fullscreen"];
  var empty2 = Object.freeze({ url: "", title: "", error: null, fullscreen: false });
  var sessions = /* @__PURE__ */ new WeakMap();
  var readers2 = /* @__PURE__ */ new Map();
  var latest3 = /* @__PURE__ */ new Map();
  var latestSource = /* @__PURE__ */ new Map();
  var latestLabel = /* @__PURE__ */ new Map();
  var NAME2 = /^[A-Za-z0-9_.-]{1,64}$/;
  var bareSessionMark = Symbol("bare fullscreen webview session");
  var fullscreenSession = null;
  function resetWebViews() {
    fullscreenSession = null;
    latest3.clear();
    latestSource.clear();
    latestLabel.clear();
  }
  var webSessionName = (handle2) => sessions.get(handle2);
  var same3 = (a, b) => Object.is(a, b) || typeof a === "object" && typeof b === "object" && a !== null && b !== null && JSON.stringify(a) === JSON.stringify(b);
  function updateStatus(name, patch) {
    const prev = latest3.get(name) ?? empty2;
    const session = `name:${name}`;
    const next = Object.freeze({
      url: patch.url !== void 0 ? patch.url : prev.url,
      title: patch.title !== void 0 ? patch.title : prev.title,
      error: patch.error !== void 0 ? patch.error ? Object.freeze({ ...patch.error }) : null : prev.error,
      fullscreen: fullscreenSession === session
    });
    latest3.set(name, next);
    const named = readers2.get(name);
    for (const reader of named ?? []) {
      const { key, cell } = cellOf(reader.binding);
      if (page.locals.get(key) !== cell) {
        named.delete(reader);
        continue;
      }
      const current2 = reader.binding.value;
      if (page.updates.has(cell) || [...reader.read].some((field2) => !same3(current2[field2], next[field2]))) reader.binding.set(next);
      else cell.value = next;
    }
  }
  function setWebFullscreen(session) {
    if (fullscreenSession === session) return;
    const touched = [fullscreenSession, session].filter((s) => s !== null);
    fullscreenSession = session;
    page.invalidated = true;
    for (const each of touched) {
      if (each.startsWith("name:")) {
        const name = each.slice(5);
        const prev = latest3.get(name) ?? empty2;
        const next = Object.freeze({ ...prev, fullscreen: fullscreenSession === each });
        latest3.set(name, next);
        for (const reader of readers2.get(name) ?? []) {
          const { key, cell } = cellOf(reader.binding);
          if (page.locals.get(key) === cell) reader.binding.set(next);
        }
      }
      for (const handler of page.committed?.webFullscreenHandlers?.get(each) ?? []) {
        try {
          handler(Object.freeze({ fullscreen: fullscreenSession === each }));
        } catch (error) {
          report(sited(error));
        }
      }
    }
  }
  function useWebView(name) {
    if (typeof name !== "string" || !NAME2.test(name))
      throw Error('UI_WEBVIEW: useWebView takes a name of 1..64 letters, digits, "_", "." or "-"');
    const session = `name:${name}`;
    const binding = local(name, latest3.get(name) ?? empty2, "webview");
    const { cell } = cellOf(binding);
    let reader = cell.webReader;
    if (!reader) {
      reader = cell.webReader = { binding, read: /* @__PURE__ */ new Set() };
      if (!readers2.has(name)) readers2.set(name, /* @__PURE__ */ new Set());
      readers2.get(name).add(reader);
    }
    reader.read.clear();
    const handle2 = {};
    for (const field2 of fields2) Object.defineProperty(handle2, field2, { enumerable: true, get() {
      if (render.active) reader.read.add(field2);
      return field2 === "fullscreen" ? fullscreenSession === session : binding.value[field2];
    } });
    Object.assign(handle2, {
      requestFullscreen: () => {
        outsideRender("webview command");
        setWebFullscreen(session);
        return Promise.resolve();
      },
      exitFullscreen: () => {
        outsideRender("webview command");
        if (fullscreenSession === session) setWebFullscreen(null);
        return Promise.resolve();
      }
    });
    Object.freeze(handle2);
    sessions.set(handle2, name);
    return handle2;
  }
  function shownWebInDialog(session) {
    var _a;
    ((_a = render.candidate).webFullscreenDrawn ?? (_a.webFullscreenDrawn = /* @__PURE__ */ new Set())).add(session);
  }
  function webFullscreenDrawn() {
    return fullscreenSession !== null && render.candidate.webFullscreenDrawn?.has(fullscreenSession) === true;
  }
  function resolveBareWeb(resolve) {
    const kept2 = render.candidate.webFullscreenDrawn;
    render.candidate.webFullscreenDrawn = new Set(kept2);
    try {
      return resolve();
    } finally {
      render.candidate.webFullscreenDrawn = kept2;
    }
  }
  function BareWebFullscreen({ session }) {
    const src = render.candidate.webSources?.get(session) ?? latestSource.get(session);
    const label2 = latestLabel.get(session) ?? "Full screen web page";
    if (!src) return null;
    return h("dialog", {
      open: true,
      closedby: "closerequest",
      onClose: () => {
        if (fullscreenSession === session) setWebFullscreen(null);
      },
      "aria-label": label2,
      "data-testid": "lx-fullscreen-webview",
      style: {
        width: "100%",
        height: "100%",
        maxWidth: "100%",
        maxHeight: "100%",
        marginTop: 0,
        marginBottom: 0,
        marginLeft: 0,
        marginRight: 0,
        paddingTop: 0,
        paddingBottom: 0,
        paddingLeft: 0,
        paddingRight: 0,
        borderRadius: 0,
        backgroundColor: "surface"
      }
    }, h("webview", {
      [bareSessionMark]: session,
      src,
      "aria-label": label2,
      style: { width: "100%", flexGrow: 1 }
    }));
  }
  function webFullscreenFallback() {
    if (fullscreenSession === null || webFullscreenDrawn()) return null;
    return h(BareWebFullscreen, { session: fullscreenSession, key: `lx-web-fullscreen:${fullscreenSession}` });
  }
  function resolveWebView(props, identity2, parent, wire, on) {
    var _a, _b;
    const bareSession = props[bareSessionMark];
    const name = props.web === void 0 ? void 0 : webSessionName(props.web);
    if (props.web !== void 0 && name === void 0)
      throw Error("UI_WEBVIEW: web takes the handle useWebView(name) returns");
    const session = bareSession ?? (name === void 0 ? `el:${identity2}` : `name:${name}`);
    const candidateSources = (_a = render.candidate).webSources ?? (_a.webSources = /* @__PURE__ */ new Map());
    if (props.src !== void 0) {
      candidateSources.set(session, props.src);
      latestSource.set(session, props.src);
    }
    const src = props.src ?? candidateSources.get(session) ?? latestSource.get(session);
    if (src === void 0)
      throw Error("UI_WEBVIEW: a <webview> needs src, or a web handle another <webview> gives one");
    latestLabel.set(session, props.accessibilityLabel);
    wire("webSrc", src);
    wire("webSession", session);
    if (props.handlesDrags) wire("webHandlesDrags", 1);
    if (parent.inDialog && bareSession === void 0) shownWebInDialog(session);
    if (props.onFullscreenChange !== void 0) {
      const handlers = (_b = render.candidate).webFullscreenHandlers ?? (_b.webFullscreenHandlers = /* @__PURE__ */ new Map());
      if (!handlers.has(session)) handlers.set(session, []);
      handlers.get(session).push(props.onFullscreenChange);
    }
    if (bareSession !== void 0) return;
    on("onWebEvent", (payload2) => {
      const { events } = JSON.parse(payload2);
      if (!Array.isArray(events)) return;
      for (const event of events) {
        try {
          switch (event.type) {
            case "load":
              if (name !== void 0) updateStatus(name, { url: event.url, title: event.title, error: null });
              props.onLoad?.(Object.freeze({ url: event.url, title: event.title }));
              break;
            case "title":
              if (name !== void 0) updateStatus(name, { title: event.title });
              break;
            case "message":
              props.onMessage?.(Object.freeze({ data: JSON.parse(event.json), origin: event.origin }));
              break;
            case "blocked":
              props.onBlocked?.(Object.freeze({ url: event.url }));
              break;
            case "error":
              if (name !== void 0) updateStatus(name, { error: { code: event.code, message: event.message } });
              props.onError?.(Object.freeze({ code: event.code, message: event.message }));
              break;
            case "fullscreenchange": {
              const next = Boolean(event.fullscreen);
              if (next) setWebFullscreen(session);
              else if (fullscreenSession === session) setWebFullscreen(null);
              break;
            }
          }
        } catch (error) {
          report(sited(error));
        }
      }
    }, "text", true);
  }

  // src/runtime/elements.js
  var videoFlags = [["videoMuted", "muted"], ["videoLoop", "loop"]];
  var videoEvents = [
    ["loadedmetadata", "onLoadedMetadata"],
    ["play", "onPlay"],
    ["playing", "onPlaying"],
    ["pause", "onPause"],
    ["waiting", "onWaiting"],
    ["seeked", "onSeeked"],
    ["timeupdate", "onTimeUpdate"],
    ["ended", "onEnded"],
    ["error", "onError"],
    ["ratechange", "onRateChange"],
    ["volumechange", "onVolumeChange"],
    ["qualitychange", "onQualityChange"]
  ];
  var common2 = new Set(commonProps);
  var semanticStates = { pressed: "statePressed", selected: "stateSelected", expanded: "stateExpanded", busy: "stateBusy", invalid: "stateInvalid", required: "stateRequired", checked: "stateChecked" };
  var accepts = (field2, tag) => admission.fields[field2].includes(kindOf(tag).wire);
  var ignorable2 = new Set(ignorableFields);
  var warned = /* @__PURE__ */ new Set();
  function dropped(key) {
    const host = globalThis.__lingxiaViewHost;
    if (!ignorable2.has(key) || !host || field[key] <= host.maxField) return false;
    if (!warned.has(key) && typeof globalThis.__lingxiaWarn === "function") {
      warned.add(key);
      globalThis.__lingxiaWarn(`UI_COMPAT: the native host predates ${key}; it is left out of this page's semantics`);
    }
    return true;
  }
  var warnedPicker = /* @__PURE__ */ new Set();
  function warnNoPicker(identity2, tag, keyboard) {
    if (warnedPicker.has(identity2) || typeof globalThis.__lingxiaWarn !== "function") return;
    warnedPicker.add(identity2);
    globalThis.__lingxiaWarn(`UI_INPUT: showPicker() does nothing on <${tag}${keyboard ? ` type="${keyboard}"` : ""}>; only a date or time editor opens a picker`);
  }
  var interactiveTags = /* @__PURE__ */ new Set(["button", "link", "checkbox", "switch", "radio", "input", "textarea", "select", "slider", "webview"]);
  var bindCodes = { input: "UI_INPUT", textarea: "UI_INPUT", checkbox: "UI_CHECK", switch: "UI_CHECK", slider: "UI_SLIDER", radio: "UI_RADIO", select: "UI_SELECT" };
  var optionStyle = ["fontSize", "fontWeight", "fontFamily", "lineHeight", "color", "background"];
  var componentIds = /* @__PURE__ */ new WeakMap();
  var componentNames = /* @__PURE__ */ new Map();
  var nextHandle = 1;
  function componentId(tag) {
    if (typeof tag.__lxid === "string") return tag.__lxid;
    let id = componentIds.get(tag);
    if (id !== void 0) return id;
    const name = tag.name || "anonymous";
    const count = (componentNames.get(name) ?? 0) + 1;
    componentNames.set(name, count);
    id = count === 1 ? name : `${name}#${count}`;
    componentIds.set(tag, id);
    return id;
  }
  var handle = () => nextHandle++;
  var handlesIssued = () => nextHandle;
  var skipHandle = () => {
    nextHandle++;
  };
  var pointerCrossing = ["onPointerEnter", "onPointerLeave"];
  var boxTags = /* @__PURE__ */ new Set(["view", "text", "button", "vector", "plot", "scroll"]);
  var editorKeys = /* @__PURE__ */ new Set(["ArrowUp", "ArrowDown", "Enter", "Escape", "Home", "End", "PageUp", "PageDown"]);
  var pointerTags = /* @__PURE__ */ new Set(["view", "button", "vector"]);
  var pointerAxes = /* @__PURE__ */ new Set(["x", "y", "both"]);
  var callbackOf = (callback, kind = "void", isolated = false, press = void 0, site = void 0) => ({ callback, kind, isolated, press, site });
  var dataProp = /^data-[a-z][a-z0-9-]*$/;
  function dataOf(props) {
    let data;
    for (const key in props) {
      const value = props[key];
      if (!key.startsWith("data-") || !dataProp.test(key) || value === false || value == null) continue;
      (data ?? (data = {}))[key.slice(5)] = value === true ? "" : String(value);
    }
    return data;
  }
  var linkKeys = ["page", "tab", "app", "url", "back"];
  function linkTarget(to, props) {
    if (props.onClick !== void 0 || props.popoverTarget !== void 0) throw Error("UI_PROP: a link takes to, onClick or popoverTarget, not two");
    if (!to || typeof to !== "object" || Array.isArray(to)) throw Error("UI_PROP: to is a target object");
    const named = linkKeys.filter((key2) => to[key2] !== void 0 && !(key2 === "page" && to.app !== void 0));
    if (named.length !== 1) throw Error("UI_PROP: to names exactly one of page, tab, app, url or back");
    const [key] = named;
    if (key === "back" ? !Number.isInteger(to.back) || to.back < 1 : typeof to[key] !== "string" || !to[key])
      throw Error(`UI_PROP: to.${key} must be ${key === "back" ? "a positive integer" : "a non-empty string"}`);
    return to;
  }
  function admit2(tag, element, props) {
    let data = 0;
    for (const key in props) {
      if (common2.has(key) || has(element.props, key)) continue;
      const value = props[key];
      if (dataProp.test(key)) {
        if (!["string", "number", "boolean"].includes(typeof value) && value != null) throw Error(`UI_PROP: ${key} must be a string, number or boolean`);
        if (typeof value === "string" && value.length > 256) throw Error(`UI_PROP: ${key} exceeds 256 characters`);
        if (++data > 16) throw Error("UI_PROP: more than 16 data-* props");
        continue;
      }
      if (element.binding && (key === "bind" || key === "value" || key === element.binding.event || key === element.binding.end)) continue;
      throw Error(`UI_PROP: ${key} is not a property of <${tag}>`);
    }
    if (props.key !== void 0 && typeof props.key !== "string") throw Error("UI_PROP: key must be a string");
    if (props["data-testid"] !== void 0 && typeof props["data-testid"] !== "string") throw Error("UI_PROP: data-testid must be a string");
    if (props.disabled !== void 0 && typeof props.disabled !== "boolean") throw Error("UI_PROP: disabled");
    if (props.style !== void 0 && (!props.style || typeof props.style !== "object" || Array.isArray(props.style))) throw Error("UI_PROP: style");
  }
  function bind(tag, element, props) {
    const { type, event } = element.binding;
    const code = bindCodes[tag];
    if (props.bind !== void 0) {
      const cell = cellOf(props.bind);
      if (!cell || render.candidate.locals.get(cell.key) !== cell.cell || typeof props.bind.value !== type || props.value !== void 0)
        throw Error(`${code}: bind requires a mounted ${type} useLocal cell without value`);
      if (props[event] !== void 0 && typeof props[event] !== "function") throw Error(`${code}: ${event}`);
      const binding = props.bind, heard = props[event];
      props.value = binding.value;
      props[event] = heard ? (value) => {
        binding.set(value);
        heard(value);
      } : (value) => binding.set(value);
    }
    if (props[event] !== void 0 && typeof props[event] !== "function") throw Error(`${code}: ${event}`);
  }
  var lowered = /* @__PURE__ */ new Map();
  var loweredEnv = null;
  var loweredClasses = classGeneration;
  var fresh = /* @__PURE__ */ new Set();
  var seeded = null;
  var loweringFields = ["appearance", "viewport", "pointer", "reducedMotion", "insets", "keyboard", "sizeClass"];
  var loweringEnv = (env, scoped) => {
    const base = Object.fromEntries(loweringFields.map((name) => [name, env?.[name]]));
    if (scoped) base.sizeClass = scoped;
    return base;
  };
  var keyedEnv = {};
  var keyedEnvKey;
  function envKey(env) {
    if (env !== keyedEnv) {
      keyedEnv = env;
      keyedEnvKey = JSON.stringify(loweringFields.map((name) => env?.[name]));
    }
    return keyedEnvKey;
  }
  function visibleEvent({ items = [], timeStamp }, keys2) {
    const shown = items.map(({ id, index, ratio, top }) => Object.freeze(index === void 0 ? { id: String(id), ratio, top } : { id: keys2 ? String(keys2[index]) : String(index), index, ratio, top }));
    let current2 = shown[0];
    for (const item of shown) if (item.top <= 0) current2 = item;
    return Object.freeze({
      items: Object.freeze(shown),
      ...current2 ? { current: current2.id } : {},
      timeStamp: typeof timeStamp === "number" ? timeStamp : Date.now()
    });
  }
  function takeLowered() {
    if (!fresh.size) return "";
    fresh.clear();
    if (lowered.size > 400) return "";
    return JSON.stringify({ env: loweredEnv, entries: [...lowered].filter(([, style2]) => portable(style2)) });
  }
  var droppable = /* @__PURE__ */ new Set(["children", "every", "release", "descendants"]);
  function portable(style2) {
    return Object.entries(style2).every(([name, value]) => value === void 0 ? droppable.has(name) : name === "typography" ? Object.entries(value).every(([field2, size]) => size === void 0 ? field2 === "lineHeight" : plainJson(size)) : plainJson(value));
  }
  function plainJson(value) {
    if (value === null || typeof value === "string" || typeof value === "boolean") return true;
    if (typeof value === "number") return Number.isFinite(value) && !Object.is(value, -0);
    if (Array.isArray(value)) return value.every(plainJson);
    if (typeof value !== "object" || Object.getPrototypeOf(value) !== Object.prototype) return false;
    return Object.values(value).every(plainJson);
  }
  function seedLowered(text) {
    let value;
    try {
      value = JSON.parse(text);
    } catch {
      return false;
    }
    const entry = (item) => Array.isArray(item) && item.length === 2 && typeof item[0] === "string" && item[1] !== null && typeof item[1] === "object";
    if (typeof value?.env !== "string" || !Array.isArray(value.entries) || !value.entries.every(entry)) return false;
    seeded = value;
    if (loweredClasses !== classGeneration) {
      loweredClasses = classGeneration;
      loweredEnv = null;
    }
    if (value.env === loweredEnv) {
      for (const [key, style2] of value.entries) if (!lowered.has(key)) lowered.set(key, style2);
    }
    return true;
  }
  var warnedClamp = /* @__PURE__ */ new Set();
  function resetStyleWarnings() {
    warnedClamp.clear();
  }
  function nodeStyle(tag, props, data, extra, inherited2, direction, structure2, every, sizeClass, identity2) {
    if (typeof props.className === "string") noteEnv(classEnvFields(props.className));
    if (usesViewportUnits(props.style) || usesViewportUnits(extra) || usesViewportUnits(every)) noteEnv(VIEWPORT_FIELDS);
    if (props.style !== void 0 || extra !== void 0 || every !== void 0 || props.className === void 0) return lowerNodeStyle(tag, props, data, extra, inherited2, direction, structure2, every, sizeClass, identity2);
    const environment = envKey(envOf(render.view));
    if (loweredClasses !== classGeneration) {
      loweredClasses = classGeneration;
      seeded = null;
      loweredEnv = null;
    }
    if (environment !== loweredEnv) {
      lowered.clear();
      fresh.clear();
      loweredEnv = environment;
      if (seeded?.env === environment) for (const [key2, style3] of seeded.entries) lowered.set(key2, style3);
    }
    const key = `${tag}\0${props.className}\0${data ? JSON.stringify(data) : ""}\0${props.disabled === true}\0${mixedCheckbox(tag, props)}\0${direction}\0${sizeClass ?? ""}\0${(structure2 ?? []).join(",")}\0${inherited2.fontSize}\0${JSON.stringify(inherited2.lineHeight)}`;
    let style2 = lowered.get(key);
    if (!style2) {
      style2 = lowerNodeStyle(tag, props, data, extra, inherited2, direction, structure2, void 0, sizeClass, identity2);
      fresh.add(key);
      if (lowered.size >= 2048) lowered.delete(lowered.keys().next().value);
      lowered.set(key, style2);
    } else if (style2.clampWarnings?.length && typeof globalThis.__lingxiaWarn === "function") {
      for (const [name, message] of style2.clampWarnings) {
        const warnKey = `${identity2 ?? ""}\0${name}`;
        if (warnedClamp.has(warnKey)) continue;
        warnedClamp.add(warnKey);
        globalThis.__lingxiaWarn(message);
      }
    }
    return { ...style2, fields: { ...style2.fields } };
  }
  function authoredClass(className, key, env) {
    for (const token of className.split(/\s+/).filter(Boolean)) {
      const one = resolveClasses(token, env);
      if (one?.base && key in one.base || one?.disabled && key in one.disabled) return token;
    }
    return className;
  }
  var fieldTags = /* @__PURE__ */ new Set(["input", "textarea", "select"]);
  var runTags = /* @__PURE__ */ new Set(["span", "link"]);
  var disabledField = { color: "text-muted", backgroundColor: "surface-sunken", borderColor: "line" };
  var videoBacking = { backgroundColor: "#000000" };
  var mixedCheckbox = (tag, props) => tag === "checkbox" && props.indeterminate === true;
  function lowerNodeStyle(tag, props, data, extra, inherited2, direction, structure2, every, sizeClass, identity2) {
    const env = { ...loweringEnv(envOf(render.view), sizeClass), direction, structure: structure2, data, indeterminate: mixedCheckbox(tag, props) };
    const classes = resolveClasses(props.className, env);
    if (classes?.pairedLeading && runTags.has(tag)) delete classes.base.lineHeight;
    const disabled = props.disabled === true;
    const muted = disabled && !classes?.disabled && fieldTags.has(tag) ? disabledField : void 0;
    const base = disabled && classes?.disabled ? classes.disabled : classes?.base;
    const stray = base && Object.keys(base).find((key) => !kindOf(tag).style.includes(key) && !(props.style && key in props.style));
    if (stray) throw Error(classElementProblem(authoredClass(props.className, stray, env), stray, tag));
    const author = mergeStyle(direction, tag === "video" ? videoBacking : void 0, extra, base, every, muted, props.style);
    const flexOnly = author?.display === "grid" && base && gridRefuses(Object.keys(base).filter((key) => !(props.style && key in props.style)));
    if (flexOnly) throw Error(gridProblem(authoredClass(props.className, flexOnly, env), flexOnly));
    if ((tag === "view" || tag === "page") && (author?.display === "flex" || author?.display === "inline-flex") && author.flexDirection === void 0)
      author.flexDirection = "row";
    const clampWarnings = [];
    const warn = typeof globalThis.__lingxiaWarn === "function" ? (name, message) => {
      clampWarnings.push([name, message]);
      const key = `${identity2 ?? ""}\0${name}`;
      if (warnedClamp.has(key)) return;
      warnedClamp.add(key);
      globalThis.__lingxiaWarn(message);
    } : void 0;
    const { fields: fields3, typography } = lowerStyle(tag, author, env, inherited2, warn);
    const states2 = {};
    const state = (name, wire, allowed, requirement) => {
      const delta = classes?.[name];
      if (!delta || !Object.keys(delta).length) return;
      if (!allowed) throw Error(`UI_STYLE: ${name}: requires ${requirement}, not <${tag}>`);
      paintOnly(name, tag, delta);
      states2[wire] = lowerStyle(tag, delta, env, { fontSize: typography.fontSize }).fields;
    };
    state("focus", "focusStyle", accepts("focusStyle", tag), "an editor or <select>");
    state("focusVisible", "focusVisibleStyle", accepts("focusVisibleStyle", tag), "an element that takes focus");
    if (!disabled) {
      state("active", "pressedStyle", accepts("pressedStyle", tag), "a control");
      state("hover", "hoverStyle", true);
    }
    state("starting", "startingStyle", true);
    state("exit", "exitStyle", true);
    const presence = props[presenceStyle];
    if (presence) for (const [name, wire] of [["starting", "startingStyle"], ["exit", "exitStyle"]]) {
      if (!presence[name] || !Object.keys(presence[name]).length) continue;
      paintOnly(`backdrop:${name}`, tag, presence[name]);
      states2[wire] = lowerStyle(tag, presence[name], env, { fontSize: typography.fontSize }).fields;
    }
    state("focusWithin", "focusWithinStyle", true);
    state("groupActive", "groupPressedStyle", true);
    for (const side of ["Top", "Bottom", "Start", "End"]) {
      const delta = classes?.[`side${side}`];
      if (!delta || !Object.keys(delta).length) continue;
      paintOnly(`side-${side.toLowerCase()}`, tag, delta);
      states2[`side${side}Style`] = lowerStyle(tag, delta, env, { fontSize: typography.fontSize }).fields;
    }
    const checked2 = classes?.checked && (disabled ? classes.checked.disabled : classes.checked.base);
    if (checked2 && Object.keys(checked2).length) {
      if (!accepts("checkedStyle", tag)) throw Error(`UI_STYLE: checked: requires a <checkbox>, <switch> or radio option, not <${tag}>`);
      paintOnly("checked", tag, checked2);
      states2.checkedStyle = lowerStyle(tag, checked2, env, { fontSize: typography.fontSize }).fields;
    }
    if (classes?.hasChecked && Object.keys(classes.hasChecked).length) {
      paintOnly("has-checked", tag, classes.hasChecked);
      states2.hasCheckedStyle = lowerStyle(tag, classes.hasChecked, env, { fontSize: typography.fontSize }).fields;
    }
    if (classes?.backdrop && !props[backdropOwner]) throw Error(`UI_STYLE: backdrop: styles the backdrop of a <dialog> or a light-dismiss popover, not <${tag}>`);
    const hasDisabled = classes?.hasDisabled && Object.keys(classes.hasDisabled).length ? classes.hasDisabled : void 0;
    if (hasDisabled) paintOnly("has-disabled", tag, hasDisabled);
    const descendants = hasDisabled ? lowerStyle(tag, hasDisabled, env, { fontSize: typography.fontSize }).fields : void 0;
    return { fields: fields3, states: states2, typography, author, children: classes?.children, every: classes?.every, descendants, release: classes?.release, ...clampWarnings.length ? { clampWarnings } : {} };
  }
  var listTag = Object.freeze({});
  function resolveNode(node, identity2, depth = 0, parent = {}) {
    if (depth > limits.depth) throw Error("UI_LIMIT: children too deep");
    if (node == null || typeof node === "boolean" || node === "") return null;
    if (typeof node === "string" || typeof node === "number") node = h("text", {}, node);
    if (Array.isArray(node)) return { fragment: entries(node) };
    if (isFragment(node)) return { fragment: entries([node]).map((entry) => entry.slot ? { ...entry, slot: entry.slot.slice(2) } : entry) };
    if (!isNode(node)) throw Error("UI_ROOT: expected one node");
    const record = readNode(node);
    const authored2 = record[0], original = record[1], children = record[2], location = record[3];
    if (authored2 === "list") throw located(Error("UI_TAG: <list> is not an element; use <List> from @lingxia/view"), `<list>${location ? ` (${location})` : ""}`);
    const tag = authored2 === listTag ? "list" : authored2;
    if (location) render.site = `${typeof tag === "function" ? tag.name || "<anonymous>" : `<${tag}>`} (${location})`;
    if (typeof tag === "function") {
      const previous = render.scope, scope = `${identity2}/component:${componentId(tag)}`;
      render.scope = scope;
      const supplied = children.length === 0 ? void 0 : children.length === 1 ? children[0] : children;
      try {
        if (isProvider(tag)) return provide(
          tag,
          original,
          children,
          parent.contexts,
          (child, contexts) => resolveNode(child, scope, depth + 1, { ...parent, contexts })
        );
        const recalled = recall(scope, tag, original, children, parent);
        if (recalled !== void 0) return recalled;
        const frame = enter(scope, parent.contexts);
        const outerContexts = render.contexts;
        let result;
        try {
          render.contexts = parent.contexts;
          const output = tag({ ...original, children: supplied });
          render.contexts = outerContexts;
          result = resolveNode(output, scope, depth + 1, parent);
        } finally {
          render.contexts = outerContexts;
          leave(frame);
        }
        remember(frame, tag, original, children, parent, result);
        return result;
      } catch (error) {
        throw located(error, `${tag.name || "<anonymous>"}${location ? ` (${location})` : ""}`);
      } finally {
        render.scope = previous;
      }
    }
    try {
      if (!original[panelMark]) authoredRules(tag, original, parent);
      let expansion = expand(tag, original, children, identity2, parent);
      if (expansion === null) return null;
      if (tag === "slider" && original.values !== void 0) expansion = rangeSlider(original);
      if (expansion) {
        const result = resolveNode(expansion.node, identity2, depth + 1, { ...parent, extra: void 0, every: void 0, sibling: void 0 });
        expansion.finish(result);
        return result;
      }
      return resolveElement(tag, original, children, identity2, depth, parent);
    } catch (error) {
      throw located(error, `<${tag}>${location ? ` (${location})` : ""}`);
    }
  }
  var rtlScripts = /[\u0590-\u08FF\uFB1D-\uFDFF\uFE70-\uFEFF]/u;
  function strongDirection(children, depth = 0) {
    for (const child of Array.isArray(children) ? children : [children]) {
      if (typeof child === "string") {
        const letter = child.match(/\p{L}/u)?.[0];
        if (letter) return rtlScripts.test(letter) ? "rtl" : "ltr";
      } else if (Array.isArray(child) && depth < 32) {
        const found = strongDirection(child, depth + 1);
        if (found) return found;
      } else if (isNode(child) && depth < 32) {
        const found = strongDirection(readNode(child)[2], depth + 1);
        if (found) return found;
      }
    }
    return void 0;
  }
  var ariaNames = [
    ["aria-label", "accessibilityLabel"],
    ["aria-hidden", "accessibilityHidden"],
    ["aria-live", "accessibilityLiveRegion"],
    ["aria-valuenow", "ariaValueNow"],
    ["aria-valuemin", "ariaValueMin"],
    ["aria-valuemax", "ariaValueMax"],
    ["aria-valuetext", "ariaValueText"],
    ["aria-level", "ariaLevel"],
    ["aria-modal", "ariaModal"]
  ];
  var ariaStates = [
    ["aria-pressed", "pressed"],
    ["aria-selected", "selected"],
    ["aria-expanded", "expanded"],
    ["aria-busy", "busy"],
    ["aria-invalid", "invalid"],
    ["aria-required", "required"],
    ["aria-checked", "checked"]
  ];
  function ariaProps(props) {
    for (let i = 0; i < ariaNames.length; i++) {
      const authored2 = ariaNames[i][0];
      if (!(authored2 in props)) continue;
      const value = props[authored2];
      delete props[authored2];
      if (authored2 === "aria-label" && typeof value === "string" && !value.trim()) continue;
      props[ariaNames[i][1]] = value;
    }
    let state;
    for (let i = 0; i < ariaStates.length; i++) {
      const authored2 = ariaStates[i][0];
      if (props[authored2] === void 0) continue;
      (state ?? (state = {}))[ariaStates[i][1]] = props[authored2];
      delete props[authored2];
    }
    if (state) props.accessibilityState = state;
  }
  function resolveElement(tag, original, children, identity2, depth, parent) {
    if (typeof tag !== "string" || !has(elements, tag)) throw Error(`UI_TAG: ${String(tag)}${tagHint(tag)}`);
    const block = tag === "link" && !parent.inText;
    const kind = block ? "link-box" : tag;
    const element = kindOf(kind);
    if (block && parent.inBlockLink) throw Error("UI_LINK: a block link cannot sit inside another block link");
    const props = { ...original };
    admit2(tag, element, props);
    if (props["data-testid"] !== void 0) {
      props.testId = props["data-testid"];
      delete props["data-testid"];
    }
    ariaProps(props);
    if (parent.inControl && (tag === "scroll" || tag === "list" || tag === "webview"))
      throw Error(`UI_BUTTON: a ${tag === "webview" ? "<webview>" : "scroll or list"} cannot sit inside a pressable control`);
    if (element.binding) bind(tag, element, props);
    const editor2 = tag === "input" || tag === "textarea";
    if (props.dir !== void 0 && !["ltr", "rtl", "auto"].includes(props.dir)) throw Error('UI_PROP: dir must be "ltr", "rtl" or "auto"');
    const own2 = props.dir === "auto" && !editor2 ? strongDirection(children) : props.dir === "auto" ? void 0 : props.dir;
    if (props.dir === "auto" && !editor2) props.dir = own2;
    if (own2 === void 0 && parent.direction === void 0) noteEnv("direction");
    const direction = own2 ?? parent.direction ?? (envOf(render.view)?.direction === "rtl" ? "rtl" : "ltr");
    const authored2 = entries(children), flat = authored2.map((entry) => entry.child);
    const data = dataOf(props);
    const rootFamily = !parent.typography && pageFamily && element.style.includes("fontFamily") ? { fontFamily: pageFamily } : void 0;
    const scopedSize = scopedSizeClass(parent.contexts, classesUseSizeClass(props.className));
    const style2 = nodeStyle(kind, props, data, parent.extra ?? rootFamily, parent.typography ?? { fontSize: 14 }, direction, structure(parent.sibling, flat), parent.every, scopedSize, identity2);
    const inline = (child) => typeof child === "string" || typeof child === "number";
    if (admission.tags[element.wire].parent === "text" && !parent.inText) throw Error(`UI_TEXT: <${tag}> belongs inside <text>; wrap it in a <text>`);
    if (tag === "link" && parent.inLink) throw Error("UI_TEXT: links do not nest");
    const composed = element.children === "content" && flat.some((child) => !inline(child));
    const runs = element.children === "runs" && flat.some((child) => !inline(child));
    const leaf = element.children === "text" || element.children === "content" && !composed || element.children === "runs" && !runs;
    if (leaf && flat.some((child) => typeof child !== "string" && typeof child !== "number")) throw Error(`UI_CHILD: <${tag}> accepts only text`);
    if (element.children === "none" && flat.length) throw Error(`UI_CHILD: <${tag}> takes no children`);
    const disabled = props.disabled === true;
    const result = {
      tag: tag === "input" && props.type === "search" ? "search" : element.wire,
      element: tag,
      props,
      fields: style2.fields,
      states: style2.states,
      typography: style2.typography,
      identity: identity2,
      text: leaf ? flat.join("") : "",
      wire: [],
      handlers: [],
      children: []
    };
    const wire = (key, value) => {
      if (!dropped(key)) result.wire.push([key, value]);
    };
    const on = (key, callback, kind2, isolated) => result.handlers.push([key, handle(), callbackOf(callback, kind2, isolated, void 0, [key, tag, props.testId])]);
    if (disabled) wire("enabled", 0);
    if (tag === "page" && props.title !== void 0) {
      if (typeof props.title !== "string") throw Error("UI_PROP: <page> title must be a string");
      wire("pageTitle", props.title);
    }
    if (props.bleed !== void 0 && props.bleed !== false) {
      if (parent.tag !== "page") {
        throw Error("UI_BLEED_PARENT: bleed is only allowed on direct children of <page>");
      }
      const mode = props.bleed === true ? "all" : props.bleed;
      if (!["all", "top", "bottom", "horizontal"].includes(mode)) {
        throw Error('UI_PROP: bleed must be boolean, "top", "bottom" or "horizontal"');
      }
      wire("bleed", mode);
    }
    if (data) wire("dataAttributes", JSON.stringify(data));
    if (props.dir !== void 0) wire("direction", props.dir);
    semantics(tag, props, wire);
    if (props.drag !== void 0) dragElement(tag, props, wire, on, style2.release, `${identity2}#drag`);
    if (tag === "scroll" && props.snapIndex !== void 0) {
      const { snapIndex, onSnapChange } = props;
      if (!Number.isInteger(snapIndex) || snapIndex < 0) throw Error("UI_PROP: snapIndex is a snap point index, an integer from 0");
      if (onSnapChange !== void 0 && typeof onSnapChange !== "function") throw Error("UI_PROP: onSnapChange takes a function");
      const key = `${identity2}#scroll`;
      wire("snapIndex", controlledSnap(key, snapIndex));
      if (onSnapChange) on("onSnapChange", (payload2) => {
        const index = Number(payload2);
        if (!Number.isInteger(index) || index < 0) return;
        snapSettled(key, index, snapIndex, index !== snapIndex ? () => onSnapChange(index) : void 0);
      }, "text");
    }
    if (tag === "scroll" && props.bind !== void 0) {
      const cell = props.bind, bound = cellOf(cell);
      if (!bound || render.candidate.locals.get(bound.key) !== bound.cell || !Number.isInteger(cell.value) || cell.value < 0)
        throw Error("UI_PROP: bind on a <scroll> takes a mounted useLocal cell holding a snap index");
      wire("snapIndex", cell.value);
      on("onSnapChange", (payload2) => {
        const index = Number(payload2);
        if (Number.isInteger(index) && index >= 0 && cell.value !== index) cell.set(index);
      }, "text");
    }
    if (props[scrimMark]) wire("dragScrim", 1);
    for (const key of pointerCrossing) {
      if (props[key] === void 0) continue;
      if (tag !== "view" && tag !== "button") throw Error(`UI_PROP: ${key} belongs to a <view> or <button>`);
      if (typeof props[key] !== "function") throw Error(`UI_PROP: ${key} takes a function`);
      on(key, props[key]);
    }
    if (props.onPointer !== void 0 || props.pointerAxis !== void 0) {
      if (!pointerTags.has(tag)) throw Error("UI_PROP: onPointer belongs to a <view>, <button> or <vector>");
      const { onPointer, pointerAxis = "both" } = props;
      if (onPointer === void 0) throw Error("UI_PROP: pointerAxis needs onPointer on the same element; add onPointer or remove pointerAxis");
      if (typeof onPointer !== "function") throw Error("UI_PROP: onPointer takes a function");
      if (!pointerAxes.has(pointerAxis)) throw Error('UI_PROP: pointerAxis must be "x", "y" or "both"');
      on("onPointer", (payload2) => onPointer(Object.freeze(JSON.parse(payload2))), "text");
      if (pointerAxis !== "both") wire("pointerAxis", pointerAxis);
    }
    if (props.onPage !== void 0) {
      const { onPage } = props;
      if (typeof onPage !== "function") throw Error("UI_PROP: onPage takes a function");
      on("onPage", (direction2) => onPage(direction2), "text");
    }
    if (props.onContextMenu !== void 0) {
      if (!["view", "button", "link", "text"].includes(tag)) throw Error("UI_PROP: onContextMenu belongs to a <view>, <button>, <link> or <text>");
      const { onContextMenu } = props;
      if (typeof onContextMenu !== "function") throw Error("UI_PROP: onContextMenu takes a function");
      on("onContextMenu", (payload2) => onContextMenu(Object.freeze(JSON.parse(payload2))), "text");
    }
    if (props.onLayout !== void 0 || props.box !== void 0) {
      if (!boxTags.has(tag)) throw Error("UI_PROP: onLayout and box belong to a <view>, <text>, <button>, <vector>, <plot> or <scroll>");
      const { onLayout } = props;
      if (onLayout !== void 0 && typeof onLayout !== "function") throw Error("UI_PROP: onLayout takes a function");
      const binding = props.box === void 0 ? void 0 : boxBinding(props.box);
      if (props.box !== void 0 && !binding) throw Error("UI_PROP: box takes the handle useBox(name) returns");
      on("onLayout", (payload2) => {
        const { width, height, minContentWidth, maxContentWidth } = JSON.parse(payload2);
        const size = { width, height, minContentWidth, maxContentWidth }, held2 = binding?.value;
        if (binding && !(held2 && Object.keys(size).every((key) => held2[key] === size[key]))) binding.set(size);
        onLayout?.(size);
      }, "text");
    }
    if (props["aria-actions"] !== void 0) {
      const ids = typeof props["aria-actions"] === "string" ? props["aria-actions"].trim().split(/\s+/) : [];
      if (!ids.length || ids.length > 8 || ids.some((id) => !id || id.length > 256)) throw Error("UI_PROP: aria-actions lists 1..8 element ids");
      wire("ariaActions", ids.join(" "));
    }
    if (props["aria-describedby"] !== void 0) {
      const ids = typeof props["aria-describedby"] === "string" ? props["aria-describedby"].trim().split(/\s+/) : [];
      if (!ids.length || ids.length > 8 || ids.some((id) => !id || id.length > 256)) throw Error("UI_PROP: aria-describedby lists 1..8 element ids");
      wire("ariaDescribedBy", ids.join(" "));
    }
    if (props.id !== void 0) {
      if (typeof props.id !== "string" || !props.id.length || props.id.length > 256 || /\s/.test(props.id)) throw Error("UI_ID: an id is 1..256 characters without spaces");
      wire("elementId", props.id);
    }
    if (tag === "dialog") {
      if (!props.accessibilityLabel) throw Error('UI_A11Y: <dialog> needs an accessible name; add aria-label="\u2026"');
      wire("role", props.role ?? "dialog");
      wire("modal", 1);
    }
    const focusEvents = () => {
      for (const name of ["onFocus", "onBlur"]) {
        if (props[name] === void 0) continue;
        if (typeof props[name] !== "function") throw Error(`UI_PROP: ${name} takes a function`);
        if (!disabled) on(name, props[name], "focus");
      }
    };
    switch (tag) {
      case "link": {
        label(props.accessibilityLabel, "UI_TEXT");
        if (props.onClick !== void 0 && typeof props.onClick !== "function") throw Error("UI_PROP: onClick");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        const to = props.to === void 0 ? void 0 : linkTarget(props.to, props);
        if (!to && props.onClick === void 0 && props.popoverTarget === void 0 && props.interestFor === void 0) throw Error("UI_LINK: a <link> needs to, onClick, popoverTarget or interestFor, disabled or not; disabled only stops the press");
        const click = invokerHandler(to ? { ...props, onClick: () => requestNavigation(to) } : props, identity2, parent.popovers ?? []);
        interestTrigger(props, identity2, parent.popovers ?? []);
        if (props.interestFor !== void 0) wire("interestFor", props.interestFor);
        if (click && !disabled) on("onClick", click, "press");
        focusEvents();
        break;
      }
      case "button": {
        label(props.accessibilityLabel, "UI_BUTTON");
        if (props.onClick !== void 0 && typeof props.onClick !== "function") throw Error("UI_PROP: onClick");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        const click = invokerHandler(props, identity2, parent.popovers ?? []);
        interestTrigger(props, identity2, parent.popovers ?? []);
        if (props.interestFor !== void 0) wire("interestFor", props.interestFor);
        if (props.keepsFocus !== void 0 && typeof props.keepsFocus !== "boolean") throw Error("UI_PROP: keepsFocus is a boolean");
        if (props.keepsFocus) wire("keepsFocus", 1);
        if (click && !disabled) on("onClick", click, "press");
        focusEvents();
        break;
      }
      case "checkbox":
      case "switch":
        label(props.accessibilityLabel, "UI_CHECK");
        if (props.value !== void 0 && typeof props.value !== "boolean") throw Error("UI_CHECK: value must be boolean");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        if (props.value !== void 0) wire("checked", +props.value);
        if (props.defaultChecked !== void 0) {
          if (typeof props.defaultChecked !== "boolean") throw Error("UI_CHECK: defaultChecked must be boolean");
          if (props.value !== void 0) throw Error("UI_CHECK: defaultChecked starts a control without value or bind");
          wire("defaultValue", +props.defaultChecked);
        }
        if (props.indeterminate !== void 0) {
          if (tag !== "checkbox" || typeof props.indeterminate !== "boolean") throw Error("UI_CHECK: indeterminate is a boolean on a <checkbox>");
          if (props.indeterminate) wire("mixed", 1);
        }
        if (!disabled && (props.onChange || props.value === void 0)) {
          const { onChange, value, indeterminate } = props;
          on("onCheckedChange", (next) => onChange?.(next ?? (indeterminate === true || !value)), "check");
        }
        break;
      case "slider": {
        slider(props);
        const thumb = props[thumbMark];
        const within2 = (value) => !thumb ? value : thumb.upper ? Math.max(value, thumb.other) : Math.min(value, thumb.other);
        const unbound = props.value === void 0;
        if (!unbound) {
          props.value = within2(props.value);
          wire("sliderValue", props.value);
        }
        if (props.defaultValue !== void 0) {
          if (!unbound) throw Error("UI_SLIDER: defaultValue starts a slider without value or bind");
          wire("defaultValue", sliderDefault(props));
        }
        for (const [key, name] of [["sliderMin", "min"], ["sliderMax", "max"], ["sliderStep", "step"], ["accessibilityLabel", "accessibilityLabel"]])
          wire(key, props[name]);
        if (thumb) {
          wire("sliderOther", thumb.other);
          wire("sliderThumb", thumb.upper ? "upper" : "lower");
        }
        if (props.secondaryValue !== void 0) wire("sliderSecondary", sliderSecondary(props));
        const { onChange, onChangeEnd, onPressChange } = props;
        if (onChangeEnd !== void 0 && typeof onChangeEnd !== "function") throw Error("UI_SLIDER: onChangeEnd takes a function");
        if ((onChange || onChangeEnd || unbound) && !disabled) on("onSliderChange", (value) => {
          const next = within2(sliderValue(props, value));
          if (!unbound && next === props.value) return;
          onChange?.(next);
          if (onChangeEnd) changeEnd(identity2, next, onChangeEnd);
        }, "number");
        if (onPressChange !== void 0 || onChangeEnd) {
          if (onPressChange !== void 0 && typeof onPressChange !== "function") throw Error("UI_SLIDER: onPressChange takes a function");
          const report2 = (pressed) => {
            if (onChangeEnd) pressEnd(identity2, pressed, onChangeEnd);
            return onPressChange?.(pressed);
          };
          result.handlers.push([
            "onSliderPress",
            handle(),
            callbackOf(() => pressChange(identity2, report2), "void", false, { identity: identity2, callback: report2 })
          ]);
        }
        break;
      }
      case "input":
      case "textarea":
        label(props.accessibilityLabel, "UI_INPUT");
        if (!validInputText(props.value === void 0 ? "" : props.value) || !validInputText(props.placeholder === void 0 ? "" : props.placeholder)) throw Error("UI_INPUT: invalid text");
        if (props.autoFocus !== void 0 && typeof props.autoFocus !== "boolean") throw Error("UI_INPUT: autoFocus");
        const { keyboard, secure } = editorKind(tag, props);
        const attributes = editor(tag, props, keyboard);
        if (props.onSubmit !== void 0 && typeof props.onSubmit !== "function") throw Error("UI_INPUT: onSubmit");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        if (props.value !== void 0) wire("inputValue", props.value);
        if (props.placeholder !== void 0) wire("placeholder", props.placeholder);
        if (keyboard !== void 0) wire("keyboard", keyboard);
        if (keyboard === "date" || keyboard === "time") {
          const env = envOf(render.view);
          noteEnv(["language", "hourCycle"]);
          const shown = fieldText(keyboard, props.value ?? "", {
            seconds: typeof props.step === "number" && props.step < 60,
            language: env?.language,
            hourCycle: env?.hourCycle
          });
          if (props.value !== void 0) wire("displayValue", shown.displayValue);
          wire("sizeText", shown.sizeText);
        }
        if (secure) wire("secure", 1);
        if (props.autoFocus !== void 0) wire("autoFocus", +props.autoFocus);
        if (tag === "textarea" && ["done", "go", "search", "send"].includes(props.enterKeyHint)) wire("submitOnEnter", 1);
        if (props.bind !== void 0 && cellOf(props.bind).cell.focus) wire("focusRequest", cellOf(props.bind).cell.focus);
        if (props.bind !== void 0 && cellOf(props.bind).cell.selection) wire("selectionRequest", cellOf(props.bind).cell.selection);
        if (props.bind !== void 0 && cellOf(props.bind).cell.picker) {
          const request = cellOf(props.bind).cell.picker;
          if (keyboard === "date" || keyboard === "time") {
            if (!disabled && props.readOnly !== true) wire("pickerRequest", request);
          } else warnNoPicker(identity2, tag, keyboard);
        }
        for (const [key, value] of attributes) wire(key, value);
        if (tag === "textarea") {
          const { rows, maxRows } = props;
          const valid = (value) => value === void 0 || Number.isInteger(value) && value >= 1 && value <= 100;
          if (!valid(rows) || !valid(maxRows) || maxRows !== void 0 && (rows === void 0 || maxRows < rows))
            throw Error("UI_INPUT: rows and maxRows are integers 1..100, maxRows needs rows and is at least rows");
          if (rows !== void 0) wire("rows", rows);
          if (maxRows !== void 0) wire("maxRows", maxRows);
        }
        if (!disabled) {
          for (const name of ["onInput", "onSubmit"]) if (props[name]) on(name, props[name], "text");
        }
        if (props.onKeyDown !== void 0) {
          const map = props.onKeyDown, keys2 = map && typeof map === "object" ? Object.keys(map) : null;
          if (!keys2 || !keys2.length || keys2.some((key) => !editorKeys.has(key) || typeof map[key] !== "function"))
            throw Error(`UI_INPUT: onKeyDown maps ${[...editorKeys].join(", ")} to functions`);
          if (!disabled) {
            wire("keyMap", keys2.join(" "));
            on("onKeyDown", (key) => {
              if (Object.hasOwn(map, key)) map[key]();
            }, "text");
          }
        }
        if (props.onSelectionChange !== void 0) {
          if (typeof props.onSelectionChange !== "function") throw Error("UI_INPUT: onSelectionChange takes a function");
          const { onSelectionChange } = props;
          if (!disabled) on("onSelectionChange", (payload2) => {
            const { start, end, caret, rects } = JSON.parse(payload2);
            onSelectionChange({ start, end, caret, rects });
          }, "text");
        }
        if (props.onPaste !== void 0) {
          if (typeof props.onPaste !== "function") throw Error("UI_INPUT: onPaste takes a function");
          if (!disabled && props.readOnly !== true) on("onPaste", props.onPaste, "paste");
        }
        focusEvents();
        break;
      case "select": {
        label(props.accessibilityLabel, "UI_SELECT");
        const options2 = selectOptions(props);
        if (props.value !== void 0 && typeof props.value !== "string") throw Error("UI_SELECT: value must be a string");
        if (props.placeholder !== void 0 && !validInputText(props.placeholder)) {
          let got;
          if (typeof props.placeholder === "number") got = String(props.placeholder);
          else {
            try {
              got = (JSON.stringify(props.placeholder) ?? typeof props.placeholder).slice(0, 40);
            } catch {
              got = typeof props.placeholder;
            }
          }
          throw Error(`UI_SELECT: placeholder takes a UTF-8 string up to 65536 bytes; got ${got}`);
        }
        if (props.defaultValue !== void 0 && (props.value !== void 0 || !options2.some((option2) => option2.value === props.defaultValue)))
          throw Error("UI_SELECT: defaultValue names an option of a select without value or bind");
        const chosen = options2.find((option2) => option2.value === (props.value ?? props.defaultValue));
        result.text = chosen?.label ?? props.placeholder ?? "";
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        const unbound = props.value === void 0;
        if (!unbound) wire("inputValue", chosen ? chosen.value : "");
        else if (props.defaultValue !== void 0) wire("defaultValue", props.defaultValue);
        if (props.placeholder !== void 0) wire("placeholder", props.placeholder);
        wire("selectOptions", JSON.stringify(options2));
        if ((props.onChange || unbound) && !disabled) {
          const { onChange } = props;
          on("onSelect", (value) => {
            if (unbound || value !== props.value) onChange?.(value);
          }, "text");
        }
        break;
      }
      case "image":
        image(props);
        wire("imageSrc", props.src);
        if (props.alt !== void 0) wire("imageAlt", props.alt);
        if (props.retryKey !== void 0) wire("imageRetryKey", props.retryKey);
        if (props.placeholder !== void 0) wire("imagePlaceholder", props.placeholder);
        if (props.onLoad) on("onImageLoad", props.onLoad, "void", true);
        if (props.onError) on("onImageError", props.onError, "void", true);
        break;
      case "video": {
        video(props);
        const name = props.player === void 0 ? void 0 : playerName(props.player);
        if (props.player !== void 0 && name === void 0) throw Error("UI_VIDEO: player takes the handle useVideo(name) returns");
        const source3 = props.src ?? props.sources;
        if (source3 === void 0 && name === void 0) throw Error(`UI_PROP: ${authoringRules.exclusive.videoSource.missing}`);
        if (props.src !== void 0) wire("videoSrc", props.src);
        if (props.sources !== void 0) wire("videoSources", JSON.stringify(props.sources.map(({ src, label: label2 }) => ({ src, label: label2 }))));
        wire("videoPlayer", name === void 0 ? `el:${identity2}` : `name:${name}`);
        if (name !== void 0 && parent.inDialog) shownInDialog(name);
        if (props.poster !== void 0) wire("videoPoster", props.poster);
        if (source3 !== void 0) wire("videoPaused", +(props.paused ?? !props.autoPlay));
        for (const [key, prop] of videoFlags) if (props[prop] !== void 0) wire(key, +props[prop]);
        if (props.playbackRate !== void 0) wire("videoRate", props.playbackRate);
        if (props.volume !== void 0) wire("videoVolume", props.volume);
        if (props.accessibilityLabel !== void 0) {
          label(props.accessibilityLabel, "UI_VIDEO");
          wire("accessibilityLabel", props.accessibilityLabel);
        }
        if (name !== void 0 && props.onFullscreenChange !== void 0) {
          if (typeof props.onFullscreenChange !== "function") throw Error("UI_VIDEO: onFullscreenChange takes a function");
          fullscreenHandler(name, props.onFullscreenChange);
        }
        const handlers = videoEvents.filter(([, prop]) => props[prop]);
        if (name !== void 0 || handlers.length) on("onVideoEvent", (payload2) => {
          const { status, events, timeStamp = Date.now() } = JSON.parse(payload2);
          if (name !== void 0) videoStatus(name, status);
          for (const { type, ...detail } of events) {
            const handler = props[videoEvents.find(([event]) => event === type)?.[1]];
            if (handler) handler(Object.freeze({ ...type === "error" ? status.error : detail, timeStamp }));
          }
        }, "text", true);
        break;
      }
      case "webview":
        webview(props);
        wire("accessibilityLabel", props.accessibilityLabel);
        resolveWebView(props, identity2, parent, wire, on);
        break;
      case "vector": {
        const fit = result.fields.imageFit;
        delete result.fields.imageFit;
        wire("vectorSource", vector(props, fit));
        break;
      }
      case "plot": {
        plot(props);
        wire("plotValues", JSON.stringify(props.values));
        wire("plotKind", props.kind);
        wire("plotColors", JSON.stringify(props.colors ?? defaultSeries));
        wire("accessibilityLabel", props.accessibilityLabel);
        for (const name of ["smooth", "area"]) if (props[name] !== void 0) wire(`plot${name[0].toUpperCase()}${name.slice(1)}`, +props[name]);
        for (const [key, name] of [["plotDomainMin", "domainMin"], ["plotDomainMax", "domainMax"], ["plotMarkerEmphasisScale", "markerEmphasisScale"], ["plotSelectedIndex", "selectedIndex"]])
          if (props[name] !== void 0) wire(key, props[name]);
        if (props.pieLayout !== void 0) wire("plotPieLayout", JSON.stringify(props.pieLayout));
        if (props.onSelect) {
          const { onSelect, values } = props;
          on("onPlotSelect", (index) => {
            if (Number.isInteger(index) && index >= -1 && index < values.length) onSelect(index);
          }, "number");
        }
        const count = props.values.length, points = (count - 1) * (props.smooth ? 8 : 1) + 1;
        result.plotWork = props.kind === "pie" ? 256 + count : props.kind === "line" ? points * (props.area ? 2 : 1) + 2 * count + (props.area ? 2 : 0) : 8 * count;
        break;
      }
      case "radio":
        label(props.accessibilityLabel, "UI_RADIO");
        if (props.value !== void 0 && (typeof props.value !== "string" || props.value.length > 256)) throw Error("UI_RADIO: invalid value");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        result.children = radioOptions(props, identity2, result.fields, disabled);
        break;
    }
    if (tag === "scroll" || tag === "list") for (const name of ["onReachStart", "onReachEnd"]) {
      if (props[name] === void 0) continue;
      if (typeof props[name] !== "function") throw Error(`UI_PROP: ${name}`);
      on(name, props[name]);
    }
    if ((tag === "scroll" || tag === "list") && props.onScrollEdge !== void 0) {
      const { onScrollEdge } = props;
      if (typeof onScrollEdge !== "function") throw Error("UI_PROP: onScrollEdge takes a function");
      on("onScrollEdge", (payload2) => {
        const { edge, at } = JSON.parse(payload2);
        if ((edge === "start" || edge === "end") && typeof at === "boolean") onScrollEdge(Object.freeze({ edge, at }));
      }, "text");
    }
    if ((tag === "scroll" || tag === "list") && props.onVisibleChange !== void 0) {
      const { onVisibleChange, itemKeys: itemKeys2 } = props;
      if (typeof onVisibleChange !== "function") throw Error("UI_PROP: onVisibleChange takes a function");
      on("onVisibleChange", (payload2) => onVisibleChange(visibleEvent(JSON.parse(payload2), tag === "list" ? itemKeys2 : void 0)), "text");
    }
    if (tag === "list") {
      listKeys(props);
      result.list = listState(props, identity2, direction, parent.contexts);
      render.lists++;
    } else if (runs) {
      const extra = tag === "span" || tag === "link" ? style2.author : void 0;
      const lifted = [];
      const runOf = (child, slot, key, inFragment, nesting) => {
        if (nesting > 128) throw Error("UI_LIMIT: children too deep");
        const part = slot === void 0 ? `key:${JSON.stringify(key)}` : `position:${slot}`;
        const run = resolveNode(inline(child) ? h("span", {}, child) : child, `${identity2}/${tag}/${part}`, depth + 1, {
          tag,
          typography: style2.typography,
          direction,
          extra,
          inText: true,
          inControl: parent.inControl,
          inLink: parent.inLink || tag === "link",
          inDialog: parent.inDialog,
          popovers: parent.popovers,
          contexts: parent.contexts
        });
        if (run === null) return [];
        if (run.fragment) return run.fragment.flatMap((inner) => key === void 0 ? runOf(inner.child, inner.slot === void 0 ? void 0 : `${slot}.${inner.slot}`, inner.key, true, nesting + 1) : runOf(inner.child, void 0, JSON.stringify([key, inner.key ?? `#${inner.slot}`]), true, nesting + 1));
        if (run.lifted) lifted.push(...run.lifted);
        if (run.wire.some(([name]) => name === "topLayer")) {
          lifted.push(run);
          return [];
        }
        if (inFragment && run.element !== "span" && run.element !== "link")
          throw Error(`UI_TEXT: a component inside <${tag}> returns a <${run.element}> beside its text; with text it may return <span>, <link>, a popover or a <dialog>`);
        return (run.element === "span" || run.element === "link") && run.children.length ? run.children : [run];
      };
      result.children = flat.flatMap((child, position) => runOf(child, authored2[position].slot, authored2[position].key, false, 0));
      if (lifted.length) result.lifted = lifted;
      if (tag === "link") result.children = result.children.map((piece, i) => ({
        ...piece,
        tag: "link",
        element: "link",
        states: result.states,
        handlers: result.handlers.map((entry) => [...entry]),
        // The first piece stands for the whole link, so it answers to the link's test id.
        props: i === 0 && props.testId !== void 0 ? { ...piece.props, testId: props.testId } : piece.props,
        wire: [...piece.wire, ...result.wire.filter(([key]) => i === 0 || key !== "accessibilityLabel")]
      }));
    } else if (!leaf && tag !== "radio") {
      const keys2 = /* @__PURE__ */ new Set(), last = flat.length - 1, placed = [], slots = [];
      const place = ({ child, slot: at, key }, position, nesting) => {
        if (nesting > 128) throw Error("UI_LIMIT: children too deep");
        if (key !== void 0 && typeof key !== "string") throw Error("UI_KEY: expected string");
        if (key !== void 0 && keys2.has(key)) throw Error("UI_KEY: duplicate sibling");
        if (key !== void 0) keys2.add(key);
        const part = key === void 0 ? `position:${at}` : `key:${JSON.stringify(key)}`;
        const node = resolveNode(child, `${identity2}/${tag}/${part}`, depth + 1, {
          tag,
          typography: style2.typography,
          direction,
          extra: position < last ? style2.children : void 0,
          every: style2.every,
          sibling: { index: position, count: flat.length },
          inControl: parent.inControl || admission.tags[element.wire].children === "content",
          inBlockLink: parent.inBlockLink || block,
          inDialog: parent.inDialog || tag === "dialog",
          popovers: props[popoverScope] ?? parent.popovers,
          contexts: parent.contexts
        });
        if (node?.fragment) {
          for (const inner of node.fragment) place(key === void 0 ? { ...inner, slot: inner.slot === void 0 ? void 0 : `${at}.${inner.slot}`, key: inner.key } : { child: inner.child, key: JSON.stringify([key, inner.key ?? `#${inner.slot}`]) }, position, nesting + 1);
          return;
        }
        if (node === null) return;
        if (key === void 0) {
          placed.push(node);
          slots.push(at);
        } else {
          placed.push(node.props.key === key ? node : { ...node, props: { ...node.props, key } });
          slots.push(void 0);
        }
        for (const layer of liftedFrom(node, key === void 0 ? `position:${at}` : `key:${key}`)) {
          placed.push(layer);
          slots.push(void 0);
        }
      };
      for (let position = 0; position < authored2.length; position++) place(authored2[position], position, 0);
      result.children = placed;
      if ((tag === "button" || tag === "checkbox") && result.children.length === 1 && interactiveTags.has(result.children[0].tag))
        throw Error(`UI_BUTTON: <${tag}> holds only a <${result.children[0].tag}>; put the action on that control`);
      for (let ordinal = 0; ordinal < placed.length; ordinal++) {
        const at = slots[ordinal];
        if (at !== void 0 && at !== String(ordinal)) placed[ordinal] = { ...placed[ordinal], slot: at };
      }
    }
    if (style2.descendants && result.children.some(holdsDisabled)) Object.assign(result.fields, style2.descendants);
    return result;
  }
  function liftedFrom(node, part) {
    return (node.lifted ?? []).map((layer) => ({ ...layer, props: { ...layer.props, key: `lx-lifted:${part}${layer.identity.slice(node.identity.length)}` } }));
  }
  var holdsDisabled = (node) => node.props?.disabled === true || (node.children ?? []).some(holdsDisabled);
  function structure(sibling, flat) {
    const out = flat.length ? [] : ["empty"];
    if (!sibling) return out;
    const { index, count } = sibling;
    if (index === 0) out.push("first");
    if (index === count - 1) out.push("last");
    if (count === 1) out.push("only");
    out.push(index % 2 === 0 ? "odd" : "even");
    return out;
  }
  function progressValues(role, props, wire, tag) {
    const { ariaValueNow: now3, ariaValueMin: min, ariaValueMax: max, ariaValueText: text } = props;
    if (now3 === void 0 && min === void 0 && max === void 0 && text === void 0) return;
    if (tag === "slider" && now3 === void 0 && min === void 0 && max === void 0) {
      if (typeof text !== "string") throw Error("UI_PROP: aria-valuetext must be a string");
      wire("ariaValueText", text);
      return;
    }
    if (role === "combobox") {
      if (typeof text !== "string") throw Error("UI_PROP: aria-valuetext must be a string");
      wire("ariaValueText", text);
      return;
    }
    const number = (value, name) => {
      if (value !== void 0 && (typeof value !== "number" || !Number.isFinite(value))) throw Error(`UI_PROP: ${name} must be a finite number`);
    };
    number(now3, "aria-valuenow");
    number(min, "aria-valuemin");
    number(max, "aria-valuemax");
    if ((min ?? 0) >= (max ?? 100)) throw Error("UI_PROP: aria-valuemin must be below aria-valuemax");
    if (text !== void 0 && typeof text !== "string") throw Error("UI_PROP: aria-valuetext must be a string");
    if (now3 !== void 0) wire("ariaValueNow", now3);
    if (min !== void 0) wire("ariaValueMin", min);
    if (max !== void 0) wire("ariaValueMax", max);
    if (text !== void 0) wire("ariaValueText", text);
  }
  var present = (value) => value !== void 0 && value !== false && !(typeof value === "string" && !value.trim());
  var breaks = (rule, kind, role, props) => rule.tags && !rule.tags.includes(kind) || rule.role && !rule.role.includes(role) || rule.with?.some((name) => !present(props[name])) || rule.not && Object.keys(rule.not).some((name) => rule.not[name].includes(props[name])) || rule.any?.every((alternative) => breaks(alternative, kind, role, props)) || rule.without?.some((name) => present(props[name]));
  function authoredRules(tag, props, parent) {
    if (typeof tag !== "string" || !has(elements, tag)) return;
    const kind = tag === "link" && !parent.inText ? "link-box" : tag;
    const run = has(authoringRules.run, kind) ? authoringRules.run[kind] : void 0;
    if (run) {
      for (const key in props)
        if (props[key] !== void 0 && common2.has(key) && !run.includes(key)) throw Error(`UI_PROP: <${tag}> is a run of its <text>; put ${key} on the <text>`);
    }
    const only = authoringRules.elementRole[kind];
    const role = props.role ?? only?.implied;
    if (props.role !== void 0) {
      for (const rule of has(authoringRules.role, role) ? authoringRules.role[role] : [])
        if (breaks(rule, kind, role, props)) throw Error(`UI_PROP: ${rule.error}`);
      if (only && !only.roles.includes(props.role)) throw Error(`UI_PROP: ${only.error}`);
    }
    for (const name in authoringRules.prop) {
      const value = props[name];
      if (value === void 0) continue;
      for (const rule of authoringRules.prop[name])
        if ((!rule.on || rule.on.includes(kind)) && (!("value" in rule) || rule.value === value) && breaks(rule, kind, role, props)) throw Error(`UI_PROP: ${rule.error}`);
    }
  }
  function semantics(tag, props, wire) {
    const { role, headingLevel, ariaLevel, accessibilityHidden, accessibilityLiveRegion } = props;
    if (role !== void 0) {
      if (!fieldOptions.role.includes(role)) throw Error(`UI_PROP: role must be one of ${fieldOptions.role.join(", ")}`);
      if (tag !== "dialog") wire("role", role);
    }
    if (props.ariaModal !== void 0) {
      if (typeof props.ariaModal !== "boolean") throw Error("UI_PROP: aria-modal is a boolean");
      if (tag !== "dialog" && props.ariaModal) wire("modal", 1);
    }
    progressValues(role, props, wire, tag);
    const idOf = (name) => {
      const value = props[name];
      if (typeof value !== "string" || !value.length || value.length > 256 || /\s/.test(value)) throw Error(`UI_ID: ${name} names an element id`);
      return value;
    };
    if (props["aria-activedescendant"] !== void 0) {
      wire("ariaActiveDescendant", idOf("aria-activedescendant"));
    }
    if (props["aria-controls"] !== void 0 || props["aria-autocomplete"] !== void 0) {
      if (props["aria-controls"] !== void 0) {
        const ids = typeof props["aria-controls"] === "string" ? props["aria-controls"].trim().split(/\s+/) : [];
        if (!ids.length || ids.length > 8 || ids.some((id) => !id || id.length > 256)) throw Error("UI_ID: aria-controls lists 1..8 element ids");
        wire("ariaControls", ids.join(" "));
      }
      if (props["aria-autocomplete"] !== void 0) {
        if (!["list", "none"].includes(props["aria-autocomplete"])) throw Error('UI_PROP: aria-autocomplete is "list" or "none"');
        wire("ariaAutocomplete", props["aria-autocomplete"]);
      }
    }
    const current2 = props["aria-current"];
    if (current2 !== void 0 && current2 !== false) {
      if (current2 !== true && current2 !== "page" && current2 !== "step") throw Error('UI_PROP: aria-current is "page", "step" or a boolean');
      wire("ariaCurrent", String(current2));
    }
    if (headingLevel !== void 0) {
      if (!Number.isInteger(headingLevel) || headingLevel < 1 || headingLevel > 6) throw Error("UI_PROP: headingLevel is an integer 1..6");
      wire("headingLevel", headingLevel);
    }
    if (ariaLevel !== void 0) {
      if (!Number.isInteger(ariaLevel) || ariaLevel < 1 || ariaLevel > 32) throw Error("UI_PROP: aria-level is an integer 1..32");
      wire("ariaLevel", ariaLevel);
    }
    if (accessibilityHidden !== void 0) {
      if (typeof accessibilityHidden !== "boolean") throw Error("UI_PROP: aria-hidden must be boolean");
      if (accessibilityHidden) wire("accessibilityHidden", 1);
    }
    if (accessibilityLiveRegion !== void 0) {
      if (!fieldOptions.accessibilityLiveRegion.includes(accessibilityLiveRegion)) throw Error("UI_PROP: aria-live must be off, polite or assertive");
      wire("accessibilityLiveRegion", accessibilityLiveRegion);
    }
    const { accessibilityState: state } = props;
    if (state !== void 0) {
      if (!state || typeof state !== "object" || Array.isArray(state)) throw Error("UI_PROP: accessibilityState must be an object");
      for (const [name, value] of Object.entries(state)) {
        if (!has(semanticStates, name)) throw Error(`UI_PROP: accessibilityState.${name}; known: ${Object.keys(semanticStates).join(", ")}`);
        if (value === void 0) continue;
        if (name === "checked" && value === "mixed") {
          wire(semanticStates[name], 0);
          wire("mixed", 1);
          continue;
        }
        if (typeof value !== "boolean") throw Error(`UI_PROP: aria-${name} must be boolean`);
        wire(semanticStates[name], +value);
      }
    }
    if (["view", "page", "text", "dialog", "list"].includes(tag) && props.accessibilityLabel !== void 0) {
      label(props.accessibilityLabel, "UI_PROP");
      wire("accessibilityLabel", props.accessibilityLabel);
    }
  }
  var dragPoint = /^-?\d+(\.\d+)?%$/;
  function dragElement(tag, props, wire, on, release, key) {
    const { drag: axis, dragSnapPoints: points = [0], bind: cell, snapIndex, onSnapChange, onDragStart, onDragEnd } = props;
    if (axis !== "x" && axis !== "y") throw Error('UI_PROP: drag is "x" or "y"');
    if (!Array.isArray(points) || !points.length || points.length > limits.dragSnapPoints || points.some((point) => typeof point === "number" ? !Number.isFinite(point) || Math.abs(point) > limits.styleNumber : typeof point !== "string" || !dragPoint.test(point) || Math.abs(parseFloat(point)) > 1e3))
      throw Error(`UI_PROP: dragSnapPoints are 1..${limits.dragSnapPoints} offsets in px or %`);
    if (tag !== "dialog" && tag !== "button" && !props.accessibilityLabel)
      throw Error("UI_A11Y: an element with drag needs aria-label, so assistive tech can move it");
    for (const [name, value] of [["onDragStart", onDragStart], ["onDragEnd", onDragEnd], ["onSnapChange", onSnapChange]])
      if (value !== void 0 && typeof value !== "function") throw Error(`UI_PROP: ${name}`);
    let index = 0;
    if (cell !== void 0) {
      const bound = cellOf(cell);
      if (!bound || render.candidate.locals.get(bound.key) !== bound.cell || typeof cell.value !== "number")
        throw Error("UI_PROP: bind on a drag element takes a mounted number useLocal cell");
      index = cell.value;
    } else if (snapIndex !== void 0) index = snapIndex;
    if (!Number.isInteger(index) || index < 0 || index >= points.length) throw Error("UI_PROP: the bound snap index is out of range");
    const sent = cell === void 0 && snapIndex !== void 0 ? controlledSnap(key, index) : index;
    wire("drag", JSON.stringify({ axis, points, index: sent, ...release ? { release } : {} }));
    const close = props[dragClose];
    on("onDragEvent", (payload2) => {
      const event = JSON.parse(payload2);
      if (event.phase === "start") onDragStart?.();
      else if (event.phase === "end") onDragEnd?.({ index: event.index, velocity: event.velocity });
      else if (event.phase === "settle" && Number.isInteger(event.index) && event.index >= 0 && event.index < points.length) {
        if (cell !== void 0 && cell.value !== event.index) cell.set(event.index);
        else if (snapIndex !== void 0) {
          snapSettled(key, event.index, index, onSnapChange && event.index !== index ? () => onSnapChange(event.index) : void 0);
        } else if (onSnapChange && event.index !== index) onSnapChange(event.index);
        if (event.hidden === true) close?.();
      }
    }, "text");
  }
  function selectOptions(props) {
    if (!Array.isArray(props.options) || !props.options.length || props.options.length > 64) throw Error("UI_SELECT: expected 1..64 options");
    const values = /* @__PURE__ */ new Set();
    return props.options.map((option2) => {
      if (!option2 || typeof option2 !== "object" || Object.keys(option2).some((k) => !["value", "label", "disabled", "data-testid"].includes(k)) || typeof option2.value !== "string" || !option2.value.length || option2.value.length > 256 || values.has(option2.value) || typeof option2.label !== "string" || !validInputText(option2.label) || option2.disabled !== void 0 && typeof option2.disabled !== "boolean")
        throw Error("UI_SELECT: invalid or duplicate option");
      values.add(option2.value);
      return option2.disabled ? { value: option2.value, label: option2.label, disabled: true } : { value: option2.value, label: option2.label };
    });
  }
  function radioOptions(props, identity2, fields3, disabled) {
    if (!Array.isArray(props.options) || !props.options.length || props.options.length > 64) throw Error("UI_RADIO: expected 1..64 options");
    const values = /* @__PURE__ */ new Set();
    const style2 = Object.fromEntries(Object.entries(fields3).filter(([name]) => optionStyle.includes(name)));
    return props.options.map((option2) => {
      if (!option2 || typeof option2 !== "object" || Object.keys(option2).some((k) => !["value", "label", "disabled", "data-testid"].includes(k)) || typeof option2.value !== "string" || !option2.value.length || option2.value.length > 256 || values.has(option2.value) || typeof option2.label !== "string" || option2.disabled !== void 0 && typeof option2.disabled !== "boolean" || option2["data-testid"] !== void 0 && typeof option2["data-testid"] !== "string") throw Error("UI_RADIO: invalid or duplicate option");
      values.add(option2.value);
      const { value } = option2, selected = props.value === value, enabled = !disabled && option2.disabled !== true;
      const child = {
        tag: "radio-option",
        element: "radio-option",
        identity: `${identity2}/radio/key:${JSON.stringify(value)}`,
        props: { key: value, testId: option2["data-testid"] },
        fields: style2,
        states: {},
        text: option2.label,
        wire: [...enabled ? [] : [["enabled", 0]], ["checked", +selected]],
        handlers: [],
        children: []
      };
      if (enabled && props.onChange) child.handlers.push(["onCheckedChange", handle(), callbackOf(() => {
        if (!selected) return props.onChange(value);
      })]);
      return child;
    });
  }
  function reusableRows(old, props, rowEnv, direction) {
    const source3 = props.row?.__lxRows, before = old?.rowSource;
    if (!source3 || !before || before.site !== source3.site || before.height !== source3.height || old.rowEnv !== rowEnv || old.direction !== direction || !old.pure?.size) return null;
    const same4 = before.items === source3.items;
    const rows = /* @__PURE__ */ new Map();
    const recount = old.count !== props.count;
    for (const i of old.pure) {
      if (!old.cache.has(i) || i >= props.count || recount && (i === old.count - 1 || i === props.count - 1)) continue;
      const item = source3.items[i];
      if (same4 || item === before.items[i] && isFrozen(item)) rows.set(i, old.cache.get(i));
    }
    return rows.size ? { rows, typography: old.typography } : null;
  }
  function listState(props, identity2, direction, contexts) {
    const old = page.committed?.lists.get(identity2);
    const rowSource = props.row?.__lxRows, rowEnv = rowSource ? JSON.stringify(envOf(render.view) ?? null) : void 0;
    if (rowSource) noteEnv("*");
    const anchor = props.anchor === void 0 ? null : JSON.stringify(props.anchor);
    const positions2 = props.itemKeys && old?.states?.size ? keyPositions(props.itemKeys) : null;
    const states2 = positions2 ? new Map([...old?.states ?? []].flatMap(([, state]) => positions2.has(state.key) ? [[positions2.get(state.key), state]] : [])) : new Map([...old?.states ?? []].filter(([i]) => i < props.count));
    const anchorChanged = Boolean(old && old.count === props.count && anchor !== null && anchor !== old.anchor);
    if (anchorChanged && !props.anchor.duration) render.candidate?.movedAnchors?.set(identity2, anchor);
    return {
      count: props.count,
      previousCount: old?.count ?? props.count,
      anchor,
      anchorChanged,
      anchorCache: old?.cache ?? null,
      states: states2,
      cache: /* @__PURE__ */ new Map(),
      pure: /* @__PURE__ */ new Set(),
      reusable: reusableRows(old, props, rowEnv, direction),
      rowSource,
      rowEnv,
      direction,
      contexts,
      window: old?.window ? { ...old.window, start: Math.min(old.window.start, props.count), end: Math.min(old.window.end, props.count) } : null
    };
  }

  // src/runtime/frames.js
  var pending = [];
  var waiting2 = /* @__PURE__ */ new Map();
  var nextToken2 = 1;
  var LONGEST_MS = 6e4;
  function measureFrames(ms) {
    if (typeof ms !== "number" || !Number.isFinite(ms) || ms <= 0 || ms > LONGEST_MS) {
      return Promise.reject(new RangeError(`UI_PROP: measureFrames takes 1\u2013${LONGEST_MS} ms`));
    }
    const token = nextToken2++;
    pending.push([token, Math.round(ms)]);
    return new Promise((resolve) => waiting2.set(token, resolve));
  }
  function takeFrameMeasures() {
    const result = pending;
    pending = [];
    return JSON.stringify(result);
  }
  function frameMeasured(token, stats) {
    const resolve = waiting2.get(token);
    if (!resolve) return false;
    waiting2.delete(token);
    resolve(stats);
    return true;
  }
  function resetFrameMeasures() {
    pending = [];
    waiting2 = /* @__PURE__ */ new Map();
  }

  // src/runtime/reveal.js
  var pending2 = [];
  var MOST = 16;
  function reveal(id) {
    outsideRender("reveal");
    if (typeof id !== "string" || !id.length || id.length > 256) throw Error("UI_ID: reveal takes an element id of 1..256 characters");
    if (pending2.length < MOST) pending2.push(id);
  }
  function takeReveals() {
    const out = pending2;
    pending2 = [];
    return JSON.stringify(out);
  }
  function resetReveals() {
    pending2 = [];
  }

  // src/runtime/picker.js
  var picker = null;
  var results = [];
  var nextKey = 1;
  var fixedToday = null;
  var today = () => {
    if (fixedToday) return fixedToday;
    const now3 = /* @__PURE__ */ new Date();
    return { year: now3.getFullYear(), month: now3.getMonth() + 1, day: now3.getDate() };
  };
  function openPicker({ identity: identity2, kind, value, min, max, step, today: day, options: options2, selected, label: label2, anchorWidth } = {}) {
    if (typeof identity2 !== "string" || !["date", "time", "select"].includes(kind)) return false;
    fixedToday = parseDate(day);
    const key = nextKey++;
    if (kind === "select") {
      const list = Array.isArray(options2) ? options2.slice(0, 1024).map((option2) => ({
        value: String(option2.value),
        label: String(option2.label),
        enabled: option2.enabled !== false
      })) : [];
      if (!list.length) return false;
      const chosen = Number.isInteger(selected) && list[selected] ? selected : -1;
      const width = Number.isFinite(anchorWidth) ? Math.min(Math.max(anchorWidth, 0), 360) : 0;
      picker = {
        kind,
        key,
        identity: identity2,
        options: list,
        selected: chosen,
        pick: chosen,
        focus: chosen >= 0 ? chosen : list.findIndex((o) => o.enabled),
        label: typeof label2 === "string" ? label2 : "",
        typed: "",
        typedAt: 0,
        width
      };
      return true;
    }
    if (kind === "date") {
      const low = parseDate(min), high = parseDate(max);
      picker = {
        kind,
        key,
        identity: identity2,
        min: low,
        max: high,
        value: parseDate(value),
        pick: parseDate(value),
        keyed: false,
        focus: clampDate(parseDate(value) ?? today(), low, high)
      };
    } else {
      const seconds = secondValues(Number(step));
      const { hour, minute, second } = parseTime(value) ?? { hour: 9, minute: 0 };
      picker = {
        kind,
        key,
        identity: identity2,
        step: Number(step),
        minutes: minuteValues(Number(step)),
        seconds,
        value: seconds ? { hour, minute, second: second ?? 0 } : { hour, minute },
        min: parseTime(min),
        max: parseTime(max)
      };
      if (!timeAllowed(picker.value)) picker.value = firstAllowed(picker.value) ?? picker.value;
    }
    return true;
  }
  var daySeconds = ({ hour, minute, second = 0 }) => hour * 3600 + minute * 60 + second;
  function timeAllowed(time) {
    const { min, max } = picker;
    if (!min && !max) return true;
    const at = daySeconds(time), low = min ? daySeconds(min) : 0, high = max ? daySeconds(max) : 86399;
    return low <= high ? at >= low && at <= high : at >= low || at <= high;
  }
  var hourTimes = (hour) => picker.minutes.flatMap((minute) => picker.seconds ? picker.seconds.map((second) => ({ hour, minute, second })) : [{ hour, minute }]);
  function firstAllowed(from) {
    for (let offset = 0; offset < 25; offset++) {
      const hour = (from.hour + offset) % 24;
      const found = hourTimes(hour).find((time) => (offset > 0 || daySeconds(time) >= daySeconds(from)) && timeAllowed(time));
      if (found) return found;
    }
    return null;
  }
  var hourAllowed = (hour) => hourTimes(hour).some(timeAllowed);
  var hasPicker = () => picker !== null;
  var pickerIdentity = () => picker?.identity ?? null;
  function settle(value) {
    if (!picker) return;
    results.push({ identity: picker.identity, value });
    picker = null;
  }
  function closePicker() {
    picker = null;
  }
  function takePickerResults() {
    const out = results;
    results = [];
    return JSON.stringify(out);
  }
  function settleSelect(index) {
    const option2 = picker.options[index];
    if (!option2?.enabled) return;
    results.push({ identity: picker.identity, value: option2.value, index });
    picker = null;
  }
  function selectKey(key) {
    const { options: options2 } = picker;
    const step = { ArrowDown: 1, ArrowUp: -1, PageDown: 8, PageUp: -8 }[key];
    if (step !== void 0) {
      let next = picker.focus;
      for (let moved = 0; moved < Math.abs(step); moved++) {
        let probe = next;
        do
          probe += Math.sign(step);
        while (probe >= 0 && probe < options2.length && !options2[probe].enabled);
        if (probe < 0 || probe >= options2.length) break;
        next = probe;
      }
      picker.focus = next;
      return true;
    }
    if (key === "Enter") {
      settleSelect(picker.focus);
      return true;
    }
    if (key.length === 1) {
      const now3 = Date.now();
      picker.typed = (now3 - picker.typedAt < 1e3 ? picker.typed : "") + key.toLowerCase();
      picker.typedAt = now3;
      const found = options2.findIndex((o) => o.enabled && o.label.toLowerCase().startsWith(picker.typed));
      if (found >= 0) picker.focus = found;
      return true;
    }
    return false;
  }
  function pickerKey(key, rtl = false) {
    if (!picker) return false;
    if (key === "Escape") {
      settle(null);
      return true;
    }
    if (picker.kind === "select") return selectKey(key);
    if (picker.kind === "date") {
      if (key === "Enter") {
        if (within(picker.focus, picker.min, picker.max)) settle(iso2(picker.focus));
        return true;
      }
      const moved = moveDate(picker.focus, key, rtl);
      if (!moved) return false;
      picker.focus = clampDate(moved, picker.min, picker.max);
      picker.keyed = true;
      return true;
    }
    if (key === "Enter") {
      if (timeAllowed(picker.value)) settle(timeIso(picker.value));
      return true;
    }
    if (/^[0-9]$/.test(key)) {
      const width = picker.seconds ? 6 : 4;
      picker.typed = ((picker.typed ?? "") + key).slice(-width);
      const digits = picker.typed.padStart(width, "0");
      const hour2 = Number(digits.slice(0, 2)), minute2 = Number(digits.slice(2, 4)), second = Number(digits.slice(4) || 0);
      if (hour2 < 24 && minute2 < 60 && second < 60) picker.value = picker.seconds ? { hour: hour2, minute: minute2, second } : { hour: hour2, minute: minute2 };
      return true;
    }
    const delta = { ArrowUp: -1, ArrowDown: 1 }[key];
    if (delta === void 0) return false;
    const { minutes } = picker;
    let { hour, minute } = picker.value;
    const index = Math.max(0, minutes.indexOf(minute)) + delta;
    if (index < 0) {
      hour = (hour + 23) % 24;
      minute = minutes.at(-1);
    } else if (index >= minutes.length) {
      hour = (hour + 1) % 24;
      minute = minutes[0];
    } else minute = minutes[index];
    picker.value = minuteTime(hour, minute) ?? picker.value;
    return true;
  }
  function minuteTime(hour, minute) {
    if (!picker.seconds) return timeAllowed({ hour, minute }) ? { hour, minute } : null;
    const kept2 = { hour, minute, second: picker.value.second };
    if (timeAllowed(kept2)) return kept2;
    const second = picker.seconds.find((value) => timeAllowed({ hour, minute, second: value }));
    return second === void 0 ? null : { hour, minute, second };
  }
  function dateBody(words2, env, titled) {
    const { focus, value, min, max } = picker;
    const first = env?.firstWeekday ?? 7, now3 = today();
    const shift = (months) => {
      picker.focus = clampDate(addMonths(picker.focus, months), min, max);
    };
    const iconButton = (testId, label2, d, months) => h(
      "button",
      {
        "data-testid": testId,
        "aria-label": label2,
        onClick: () => shift(months),
        style: { width: 36, height: 36, borderRadius: 18, alignItems: "center", justifyContent: "center", color: "text" }
      },
      h("vector", {
        "aria-hidden": true,
        paths: [{ d, fill: "none", stroke: "currentColor", strokeWidth: 2, strokeLinecap: "round", strokeLinejoin: "round" }],
        style: { width: 20, height: 20 }
      })
    );
    const title = words2.title(words2.months[focus.month - 1], focus.year);
    const cells2 = monthGrid(focus, first);
    const rows = Array.from({ length: 6 }, (_, week) => cells2.slice(week * 7, week * 7 + 7));
    const cell = ({ date, inMonth }) => {
      const id = iso2(date), enabled = within(date, min, max);
      const selected = titled ? value && compare(date, value) === 0 : picker.pick !== null && compare(date, picker.pick) === 0;
      const focused = (titled || picker.keyed) && compare(date, focus) === 0, isToday = compare(date, now3) === 0;
      const name = words2.full(date.day, words2.months[date.month - 1], date.year, words2.longDays[weekday(date.year, date.month, date.day)]);
      return h(
        "button",
        {
          key: id,
          "data-testid": `lx-picker-day-${id}`,
          role: "gridcell",
          "aria-label": name,
          "aria-selected": Boolean(selected),
          disabled: !enabled,
          onClick: () => {
            if (titled) settle(id);
            else picker.pick = picker.focus = date;
          },
          style: {
            width: 36,
            height: 36,
            borderRadius: 18,
            alignItems: "center",
            justifyContent: "center",
            backgroundColor: selected ? "accent-solid" : focused ? "surface-sunken" : "transparent",
            borderWidth: isToday && !selected ? 1 : 0,
            borderColor: "accent",
            color: selected ? "on-accent-solid" : !inMonth || !enabled ? "text-faint" : "text",
            fontSize: 14,
            fontWeight: selected || isToday ? 600 : 400
          }
        },
        String(date.day)
      );
    };
    return h(
      "view",
      { style: { gap: 8, alignSelf: "center" } },
      h(
        "view",
        { style: { flexDirection: "row", alignItems: "center", gap: 4, justifyContent: titled ? "start" : "space-between" } },
        titled && h("text", { "data-testid": "lx-picker-title", "aria-live": "polite", style: { flexGrow: 1, fontSize: 16, fontWeight: 600, color: "text", paddingLeft: 8 } }, title),
        iconButton("lx-picker-previous", words2.previous, "M12.5 4l-6 6 6 6", -1),
        iconButton("lx-picker-next", words2.next, "M7.5 4l6 6-6 6", 1)
      ),
      h(
        "view",
        { role: "grid", "aria-label": title, style: { gap: 2 } },
        h("view", { "aria-hidden": true, style: { flexDirection: "row", gap: 2 } }, ...weekdayOrder(first).map((day) => h("text", {
          key: `w${day}`,
          style: { width: 36, textAlign: "center", fontSize: 12, color: "text-muted" }
        }, words2.days[day]))),
        ...rows.map((week, index) => h("view", { key: `r${index}`, style: { flexDirection: "row", gap: 2 } }, ...week.map(cell)))
      )
    );
  }
  var ROW = 40;
  var VISIBLE = 5;
  function timeColumn(testId, label2, values, selected, format, choose, cyclic = false, enabled = () => true) {
    const copies = cyclic ? 3 : 1, middle = cyclic ? 1 : 0;
    const index = middle * values.length + Math.max(0, values.indexOf(selected));
    const pad2 = cyclic ? 0 : ROW * Math.floor(VISIBLE / 2);
    const offset = cyclic ? Math.floor(VISIBLE / 2) : 0;
    return h(
      "view",
      { "data-testid": testId, role: "listbox", "aria-label": label2, style: { width: 64 } },
      h(listTag, {
        count: values.length * copies,
        itemKeys: Array.from({ length: values.length * copies }, (_, i) => `${Math.floor(i / values.length)}:${values[i % values.length]}`),
        height: () => ROW,
        anchor: { index: Math.max(0, index - offset), align: "start", seq: index },
        style: { height: ROW * VISIBLE, paddingTop: pad2, paddingBottom: pad2, scrollSnapType: "y" },
        row: (i) => {
          const value = values[i % values.length], copy2 = Math.floor(i / values.length), on = value === selected, live = enabled(value);
          const id = copy2 === middle ? `${testId}-${value}` : `${testId}-${value}-${copy2}`;
          return h("button", {
            key: `${copy2}:${value}`,
            "data-testid": id,
            role: "option",
            "aria-selected": on && copy2 === middle,
            "aria-hidden": copy2 !== middle,
            disabled: !live,
            onClick: () => choose(value),
            style: {
              height: ROW,
              alignItems: "center",
              justifyContent: "center",
              borderRadius: 8,
              scrollSnapAlign: "center",
              backgroundColor: on ? "accent/12" : "transparent",
              color: on ? "accent" : live ? "text" : "text-faint",
              fontSize: 17,
              fontWeight: on ? 600 : 400,
              fontVariantNumeric: "tabular-nums"
            }
          }, format(value));
        }
      })
    );
  }
  function timeBody(words2, env) {
    const cycle = env?.hourCycle === "h12" ? "h12" : "h23";
    const { hour, minute, second } = picker.value;
    const two = (value) => String(value).padStart(2, "0");
    const set = (next) => {
      const wanted = { ...picker.value, ...next };
      picker.value = timeAllowed(wanted) ? wanted : "second" in next ? picker.value : "minute" in next ? minuteTime(wanted.hour, wanted.minute) ?? picker.value : hourTimes(wanted.hour).find(timeAllowed) ?? picker.value;
      picker.typed = "";
    };
    const hour12 = (value) => value % 12 + (hour >= 12 ? 12 : 0);
    const columns = [
      timeColumn(
        "lx-picker-hour",
        words2.hour,
        hourValues(cycle),
        cycle === "h12" ? hour % 12 || 12 : hour,
        cycle === "h12" ? String : two,
        (value) => set({ hour: cycle === "h12" ? hour12(value) : value }),
        true,
        (value) => hourAllowed(cycle === "h12" ? hour12(value) : value)
      ),
      timeColumn(
        "lx-picker-minute",
        words2.minute,
        picker.minutes,
        minute,
        two,
        (value) => set({ minute: value }),
        true,
        (value) => minuteTime(hour, value) !== null
      )
    ];
    if (picker.seconds) columns.push(timeColumn(
      "lx-picker-second",
      words2.second,
      picker.seconds,
      second,
      two,
      (value) => set({ second: value }),
      true,
      (value) => timeAllowed({ hour, minute, second: value })
    ));
    const halfHours = (period) => Array.from({ length: 12 }, (_, index) => index + (period === "pm" ? 12 : 0));
    if (cycle === "h12") columns.push(timeColumn(
      "lx-picker-period",
      words2.period,
      ["am", "pm"],
      hour >= 12 ? "pm" : "am",
      (period) => words2[period],
      (period) => {
        const moved = hour % 12 + (period === "pm" ? 12 : 0);
        set({ hour: hourAllowed(moved) ? moved : halfHours(period).find(hourAllowed) ?? moved });
      },
      false,
      (period) => halfHours(period).some(hourAllowed)
    ));
    return h("view", { style: { flexDirection: "row", gap: 8, justifyContent: "center" } }, ...columns);
  }
  function selectBody(touch, words2) {
    const { options: options2, focus } = picker;
    const selected = touch ? picker.pick : picker.selected;
    const check = "M4 10.5l4 4 8-9";
    return h(
      "view",
      { role: "listbox", "aria-label": picker.label || words2.options, style: { gap: 2, minWidth: Math.max(160, picker.width - 24) } },
      ...options2.map((option2, index) => h(
        "button",
        {
          key: `o${index}`,
          "data-testid": `lx-picker-option-${index}`,
          role: "option",
          "aria-selected": index === selected,
          disabled: !option2.enabled,
          onClick: () => {
            if (touch) picker.pick = picker.focus = index;
            else settleSelect(index);
          },
          style: {
            flexDirection: "row",
            alignItems: "center",
            gap: 8,
            minHeight: 36,
            paddingLeft: 12,
            paddingRight: 10,
            borderRadius: 8,
            backgroundColor: index === focus ? "surface-sunken" : "transparent",
            color: option2.enabled ? "text" : "text-faint",
            fontSize: 15
          }
        },
        h("text", { style: { flexGrow: 1, textAlign: "start" } }, option2.label),
        index === selected ? h("vector", { "aria-hidden": true, paths: [{
          d: check,
          fill: "none",
          stroke: "currentColor",
          strokeWidth: 2,
          strokeLinecap: "round",
          strokeLinejoin: "round"
        }], style: { width: 18, height: 18, color: "accent" } }) : h("view", { style: { width: 18, height: 18 } })
      ))
    );
  }
  var pickerAnchor = (env) => picker && (env?.pointer ?? "coarse") !== "coarse" ? picker.identity : null;
  var sheetStyle = ({ borderRadius, ...rest }) => rest;
  var clock3 = (time, words2, env) => clockText(time, words2, env?.hourCycle);
  function pickerNode(env) {
    if (!picker) return null;
    const words2 = wordsFor(env?.language);
    const touch = (env?.pointer ?? "coarse") === "coarse";
    const body = picker.kind === "date" ? dateBody(words2, env, !touch) : picker.kind === "time" ? timeBody(words2, env) : selectBody(touch, words2);
    const unpicked = picker.kind === "date" ? picker.pick === null : picker.kind === "time" && !timeAllowed(picker.value);
    const commit2 = () => unpicked ? void 0 : picker.kind === "date" ? settle(iso2(picker.pick)) : picker.kind === "time" ? settle(timeIso(picker.value)) : picker.pick >= 0 ? settleSelect(picker.pick) : settle(null);
    const cancel = (style2) => h("button", { "data-testid": "lx-picker-cancel", onClick: () => settle(null), style: style2 }, words2.cancel);
    const done = (style2) => h("button", {
      "data-testid": "lx-picker-done",
      onClick: commit2,
      disabled: unpicked,
      style: unpicked ? { ...style2, color: "text-faint" } : style2
    }, words2.done);
    const monthTitle = picker.kind === "date" && words2.title(words2.months[picker.focus.month - 1], picker.focus.year);
    const text = { paddingTop: 8, paddingBottom: 8, paddingLeft: 4, paddingRight: 4, fontSize: 16, backgroundColor: "transparent" };
    const toolbar = touch && h(
      "view",
      { style: { flexDirection: "row", alignItems: "center", gap: 8 } },
      cancel({ ...text, color: "text-muted" }),
      h(
        "text",
        {
          "data-testid": "lx-picker-title",
          "aria-live": "polite",
          style: { flexGrow: 1, flexBasis: 0, minWidth: 0, textAlign: "center", fontSize: 16, fontWeight: 600, color: "text", lineClamp: 1 }
        },
        monthTitle || (picker.kind === "time" ? clock3(picker.value, words2, env) : picker.label || "")
      ),
      done({ ...text, color: "accent", fontWeight: 600 })
    );
    const button = { paddingTop: 8, paddingBottom: 8, paddingLeft: 14, paddingRight: 14, borderRadius: 8, fontSize: 15 };
    const footer = !touch && picker.kind === "time" && h(
      "view",
      { style: { flexDirection: "row", gap: 8, justifyContent: "end" } },
      cancel({ ...button, color: "text" }),
      done({ ...button, backgroundColor: "accent-solid", color: "on-accent-solid", fontWeight: 600 })
    );
    const name = picker.kind === "date" ? monthTitle : picker.kind === "time" ? words2.hour : picker.label || words2.options;
    const panel = {
      "data-testid": "lx-picker",
      key: `picker-${picker.key}`,
      "aria-label": name,
      style: {
        padding: 12,
        gap: 12,
        borderRadius: 14,
        backgroundColor: "surface-overlay",
        borderWidth: touch ? 0 : 1,
        borderColor: "line",
        boxShadow: [{ offsetX: 0, offsetY: 8, blur: 24, spread: 0, color: "shadow/20" }]
      }
    };
    return touch ? h("dialog", {
      ...panel,
      open: true,
      closedby: "any",
      onClose: () => settle(null),
      // Edge to edge and flush with the bottom; the content clears the home indicator.
      style: {
        ...sheetStyle(panel.style),
        marginTop: "auto",
        marginBottom: 0,
        marginLeft: 0,
        marginRight: 0,
        width: "100%",
        borderTopLeftRadius: 14,
        borderTopRightRadius: 14,
        paddingBottom: 12 + (env?.safeArea?.bottom ?? 0),
        paddingLeft: 12 + (env?.safeArea?.left ?? 0),
        paddingRight: 12 + (env?.safeArea?.right ?? 0)
      }
    }, toolbar, body) : h("view", {
      ...panel,
      id: "lx-picker",
      popover: "auto",
      open: true,
      onToggle: (open2) => {
        if (!open2) settle(null);
      },
      style: { ...panel.style, position: "absolute", top: 0, left: 0 }
    }, body, footer);
  }
  function resetPicker() {
    picker = null;
    results = [];
  }

  // src/runtime/tree.js
  function List({ items, getKey, renderItem, getItemHeight, ...props }) {
    if (!isFrozen(items) && items.length && !isFrozen(items[0]) && typeof globalThis.__lingxiaWarn === "function") warnUnfrozen(props, renderItem);
    const itemKeys2 = keysOf(items, getKey);
    const row = (index) => renderItem(items[index], index);
    if (typeof renderItem === "function") row.__lxRows = { items: isFrozen(items) ? items : items.slice(), site: renderItem, height: getItemHeight };
    return h(listTag, {
      ...props,
      count: items.length,
      itemKeys: itemKeys2,
      row,
      ...getItemHeight ? { height: (index) => getItemHeight(items[index], index) } : {}
    });
  }
  List.__lxid = "@lingxia/view#List";
  var warnedLists = /* @__PURE__ */ new Set();
  function warnUnfrozen(props, renderItem) {
    const site = props["data-testid"] ?? String(renderItem).slice(0, 120);
    if (warnedLists.has(site)) return;
    warnedLists.add(site);
    const name = props["data-testid"] ? ` "${props["data-testid"]}"` : "";
    globalThis.__lingxiaWarn(`UI_LIST: <List>${name} got items that are not frozen, so every row renders again on every render; pass the Logic data rows through (filtering or slicing them keeps reuse), or Object.freeze each item the View builds`);
  }
  function runCandidate(view, lookup2, build, recall2 = null) {
    if (render.active) throw Error("UI_RENDER: recursive render");
    view = ensureNormalizedView(view);
    render.thrown = "";
    resetArena();
    Object.assign(render, { active: true, view, lookupLocals: lookup2, recall: recall2?.memo, recallClosures: recall2?.closures });
    render.candidate = {
      locals: /* @__PURE__ */ new Map(),
      declarations: /* @__PURE__ */ new Set(),
      callbacks: /* @__PURE__ */ new Map(),
      lists: /* @__PURE__ */ new Map(),
      resources: /* @__PURE__ */ new Map(),
      resourceNames: /* @__PURE__ */ new Set(),
      view,
      memo: recall2 && /* @__PURE__ */ new Map(),
      closures: recall2 && /* @__PURE__ */ new Map(),
      closureCounts: /* @__PURE__ */ new Map()
    };
    try {
      render.scope = "root";
      return build(render.candidate);
    } catch (error) {
      render.thrown = error instanceof Error ? error.message : "JavaScript threw a value";
      discard();
      throw error;
    } finally {
      clearArena();
      Object.assign(render, { active: false, scope: null, view: null, lookupLocals: null, collectors: [], recall: null, recallClosures: null, frames: [], site: null });
    }
  }
  var movedAnchors = "";
  function takeThrown() {
    const thrown = render.thrown;
    render.thrown = "";
    return thrown;
  }
  function takeMovedAnchors() {
    const moved = movedAnchors;
    movedAnchors = "";
    return moved;
  }
  function prepare(renderPage, view, windows = {}, deferAnchors = false) {
    return runCandidate(view, null, (candidate) => {
      if (deferAnchors) candidate.movedAnchors = /* @__PURE__ */ new Map();
      render.clockStep = 0;
      let since = now();
      const root = renderPage(view);
      spent("render", since);
      since = now();
      candidate.tree = resolveNode(root, "root");
      spent("resolve", since);
      render.site = null;
      if (!candidate.tree || candidate.tree.fragment) throw Error("UI_ROOT: page must return one layout node");
      if (candidate.tree.lifted) throw Error("UI_ROOT: a <text> page root has no place for a popover it holds; wrap it in <view>");
      candidate.tree = { ...candidate.tree, children: [...candidate.tree.children] };
      const env = envOf(view);
      const fallback = fullscreenFallback();
      const bare = fallback && resolveBare(() => resolveNode(
        fallback,
        `root/${candidate.tree.element}/key:"lx-fullscreen"`,
        1,
        { tag: candidate.tree.element, typography: candidate.tree.typography }
      ));
      if (bare) candidate.tree.children.push(bare);
      const webFallback = webFullscreenFallback();
      const bareWeb = webFallback && resolveBareWeb(() => resolveNode(
        webFallback,
        `root/${candidate.tree.element}/key:"lx-web-fullscreen"`,
        1,
        { tag: candidate.tree.element, typography: candidate.tree.typography }
      ));
      if (bareWeb) candidate.tree.children.push(bareWeb);
      const picker2 = pickerNode(env);
      if (picker2) {
        const layer = resolveNode(
          picker2,
          `root/${candidate.tree.element}/key:"lx-picker-layer"`,
          1,
          { tag: candidate.tree.element, typography: candidate.tree.typography }
        );
        const anchor = pickerAnchor(env);
        const panel = layer.children.find((child) => child.props?.testId === "lx-picker");
        if (anchor && panel) panel.wire.push(["anchorTo", anchor]);
        candidate.tree.children.push(layer);
      }
      finishTopLayer(candidate.tree);
      candidate.baseLocals = new Map(candidate.locals);
      if (candidate.movedAnchors?.size) {
        movedAnchors = JSON.stringify(Object.fromEntries(candidate.movedAnchors));
        return [];
      }
      const encoding = now();
      try {
        const wire = encodeTree(candidate.tree, windows);
        const retireVideo = bare && fullscreenDrawn();
        const retireWeb = bareWeb && webFullscreenDrawn();
        if (!retireVideo && !retireWeb) return wire;
        candidate.tree.children = candidate.tree.children.filter((child) => (!retireVideo || child !== bare) && (!retireWeb || child !== bareWeb));
        return encodeTree(candidate.tree, windows);
      } finally {
        spent("encode", encoding);
      }
    }, page.committed ?? { memo: /* @__PURE__ */ new Map(), closures: /* @__PURE__ */ new Map() });
  }
  function prepareWindows(windows) {
    const base = render.candidate ?? page.committed;
    if (!base) throw Error("UI_LIST: no retained description");
    return runCandidate(base.view, base.locals, (candidate) => {
      candidate.resources = new Map(base.resources);
      candidate.topLayer = base.topLayer;
      candidate.fullscreenHandlers = base.fullscreenHandlers;
      candidate.webFullscreenHandlers = base.webFullscreenHandlers;
      candidate.webSources = base.webSources && new Map(base.webSources);
      candidate.tree = copyTree(base.tree);
      candidate.memo = new Map(base.memo);
      candidate.closures = new Map(base.closures);
      candidate.baseLocals = new Map(base.baseLocals);
      retainCells(candidate.baseLocals);
      const encoding = now();
      try {
        return encodeTree(candidate.tree, windows);
      } finally {
        spent("windows", encoding);
      }
    }, base);
  }
  function commit() {
    const since = now();
    try {
      commitCandidate();
    } finally {
      spent("commit", since);
    }
  }
  function commitCandidate() {
    const { candidate } = render;
    if (!candidate) throw Error("UI_COMMIT: no candidate");
    const previous = page.committed;
    for (const cell of candidate.locals.values()) if (page.updates.has(cell)) cell.value = page.updates.get(cell);
    commitClock();
    page.locals = candidate.locals;
    page.callbacks = candidate.callbacks;
    page.committed = candidate;
    render.candidate = null;
    page.updates.clear();
    page.invalidated = candidate.invalidateAfterCommit === true;
    for (const { key, cell, index } of candidate.dragResets ?? []) {
      if (page.locals.get(key) !== cell || cell.value === index) continue;
      page.updates.set(cell, index);
      page.invalidated = true;
    }
    const retained = new Set(page.locals.values()), next = /* @__PURE__ */ new Map(), definitions = /* @__PURE__ */ new Map();
    for (const [cell, resource] of page.resources) if (!retained.has(cell)) resource.dispose();
    for (const cell of retained) {
      const entry = candidate.resources.get(cell) ?? previous?.resources.get(cell);
      if (!entry) continue;
      definitions.set(cell, entry);
      next.set(cell, entry.resource);
      entry.resource.commit(entry.definition);
    }
    candidate.resources = definitions;
    page.resources = next;
    const pressable = /* @__PURE__ */ new Map();
    for (const entry of candidate.callbacks.values()) if (entry.press) pressable.set(entry.press.identity, entry.press.callback);
    refreshPresses(pressable);
    syncPageTitle(pageTitleOf(candidate));
  }
  var pageTitleOf = (candidate) => candidate.tree?.wire?.find(([key]) => key === "pageTitle")?.[1];
  var probeRows = 2e3;
  function probeListRow(testId) {
    const base = page.committed;
    if (!base || render.active || typeof testId !== "string") return "null";
    const lists = [];
    const walk = (node, depth) => {
      if (!node || depth > 256) return;
      if (node.list && typeof node.props?.row === "function") lists.push([node, depth]);
      for (const child of node.children ?? []) walk(child, depth + 1);
    };
    walk(base.tree, 0);
    const holds = (node) => node?.props?.testId === testId || (node?.children ?? []).some(holds);
    const saved = render.candidate;
    try {
      return JSON.stringify(runCandidate(base.view, base.locals, () => {
        for (const [node, depth] of lists) {
          const { window } = node.list;
          for (let i = 0; i < Math.min(node.props.count, probeRows); i++) {
            if (window && i >= window.start && i < window.end) continue;
            const cells2 = /* @__PURE__ */ new Map();
            render.collectors.push(cells2);
            try {
              if (holds(renderRow(node, i, cells2, depth).row)) return { list: node.identity, index: i };
            } catch {
            } finally {
              render.collectors.pop();
            }
          }
        }
        return null;
      }));
    } catch {
      return "null";
    } finally {
      render.candidate = saved;
    }
  }
  function discard(keepUpdates = false) {
    if (keepUpdates && render.candidate) syncPageTitle(pageTitleOf(render.candidate));
    render.candidate = null;
    if (!keepUpdates) page.updates.clear();
  }
  function reset() {
    outsideRender("reset");
    discard();
    page.updates.clear();
    page.committed = null;
    page.locals.clear();
    page.callbacks.clear();
    resetPresses();
    for (const resource of page.resources.values()) resource.dispose();
    page.resources.clear();
    resetActions();
    resetOverlays();
    resetPicker();
    resetTopLayer();
    resetFrameMeasures();
    resetReveals();
    resetEnvReads();
    resetVideos();
    resetWebViews();
    resetSnaps();
    resetStyleWarnings();
    resetClock();
    wireReset();
  }
  var accepts2 = (kind, value) => kind === "number" ? typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 1e9 : kind === "text" ? validInputText(value) : kind === "paste" ? typeof value === "string" : kind === "check" || kind === "focus" ? value === void 0 || typeof value === "boolean" : value === void 0;
  var pressEvent = (timeStamp) => Object.freeze({ timeStamp: typeof timeStamp === "number" ? timeStamp : Date.now() });
  var focusEvent = (focusVisible, timeStamp) => Object.freeze({ focusVisible: focusVisible === true, timeStamp: typeof timeStamp === "number" ? timeStamp : Date.now() });
  function pasteEvent(payload2, timeStamp) {
    const { text, html, selection, plain: plain3, omitted = [] } = JSON.parse(payload2);
    const state = { prevented: false, open: true };
    const event = Object.freeze({
      text,
      ...typeof html === "string" ? { html } : {},
      files: Object.freeze([]),
      omitted: Object.freeze([...omitted]),
      selection: Object.freeze({ start: selection[0], end: selection[1] }),
      plain: plain3 === true,
      timeStamp: typeof timeStamp === "number" ? timeStamp : Date.now(),
      get defaultPrevented() {
        return state.prevented;
      },
      preventDefault() {
        if (state.open) state.prevented = true;
      }
    });
    return { event, state };
  }
  var where = ({ site }) => site ? `${site[0]} of <${site[1]}${typeof site[2] === "string" ? ` data-testid="${site[2]}"` : ""}>` : "A handler";
  var messageOf = (error) => error instanceof Error ? `${error.name === "Error" ? "" : `${error.name}: `}${error.message}` : String(error);
  function dispatch(handle2, value, timeStamp) {
    outsideRender("dispatch");
    const entry = page.callbacks.get(handle2);
    if (!entry || !accepts2(entry.kind, value)) return false;
    const previous = entry.isolated ? new Map(page.updates) : void 0;
    try {
      const epoch2 = actionEpochNow();
      const paste = entry.kind === "paste" ? pasteEvent(value, timeStamp) : null;
      const result = entry.kind === "void" ? entry.callback() : entry.kind === "press" ? entry.callback(pressEvent(timeStamp)) : entry.kind === "focus" ? entry.callback(focusEvent(value, timeStamp)) : paste ? entry.callback(paste.event) : entry.callback(value);
      if (paste) paste.state.open = false;
      if (result && typeof result.then === "function") Promise.resolve(result).catch((thrown) => {
        if (!hasActionEpoch(epoch2)) return;
        reportHandlerFailure(entry.site, sited(thrown));
        const message = `UI_HANDLER: ${where(entry)} rejected: ${messageOf(sited(thrown))}`;
        if (entry.isolated) report(message);
        else failHandler(message);
      });
      return paste?.state.prevented ? "prevented" : true;
    } catch (thrown) {
      const error = sited(thrown);
      reportHandlerFailure(entry.site, error);
      if (error instanceof Error && entry.site) error.message = `UI_HANDLER: ${where(entry)} threw: ${error.message}`;
      page.updates.clear();
      if (!entry.isolated && loudFailures()) throw error;
      if (!entry.isolated) {
        report(error instanceof Error ? error.message : String(error));
        return true;
      }
      for (const [cell, kept2] of previous) page.updates.set(cell, kept2);
      report(error);
      return true;
    }
  }
  function describe(renderPage, view) {
    const output = prepare(renderPage, view);
    commit();
    return output;
  }

  // src/runtime/hot-swap.js
  function exportState() {
    const locals = [];
    for (const [key, cell] of page.locals)
      if (JSON.parse(key).length === 2) locals.push([key, page.updates.has(cell) ? page.updates.get(cell) : cell.value]);
    return JSON.stringify(locals);
  }
  function importState(text) {
    for (const [key, value] of JSON.parse(text)) page.locals.set(key, { value: immutable(value, 0, true) });
  }
  function enableHotSwap() {
    Object.assign(globalThis, { __lingxiaExportState: exportState, __lingxiaImportState: importState });
  }

  // src/host.js
  globalThis.__lingxiaRegisterCatalogue = registerLanguageCatalogue;
  function unboundFieldText(kind, iso3, opts) {
    if (typeof iso3 !== "string") return "";
    try {
      if (kind !== "date" && kind !== "time") return iso3;
      const env = envOf(page.committed?.view);
      return fieldText(kind, iso3, {
        seconds: opts?.seconds === true,
        language: opts?.language || env?.language,
        hourCycle: opts?.hourCycle || env?.hourCycle
      }).displayValue;
    } catch {
      return iso3;
    }
  }
  function pageLeft() {
    const picker2 = hasPicker();
    if (picker2) closePicker();
    const popovers = closePassing();
    return picker2 || popovers;
  }
  function publishHost(render2, catalog2, page2) {
    setActionPage(page2);
    Object.assign(globalThis, {
      __lingxiaRender: render2,
      __lingxiaDescribe: describe,
      __lingxiaPrepare: prepare,
      __lingxiaPrepareWindows: prepareWindows,
      __lingxiaTakeMovedAnchors: takeMovedAnchors,
      __lingxiaCommit: commit,
      __lingxiaDiscard: discard,
      __lingxiaReset: reset,
      __lingxiaDispatch: dispatch,
      __lingxiaCreateView: createView,
      __lingxiaSyncData: syncData,
      __lingxiaConfigureActions: configureActions,
      __lingxiaTakeViewEvents: takeViewEvents,
      __lingxiaReceiveActionResponse: receiveActionResponse,
      __lingxiaTakeActionErrors: takeActionErrors,
      __lingxiaTakeHandlerFailure: takeHandlerFailure,
      __lingxiaSetLoudFailures: setLoudFailures,
      __lingxiaHasLocalUpdates: hasLocalUpdates,
      __lingxiaReleasePress: releasePress,
      __lingxiaRequestLeave: requestLeave,
      __lingxiaReadsHostContext: readsHostContext,
      __lingxiaEnvReads: envReads,
      __lingxiaSetNow: setNow,
      __lingxiaAutomationAction: automationAction,
      __lingxiaTakeAutomationResults: takeAutomationResults,
      __lingxiaOverlayTimer: overlayTimer,
      __lingxiaTakeOverlayTimers: takeOverlayTimers,
      __lingxiaEscape: escapeOverlay,
      __lingxiaPageLeft: pageLeft,
      __lingxiaInterest: interest,
      __lingxiaDismissHints: dismissHints,
      __lingxiaCloseAfter: closeAfterExpired,
      __lingxiaFonts: takeFonts,
      __lingxiaProbeListRow: probeListRow,
      __lingxiaOpenPicker: openPicker,
      __lingxiaPickerKey: pickerKey,
      __lingxiaClosePicker: closePicker,
      __lingxiaTakePickerResults: takePickerResults,
      __lingxiaPickerIdentity: pickerIdentity,
      __lingxiaOwnedOverlays: () => JSON.stringify({ picker: pickerIdentity(), popovers: ownedPopovers() }),
      __lingxiaCloseOrphans: (picker2, ids) => {
        if (picker2) closePicker();
        closeOrphans(ids);
      },
      __lingxiaTakeFrameMeasures: takeFrameMeasures,
      __lingxiaFrameMeasured: frameMeasured,
      __lingxiaWireAck: wireAck,
      __lingxiaTakeReveals: takeReveals,
      __lingxiaEvalSpans: takeSpans,
      __lingxiaTakeLowered: takeLowered,
      __lingxiaSeedLowered: seedLowered,
      __lingxiaFieldText: unboundFieldText,
      // Cleared when a render completes, so it names only a render that never returned.
      __lingxiaSite: () => render.site || "",
      __lingxiaThrown: takeThrown
    });
    if (catalog2 !== void 0) globalThis.__lingxiaActionCatalog = catalog2;
  }

  // <stdin>
  Object.assign(globalThis, internal_exports, { __lingxiaPublish: publishHost });
})();
