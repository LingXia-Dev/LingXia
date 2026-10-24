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

  // src/runtime/state.js
  var render = { active: false, candidate: null, scope: null, view: null, lookupLocals: null, collectors: [] };
  var page = { committed: null, locals: /* @__PURE__ */ new Map(), callbacks: /* @__PURE__ */ new Map(), updates: /* @__PURE__ */ new Map(), resources: /* @__PURE__ */ new Map() };
  function outsideRender(what) {
    if (render.active) throw Error(`UI_RENDER: ${what} during render`);
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
  function readNode(node) {
    const [owner, offset] = node[brand];
    if (owner !== epoch || !arena[offset]) throw Error("UI_STALE: candidate from another render");
    return arena[offset];
  }
  var isNode = (value) => Boolean(value && typeof value === "object" && brand in value);
  var nodeKey = (value) => isNode(value) ? readNode(value)[1].key : void 0;
  function h(tag, props, ...children) {
    if (!render.active) throw Error("UI_SCOPE: JSX outside render");
    if (arena.length >= 16384) throw Error("UI_LIMIT: candidate arena too large");
    const handle2 = { [brand]: [epoch, arena.length] };
    const { children: supplied, __loc: location, ...attributes } = props ?? {};
    arena.push([tag, attributes, children.length ? children : supplied === void 0 ? [] : [supplied], location]);
    return handle2;
  }
  function flatten(children, depth = 0, result = []) {
    if (depth > 128) throw Error("UI_LIMIT: children too deep");
    for (const child of children) {
      if (result.length >= 16384) throw Error("UI_LIMIT: too many children");
      if (Array.isArray(child)) flatten(child, depth + 1, result);
      else if (child !== null && child !== void 0 && typeof child !== "boolean" && child !== "") result.push(child);
    }
    return result;
  }
  function childrenToArray(children) {
    return flatten([children]);
  }

  // src/generated/contract.js
  var elements = { "view": { "wire": "column", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "alignItems", "justifyContent", "flexWrap", "display", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": {} }, "column": { "wire": "column", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "alignItems", "justifyContent", "flexWrap", "display", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": {} }, "row": { "wire": "row", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "alignItems", "justifyContent", "flexWrap", "display", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": {} }, "page": { "wire": "page", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "alignItems", "justifyContent", "flexWrap", "display", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": {} }, "text": { "wire": "text", "children": "text", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "lineClamp", "textOverflow"], "props": {} }, "button": { "wire": "button", "children": "content", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "alignItems", "justifyContent", "flexWrap", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "onClick": false, "disabled": false, "accessibilityLabel": false } }, "spacer": { "wire": "spacer", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation"], "props": {} }, "scroll": { "wire": "scroll", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "alignItems", "justifyContent", "flexWrap", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "overflowX", "overflowY"], "props": { "onReachStart": false, "onReachEnd": false } }, "list": { "wire": "list", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "count": true, "row": true, "height": false, "anchor": false, "itemKeys": false, "onReachStart": false, "onReachEnd": false } }, "image": { "wire": "image", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "objectFit"], "props": { "src": true, "alt": false, "retryKey": false, "onLoad": false, "onError": false } }, "input": { "wire": "input", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "placeholderColor"], "props": { "accessibilityLabel": false, "placeholder": false, "secure": false, "keyboard": false, "autoFocus": false, "disabled": false, "onSubmit": false }, "binding": { "type": "string", "event": "onInput" } }, "textarea": { "wire": "textarea", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "placeholderColor"], "props": { "accessibilityLabel": false, "placeholder": false, "secure": false, "keyboard": false, "autoFocus": false, "disabled": false, "submitOnEnter": false, "onSubmit": false }, "binding": { "type": "string", "event": "onInput" } }, "search": { "wire": "search", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "placeholderColor"], "props": { "accessibilityLabel": false, "placeholder": false, "autoFocus": false, "disabled": false, "onSubmit": false }, "binding": { "type": "string", "event": "onInput" } }, "checkbox": { "wire": "checkbox", "children": "content", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "alignItems", "justifyContent", "flexWrap", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "accessibilityLabel": false, "disabled": false }, "binding": { "type": "boolean", "event": "onChange" } }, "switch": { "wire": "switch", "children": "text", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "accessibilityLabel": false, "disabled": false }, "binding": { "type": "boolean", "event": "onChange" } }, "radio": { "wire": "radio", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "options": true, "accessibilityLabel": false, "disabled": false }, "binding": { "type": "string", "event": "onChange" } }, "slider": { "wire": "slider", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "color"], "props": { "min": false, "max": false, "step": false, "accessibilityLabel": true, "disabled": false }, "binding": { "type": "number", "event": "onChange" } }, "grid": { "wire": "grid", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "gridTemplateColumns", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": {} }, "stack": { "wire": "stack", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": {} }, "plot": { "wire": "plot", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation"], "props": { "kind": true, "values": true, "colors": true, "accessibilityLabel": true, "smooth": false, "area": false, "selectedIndex": false, "domainMin": false, "domainMax": false, "onSelect": false, "pieLayout": false, "markerEmphasisScale": false } }, "vector": { "wire": "vector", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "position", "inset", "top", "right", "bottom", "left", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderColor", "boxShadow", "backdropFilter", "animation", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "color", "objectFit"], "props": { "paths": true, "viewBox": false } } };
  var style = { "width": { "lower": "dimension", "wire": "width" }, "height": { "lower": "dimension", "wire": "height" }, "minWidth": { "lower": "dimension", "wire": "minWidth" }, "minHeight": { "lower": "dimension", "wire": "minHeight" }, "maxWidth": { "lower": "dimension", "wire": "maxWidth" }, "maxHeight": { "lower": "dimension", "wire": "maxHeight" }, "margin": { "lower": "margin", "wire": ["marginTop", "marginRight", "marginBottom", "marginLeft"] }, "marginTop": { "lower": "margin", "wire": "marginTop" }, "marginRight": { "lower": "margin", "wire": "marginRight" }, "marginBottom": { "lower": "margin", "wire": "marginBottom" }, "marginLeft": { "lower": "margin", "wire": "marginLeft" }, "position": { "lower": "option", "wire": "position" }, "inset": { "lower": "inset", "wire": ["insetTop", "insetRight", "insetBottom", "insetLeft"] }, "top": { "lower": "inset", "wire": "insetTop" }, "right": { "lower": "inset", "wire": "insetRight" }, "bottom": { "lower": "inset", "wire": "insetBottom" }, "left": { "lower": "inset", "wire": "insetLeft" }, "zIndex": { "lower": "z-index", "wire": "zIndex" }, "flexGrow": { "lower": "number", "wire": "grow" }, "flexShrink": { "lower": "number", "wire": "shrink" }, "flexBasis": { "lower": "basis", "wire": "flexBasis" }, "alignSelf": { "lower": "alignment", "wire": "alignSelf" }, "opacity": { "lower": "opacity", "wire": "opacity", "paint": true }, "overflow": { "lower": "option", "wire": "overflow" }, "backgroundColor": { "lower": "color", "wire": "background", "paint": true }, "backgroundImage": { "lower": "gradient", "wire": "backgroundGradient", "paint": true }, "borderRadius": { "lower": "length", "wire": ["radius", "radiusTopLeft", "radiusTopRight", "radiusBottomRight", "radiusBottomLeft"] }, "borderTopLeftRadius": { "lower": "length", "wire": "radiusTopLeft" }, "borderTopRightRadius": { "lower": "length", "wire": "radiusTopRight" }, "borderBottomRightRadius": { "lower": "length", "wire": "radiusBottomRight" }, "borderBottomLeftRadius": { "lower": "length", "wire": "radiusBottomLeft" }, "borderWidth": { "lower": "length", "wire": ["borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth"] }, "borderTopWidth": { "lower": "length", "wire": "borderTopWidth" }, "borderRightWidth": { "lower": "length", "wire": "borderRightWidth" }, "borderBottomWidth": { "lower": "length", "wire": "borderBottomWidth" }, "borderLeftWidth": { "lower": "length", "wire": "borderLeftWidth" }, "borderColor": { "lower": "color", "wire": "borderColor", "paint": true }, "boxShadow": { "lower": "shadow", "wire": "boxShadow", "paint": true }, "backdropFilter": { "lower": "backdrop", "wire": ["backdropBlur", "backdropSaturation"] }, "animation": { "lower": "animation", "wire": "animation", "internal": true }, "padding": { "lower": "length", "wire": ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"] }, "paddingTop": { "lower": "length", "wire": "paddingTop" }, "paddingRight": { "lower": "length", "wire": "paddingRight" }, "paddingBottom": { "lower": "length", "wire": "paddingBottom" }, "paddingLeft": { "lower": "length", "wire": "paddingLeft" }, "display": { "lower": "option", "wire": "display" }, "flexDirection": { "lower": "option", "wire": "flexDirection" }, "alignItems": { "lower": "alignment", "wire": "align" }, "justifyContent": { "lower": "justification", "wire": "justify" }, "flexWrap": { "lower": "option", "wire": "flexWrap" }, "gap": { "lower": "length", "wire": "gap" }, "rowGap": { "lower": "length", "wire": "rowGap" }, "columnGap": { "lower": "length", "wire": "columnGap" }, "gridTemplateColumns": { "lower": "columns", "wire": "gridColumns" }, "color": { "lower": "color", "wire": "color", "paint": true }, "fontSize": { "lower": "font-size", "wire": "fontSize" }, "fontWeight": { "lower": "font-weight", "wire": "fontWeight" }, "fontFamily": { "lower": "family", "wire": "fontFamily" }, "fontStyle": { "lower": "option", "wire": "fontStyle" }, "lineHeight": { "lower": "line-height", "wire": "lineHeight" }, "letterSpacing": { "lower": "letter-spacing", "wire": "letterSpacing" }, "textAlign": { "lower": "option", "wire": "textAlign" }, "textDecorationLine": { "lower": "option", "wire": "textDecorationLine" }, "textTransform": { "lower": "option", "wire": "textTransform" }, "whiteSpace": { "lower": "option", "wire": "whiteSpace" }, "wordBreak": { "lower": "option", "wire": "wordBreak" }, "overflowWrap": { "lower": "option", "wire": "overflowWrap" }, "lineClamp": { "lower": "line-clamp", "wire": "textLines" }, "textOverflow": { "lower": "option", "wire": "textOverflow" }, "overflowX": { "lower": "scroll-axis", "wire": "scrollAxis" }, "overflowY": { "lower": "scroll-axis", "wire": "scrollAxis" }, "placeholderColor": { "lower": "color", "wire": "placeholderColor", "paint": true }, "objectFit": { "lower": "fit", "wire": "imageFit" } };

  // src/style/classes.js
  var byName = /* @__PURE__ */ new Map();
  var cache = /* @__PURE__ */ new Map();
  var cacheEnv = "";
  var states = ["focus", "active", "hover", "disabled"];
  function copy(value) {
    if (Array.isArray(value)) return value.map(copy);
    if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, copy(item)]));
    return value;
  }
  function configureClasses(compiled) {
    byName = /* @__PURE__ */ new Map();
    cache.clear();
    compiled.forEach((rule, order) => {
      if (!byName.has(rule.name)) byName.set(rule.name, []);
      byName.get(rule.name).push({ ...copy(rule), order });
    });
  }
  function applies(rule, env) {
    const { theme, minWidth, maxWidth } = rule.when ?? {};
    if (theme && (env.theme ?? "light") !== theme) return false;
    if (minWidth !== void 0 && !(env.viewport?.width >= minWidth)) return false;
    if (maxWidth !== void 0 && !(env.viewport?.width < maxWidth)) return false;
    return true;
  }
  var pixels = (value) => `${value}px`;
  function cascade(rules) {
    const style2 = {}, gradient = {}, shadow = {}, backdrop2 = {};
    for (const rule of rules) {
      Object.assign(style2, copy(rule.style));
      Object.assign(gradient, rule.gradient);
      Object.assign(shadow, rule.shadow);
      Object.assign(backdrop2, rule.backdrop);
    }
    const filter = backdrop2.mode === "fixed" ? backdrop2.fixed : backdrop2.mode === "variables" ? backdrop2 : void 0;
    if (filter && (filter.blur != null || filter.saturation != null))
      style2.backdropFilter = `blur(${pixels(filter.blur ?? 0)}) saturate(${filter.saturation ?? 1})`;
    if (gradient.enabled !== void 0) {
      style2.backgroundImage = "none";
      if (gradient.enabled && gradient.source && (gradient.source !== "via" || gradient.via)) {
        const stops = [{ offset: gradient.fromOffset ?? 0, color: gradient.fromColor ?? "#00000000" }];
        if (gradient.via) stops.push({ offset: gradient.viaOffset ?? 0.5, color: gradient.viaColor ?? "#00000000" });
        stops.push({ offset: gradient.toOffset ?? 1, color: gradient.toColor ?? "#00000000" });
        for (let i = 1; i < stops.length; i++) stops[i].offset = Math.max(stops[i - 1].offset, stops[i].offset);
        style2.backgroundImage = { ...gradient.orientation ?? { direction: "bottom" }, colorSpace: gradient.colorSpace ?? "srgb", stops };
      }
    }
    if (shadow.enabled) {
      style2.boxShadow = (shadow.composite === false ? shadow.fixedLayers ?? [] : shadow.layers ?? []).map(({ useColor, ...layer }) => ({
        ...layer,
        color: useColor && shadow.color !== void 0 ? shadow.color : layer.color
      }));
      if (shadow.composite !== false && shadow.ringWidth > 0)
        style2.boxShadow.unshift({ offsetX: 0, offsetY: 0, blur: 0, spread: shadow.ringWidth, color: shadow.ringColor ?? "currentColor" });
      if (style2.boxShadow.length > 8) throw Error("UI_STYLE: too many box shadows");
    }
    return style2;
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
    const environment = `${env.theme ?? "light"}|${env.viewport?.width}|${env.viewport?.height}|${env.pointer}`;
    if (environment !== cacheEnv) {
      cache.clear();
      cacheEnv = environment;
    }
    if (cache.has(className)) return copy(cache.get(className));
    const matched = [];
    for (const name of new Set(className.split(/\s+/).filter(Boolean))) {
      const rules = byName.get(name);
      if (!rules) throw Error(`UI_CLASS: ${name} was not compiled; use complete utility names in source`);
      for (const rule of rules) {
        if (rule.error) throw Error(`UI_CLASS: ${name}: ${rule.error.message}`);
        if (applies(rule, env)) matched.push(rule);
      }
    }
    matched.sort((a, b) => Number(!!a.when?.state) - Number(!!b.when?.state) || a.order - b.order);
    const own2 = matched.filter((rule) => !rule.when?.children);
    const plain = own2.filter((rule) => !rule.when?.state);
    const base = cascade(plain);
    const result = { base };
    for (const state of states) {
      const extra = own2.filter((rule) => rule.when?.state === state);
      if (!extra.length || state === "hover" && env.pointer !== "fine") continue;
      const full = cascade([...plain, ...extra]);
      result[state] = state === "disabled" ? full : changed(full, base);
    }
    const children = matched.filter((rule) => rule.when?.children);
    if (children.length) result.children = cascade(children);
    if (cache.size >= 256) cache.delete(cache.keys().next().value);
    cache.set(className, copy(result));
    return result;
  }

  // src/generated/wire.js
  var tags = { "column": 0, "row": 1, "text": 2, "button": 3, "spacer": 4, "scroll": 5, "list": 6, "image": 7, "input": 8, "textarea": 9, "checkbox": 10, "switch": 11, "radio": 12, "radio-option": 13, "search": 14, "slider": 15, "grid": 16, "stack": 17, "plot": 18, "vector": 19, "page": 20 };
  var field = { "width": 0, "height": 1, "gap": 2, "padding": 3, "fontSize": 4, "color": 5, "background": 6, "grow": 7, "onClick": 8, "enabled": 9, "radius": 10, "listId": 11, "listCount": 12, "listLeading": 13, "listTrailing": 14, "listRowIndex": 15, "listAnchor": 16, "onReachStart": 17, "onReachEnd": 18, "imageSrc": 19, "imageFit": 20, "inputValue": 21, "placeholder": 22, "secure": 23, "keyboard": 24, "autoFocus": 25, "onInput": 26, "onSubmit": 27, "minWidth": 28, "minHeight": 29, "maxWidth": 30, "maxHeight": 31, "shrink": 32, "align": 33, "justify": 34, "alignSelf": 35, "fontWeight": 36, "lineHeight": 37, "fontFamily": 38, "checked": 39, "onCheckedChange": 40, "accessibilityLabel": 41, "sliderValue": 42, "sliderMin": 43, "sliderMax": 44, "sliderStep": 45, "onSliderChange": 46, "gridColumns": 47, "offsetX": 48, "offsetY": 49, "textLines": 50, "plotValues": 51, "plotKind": 52, "plotColors": 53, "plotSmooth": 54, "plotArea": 55, "onPlotSelect": 56, "plotSelectedIndex": 57, "opacity": 58, "paddingTop": 59, "paddingRight": 60, "paddingBottom": 61, "paddingLeft": 62, "wordBreak": 63, "overflowWrap": 64, "borderWidth": 65, "borderColor": 66, "borderTopWidth": 67, "borderRightWidth": 68, "borderBottomWidth": 69, "borderLeftWidth": 70, "textAlign": 71, "backgroundGradient": 72, "boxShadow": 73, "widthPercent": 74, "heightPercent": 75, "minWidthPercent": 76, "minHeightPercent": 77, "maxWidthPercent": 78, "maxHeightPercent": 79, "vectorSource": 80, "textDecorationLine": 81, "heightAuto": 82, "submitOnEnter": 83, "fontStyle": 84, "plotDomainMin": 85, "plotDomainMax": 86, "flexWrap": 87, "plotPieLayout": 88, "letterSpacing": 89, "radiusTopLeft": 90, "radiusTopRight": 91, "radiusBottomRight": 92, "radiusBottomLeft": 93, "plotMarkerEmphasisScale": 94, "backdropBlur": 95, "backdropSaturation": 96, "placeholderColor": 97, "focusStyle": 98, "flexBasis": 99, "scrollAxis": 100, "animation": 101, "listItemKeys": 102, "flexDirection": 103, "display": 104, "imageAlt": 105, "imageRetryKey": 106, "onImageLoad": 107, "onImageError": 108, "marginTop": 109, "marginRight": 110, "marginBottom": 111, "marginLeft": 112, "position": 113, "insetTop": 114, "insetRight": 115, "insetBottom": 116, "insetLeft": 117, "zIndex": 118, "overflow": 119, "rowGap": 120, "columnGap": 121, "textTransform": 122, "whiteSpace": 123, "textOverflow": 124, "pressedStyle": 125, "hoverStyle": 126, "widthKeyword": 127, "heightKeyword": 128 };
  var fieldOptions = { "align": ["start", "center", "end", "stretch", "baseline"], "justify": ["start", "center", "end", "between", "around", "evenly"], "alignSelf": ["start", "center", "end", "stretch", "baseline"], "fontFamily": ["sans", "mono"], "wordBreak": ["normal", "break-all", "keep-all"], "overflowWrap": ["normal", "break-word", "anywhere"], "textAlign": ["start", "end", "left", "center", "right", "justify"], "textDecorationLine": ["none", "line-through", "underline"], "fontStyle": ["normal", "italic"], "flexWrap": ["nowrap", "wrap", "wrap-reverse"], "scrollAxis": ["horizontal", "vertical"], "flexDirection": ["row", "column", "row-reverse", "column-reverse"], "display": ["flex"], "position": ["relative", "absolute"], "overflow": ["visible", "hidden"], "textTransform": ["none", "uppercase", "lowercase", "capitalize"], "whiteSpace": ["normal", "nowrap"], "textOverflow": ["clip", "ellipsis"], "widthKeyword": ["fit-content", "max-content", "min-content"], "heightKeyword": ["fit-content", "max-content", "min-content"] };

  // src/style/shadow.js
  function normalizeShadows(value) {
    if (value === "none") return value;
    const invalid2 = () => {
      throw Error("UI_SHADOW: invalid box shadow");
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
      if (result.color !== "currentColor" && (typeof result.color !== "string" || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(result.color))) invalid2();
      return result;
    });
  }

  // src/style/gradient.js
  var directions = ["top", "right", "bottom", "left", "top-right", "bottom-right", "bottom-left", "top-left"];
  function normalizeGradient(value) {
    if (value === "none") return value;
    const invalid2 = () => {
      throw Error("UI_GRADIENT: invalid linear gradient");
    };
    if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some((key) => !["direction", "angle", "colorSpace", "stops"].includes(key)) || value.direction !== void 0 && !directions.includes(value.direction) || value.angle !== void 0 && (typeof value.angle !== "number" || !Number.isFinite(value.angle) || Math.abs(value.angle) > 36e3) || value.direction !== void 0 && value.angle !== void 0 || value.colorSpace !== void 0 && !["srgb", "oklab"].includes(value.colorSpace) || !Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 16) invalid2();
    let previous = -Infinity;
    const stops = value.stops.map((stop) => {
      if (!stop || typeof stop !== "object" || Array.isArray(stop) || Object.keys(stop).some((key) => key !== "offset" && key !== "color") || typeof stop.offset !== "number" || !Number.isFinite(stop.offset) || Math.abs(stop.offset) > 16 || stop.offset < previous || typeof stop.color !== "string" || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(stop.color)) invalid2();
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

  // src/runtime/data.js
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
  var longhands = Object.fromEntries(Object.entries(style).filter(([name]) => shorthand(name)).map(([name, entry]) => [name, Object.keys(style).filter((other) => !shorthand(other) && entry.wire.includes(style[other].wire))]));
  function mergeStyle(...layers) {
    const result = {};
    for (const layer of layers) {
      if (!layer) continue;
      for (const name of Object.keys(layer)) for (const longhand of longhands[name] ?? []) if (!(longhand in layer)) delete result[longhand];
      Object.assign(result, layer);
    }
    return result;
  }
  function admitStyle(tag, style2) {
    const accepted = elements[tag].style;
    for (const name of Object.keys(style2)) {
      if (!has(style, name)) throw Error(`UI_STYLE: unknown style property ${name}`);
      if (!accepted.includes(name)) throw Error(`UI_STYLE: ${name} is not supported on <${tag}>`);
    }
  }
  function invalid(name) {
    throw Error(`UI_STYLE_VALUE: ${name}`);
  }
  function length(name, value, { signed = false, max = 16384 } = {}) {
    if (!finite(value) || value > max || value < (signed ? -max : 0)) invalid(name);
    return value;
  }
  function dimension(name, value, env, out, key) {
    const viewport = typeof value === "string" && value.match(/^(\d+(?:\.\d+)?)(vw|vh)$/);
    if (viewport) {
      const extent = env?.viewport?.[viewport[2] === "vw" ? "width" : "height"];
      if (!finite(extent)) invalid(name);
      out[key] = length(name, extent * Number(viewport[1]) / 100);
    } else if (percent(value) !== void 0) out[`${key}Percent`] = length(name, percent(value));
    else if (value === "auto" && key === "height") out.heightAuto = 1;
    else if (value === "auto" && key === "width") {
    } else if (["fit-content", "max-content", "min-content"].includes(value) && (key === "width" || key === "height")) out[`${key}Keyword`] = value;
    else out[key] = length(name, value);
  }
  var alignment = { "flex-start": "start", "flex-end": "end" };
  var justification = { "flex-start": "start", "flex-end": "end", "space-between": "between", "space-around": "around", "space-evenly": "evenly" };
  function option(name, key, value) {
    if (!fieldOptions[key]?.includes(value)) invalid(name);
    return value;
  }
  function backdrop(name, value) {
    if (value === "none") return [0, 1];
    const match = typeof value === "string" && value.trim().match(/^(?:blur\((\d+(?:\.\d+)?)px\))?\s*(?:saturate\((\d+(?:\.\d+)?)(%?)\))?$/);
    if (!match || match[1] === void 0 && match[2] === void 0) invalid(name);
    const blur = match[1] === void 0 ? 0 : Number(match[1]);
    const saturation = match[2] === void 0 ? 1 : Number(match[2]) / (match[3] ? 100 : 1);
    if (blur > 64 || saturation > 16) invalid(name);
    return [blur, saturation];
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
  function lowerStyle(tag, style2, env, inherited = { fontSize: 14 }) {
    admitStyle(tag, style2);
    const out = {};
    const fontSize = style2.fontSize === void 0 ? inherited.fontSize : length("fontSize", style2.fontSize);
    if (style2.fontSize !== void 0 && style2.fontSize <= 0) invalid("fontSize");
    let lineHeight = inherited.lineHeight;
    const names = Object.keys(style2).sort((a, b) => Number(shorthand(b)) - Number(shorthand(a)));
    for (const name of names) {
      const value = style2[name], entry = style[name], key = entry.wire;
      switch (entry.lower) {
        case "dimension":
          dimension(name, value, env, out, key);
          break;
        case "margin":
          if (value !== "auto") length(name, value, { signed: true });
          for (const side of [key].flat()) out[side] = value;
          break;
        case "inset": {
          const amount = percent(value) !== void 0 ? percent(value) : value;
          length(name, amount, { signed: true });
          for (const side of [key].flat()) out[side] = value;
          break;
        }
        case "length":
          length(name, value);
          for (const side of [key].flat()) out[side] = value;
          break;
        case "z-index":
          if (!Number.isInteger(value) || Math.abs(value) > 1e4) invalid(name);
          out[key] = value;
          break;
        case "columns": {
          const count = typeof value === "string" ? Number(value.match(/^repeat\((\d+),\s*minmax\(0,\s*1fr\)\)$/)?.[1]) : value;
          if (!Number.isInteger(count) || count < 1 || count > 64) invalid(name);
          out[key] = count;
          break;
        }
        case "line-clamp":
          if (!Number.isInteger(value) || value < 1 || value > 1024) invalid(name);
          out[key] = value;
          break;
        case "number":
          out[key] = length(name, value);
          break;
        case "opacity":
          if (!finite(value) || value < 0 || value > 1) invalid(name);
          out[key] = value;
          break;
        case "font-size":
          out[key] = fontSize;
          break;
        case "font-weight":
          if (!finite(value) || value < 1 || value > 1e3) invalid(name);
          out[key] = value;
          break;
        case "color":
          if (typeof value !== "string" || !value) invalid(name);
          out[key] = value;
          break;
        case "gradient":
          out[key] = normalizeGradient(value);
          break;
        case "shadow":
          out[key] = normalizeShadows(value);
          break;
        case "backdrop":
          [out[key[0]], out[key[1]]] = backdrop(name, value);
          break;
        case "animation": {
          const lowered = animation(value);
          if (lowered !== void 0) out[key] = lowered;
          break;
        }
        case "basis":
          if (value !== "auto") length(name, percent(value) ?? value);
          out[key] = value;
          break;
        case "option":
          out[key] = option(name, key, value);
          break;
        case "alignment":
          out[key] = option(name, key, alignment[value] ?? value);
          break;
        case "justification":
          out[key] = option(name, key, justification[value] ?? value);
          break;
        case "family":
          out[key] = option(name, key, { "sans-serif": "sans", monospace: "mono" }[value]);
          break;
        case "line-height":
          if (typeof value === "string" && /^\d+(?:\.\d+)?px$/.test(value)) lineHeight = length(name, Number(value.slice(0, -2)));
          else if (finite(value) && value > 0 && value <= 64) lineHeight = { multiplier: value };
          else invalid(name);
          break;
        case "letter-spacing": {
          const em = typeof value === "string" && value.match(/^(-?\d+(?:\.\d+)?)em$/);
          const pixels2 = em ? fontSize * Number(em[1]) : value;
          out[key] = length(name, pixels2, { signed: true, max: 1024 });
          break;
        }
        case "scroll-axis":
          if (value !== "auto" || style2.overflowX !== void 0 && style2.overflowY !== void 0) invalid(name);
          out[key] = name === "overflowX" ? "horizontal" : "vertical";
          break;
        case "fit":
          if (!(tag === "vector" ? ["contain", "fill"] : ["cover", "contain", "fill"]).includes(value)) invalid(name);
          out[key] = value;
          break;
        default:
          throw Error(`UI_STYLE: ${name} has no native lowering`);
      }
    }
    if (style2.lineHeight !== void 0 || typeof lineHeight === "object") {
      const pixels2 = typeof lineHeight === "object" ? fontSize * lineHeight.multiplier : lineHeight;
      if (!(pixels2 > 0 && pixels2 <= 16384)) invalid("lineHeight");
      out.lineHeight = pixels2;
    }
    return { fields: out, typography: { fontSize, lineHeight } };
  }
  function paintOnly(state, tag, style2) {
    admitStyle(tag, style2);
    for (const name of Object.keys(style2))
      if (!style[name].paint) throw Error(`UI_STYLE: ${state}: supports paint properties only, not ${name}`);
  }

  // src/runtime/errors.js
  function located(error, frame) {
    const base = error instanceof Error ? error : Error(String(error));
    if (!base.uiFrames) {
      base.uiFrames = [];
      base.uiMessage = base.message;
    }
    if (base.uiFrames.length < 12) base.uiFrames.push(frame);
    base.message = [base.uiMessage, ...base.uiFrames.map((line) => `  at ${line}`)].join("\n");
    return base;
  }
  function bridgeError(code, message, data) {
    const error = new Error(message ?? code);
    error.name = "LxBridgeError";
    error.code = code;
    if (data !== void 0) error.data = data;
    return error;
  }
  function resourceError(error) {
    if (typeof error?.code === "string") return Object.freeze({ code: error.code, message: String(error.message ?? error.code) });
    const text = String(error?.uiMessage ?? error?.message ?? error);
    const match = text.match(/^([A-Z][A-Z0-9_]+): ([\s\S]*)$/);
    return Object.freeze(match ? { code: match[1], message: match[2] } : { code: "UI_ERROR", message: text });
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
    const finish = (cause, code, reason, discard2 = false) => {
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
      for (const fn of [...listeners.close]) invoke(fn, code, reason);
      for (const bucket of Object.values(listeners)) bucket.clear();
    };
    const handle2 = Object.freeze({
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

  // src/runtime/leave-guard.js
  function createLeaveGuards(callHost, report2) {
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
          report2(error);
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
          if (current === epoch2) report2(error);
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
      }
    };
  }

  // src/runtime/actions.js
  var actions = Object.freeze(/* @__PURE__ */ Object.create(null));
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
  var leaveGuards = createLeaveGuards(callLeaveHost, report);
  function currentActions() {
    return { actions, channels };
  }
  function actionInfo(action) {
    const name = actionNames.get(action);
    return name !== void 0 && actions[name] === action ? { name, mode: actionModes.get(action) } : void 0;
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
    channels = closedChannels;
    for (const { reject } of pending) reject(bridgeError("BRIDGE_CANCELED", "Page reset"));
    for (const entry of oldChannels) {
      const error = bridgeError("BRIDGE_CANCELED", "Page reset");
      entry.reject(error);
      entry.channel.reject(error);
    }
  }
  function resetActions() {
    cancelActions();
    actionErrors = [];
  }
  function admit(what) {
    if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256) throw Error(`UI_LIMIT: too many pending ${what}`);
  }
  function encodeFrame(frame, what) {
    const encoded = JSON.stringify(frame);
    if (encoded.length > 65536) throw Error(`UI_LIMIT: ${what} payload too large`);
    return encoded;
  }
  function configureActions(metadata) {
    outsideRender("configure actions");
    const names = metadata?.__names, modes = metadata?.__modes;
    if (!Array.isArray(names) || names.length > 256 || !modes || typeof modes !== "object" || names.some((name) => typeof name !== "string" || !name || name.length > 256 || !has(modes, name) || !["call", "stream"].includes(modes[name])) || new Set(names).size !== names.length) throw Error("UI_ACTION: invalid action catalog");
    cancelActions();
    const epoch2 = actionEpoch, built = /* @__PURE__ */ Object.create(null);
    const reportLive = (cause) => {
      if (epoch2 === actionEpoch) report(cause);
    };
    for (const name of names) built[name] = (...args) => {
      outsideRender("action");
      if (epoch2 !== actionEpoch) throw bridgeError("BRIDGE_CANCELED", "Stale page action");
      if (args.length > 1) throw Error("UI_ACTION: expected at most one JSON payload");
      admit("actions");
      const params = args.length && args[0] !== void 0 ? immutable(args[0]) : void 0;
      const id = `ui-action:${nextRequest++}`;
      const encoded = encodeFrame({ v: 2, kind: "req", id, method: name, cap: "page", ...params === void 0 ? {} : { params } }, "action");
      if (modes[name] === "stream") {
        const stream = createStream(id, (cause) => {
          if (!pendingActions.has(id)) return;
          if (requests.length >= 512) throw Error("UI_LIMIT: too many pending cancellations");
          requests.push(JSON.stringify({ v: 2, kind: "cancel", id }));
          pendingActions.delete(id);
          stream.reject(cause ?? bridgeError("BRIDGE_CANCELED", "Stream canceled"));
        }, reportLive, bridgeError);
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
      actionModes.set(built[name], modes[name]);
      actionNames.set(built[name], name);
    }
    actions = Object.freeze(built);
    channels = Object.freeze({
      open(topic, params) {
        outsideRender("channel");
        if (epoch2 !== actionEpoch) throw bridgeError("BRIDGE_CANCELED", "Stale page channel");
        if (typeof topic !== "string" || !has(built, topic) || topic.includes(".") || modes[topic] !== "call")
          throw Error("UI_CHANNEL: expected a page channel handler");
        admit("channels");
        const id = `ui-channel:${nextRequest++}`;
        const encoded = encodeFrame({ v: 2, kind: "ch.open", id, topic, cap: "page", ...params === void 0 ? {} : { params: immutable(params) } }, "channel");
        let outgoing = 0;
        const channel = createChannel(id, (payload) => {
          outsideRender("channel send");
          const entry = activeChannels.get(id);
          if (epoch2 !== actionEpoch || !entry?.open) throw bridgeError("STREAM_CLOSED", "Channel is closed");
          if (requests.length >= 256) throw Error("UI_LIMIT: too many pending channel frames");
          requests.push(encodeFrame({ v: 2, kind: "ch.data", id, seq: outgoing, payload: immutable(payload) }, "channel"));
          outgoing++;
        }, (code, reason, cause) => {
          outsideRender("channel close");
          if (!activeChannels.has(id)) return;
          if (typeof code !== "string" || code.length > 256 || typeof reason !== "string" || reason.length > 2048)
            throw Error("UI_CHANNEL: invalid close reason");
          if (requests.length >= 512) throw Error("UI_LIMIT: too many pending cancellations");
          requests.push(JSON.stringify({ v: 2, kind: "ch.close", id, code, reason }));
          activeChannels.delete(id);
          if (cause) channel.reject(cause);
          else channel.finish(code, reason, true);
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
    if (!["navigation.setLeaveGuard", "navigation.leave"].includes(method)) throw Error("UI_LEAVE: invalid control");
    admit("actions");
    const id = `ui-action:${nextRequest++}`;
    const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
    promise.catch(() => {
    });
    requests.push(JSON.stringify({ v: 2, kind: "req", id, method: `host.${method}`, cap: "host", params }));
    return promise;
  }
  function requestLeave(reason) {
    outsideRender("leave request");
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
  function hasActionEpoch(epoch2) {
    return epoch2 === actionEpoch;
  }
  function actionEpochNow() {
    return actionEpoch;
  }
  var failure = (frame) => bridgeError(
    typeof frame.error?.code === "string" ? frame.error.code : "BRIDGE_INTERNAL_ERROR",
    typeof frame.error?.message === "string" ? frame.error.message : void 0,
    frame.error?.data === void 0 ? void 0 : immutable(frame.error.data)
  );
  function receiveActionResponse(frame) {
    outsideRender("response");
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
    const value = frame.ok ? immutable(frame.result ?? null) : failure(frame);
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
        const current = ++ticket;
        update({ status: "streaming", error: null, result: null });
        try {
          if (!definition.action) throw Error("UI_STREAM: bridge not ready");
          stream = definition.action(...params);
          if (definition.onData) stream.on("data", (value) => {
            if (!mounted || current !== ticket) return;
            try {
              definition.onData(value);
            } catch (error) {
              cancel();
              fail(error);
            }
          });
          stream.result.then((value) => {
            if (!mounted || current !== ticket) return;
            stream = void 0;
            update({ status: "completed", result: value ?? null });
          }, (error) => {
            if (!mounted || current !== ticket) return;
            if (error.code === "BRIDGE_CANCELED") {
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
      update({ pending: true, stale: snapshot.value !== null, error: null });
      try {
        const value = await definition.action(...definition.params === void 0 ? [] : [definition.params]);
        if (mounted && current === ticket) update({ pending: false, stale: false, value, error: null });
      } catch (error) {
        if (mounted && current === ticket) update({ pending: false, stale: snapshot.value !== null, error: resourceError(error) });
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
        if (definition.onData) opened.on("data", (value) => {
          if (mounted && current === ticket) definition.onData(value);
        });
        opened.on("error", (error) => {
          if (mounted && current === ticket) update({ error: resourceError(error) });
        });
        opened.on("close", () => {
          if (mounted && current === ticket) {
            channel = void 0;
            update({ connecting: false, connected: false });
          }
        });
      } catch (error) {
        if (mounted && current === ticket) {
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
  var bindings = /* @__PURE__ */ new WeakMap();
  function cellOf(binding) {
    return bindings.get(binding);
  }
  function local(name, initial, resource = "") {
    if (!render.active || render.scope === null || typeof name !== "string" || !name || name.length > 256) throw Error("UI_SCOPE: invalid local state");
    const { candidate, scope } = render;
    const key = JSON.stringify(resource ? [scope, resource, name] : [scope, name]);
    if (!resource && candidate.declarations.has(key)) throw Error(`UI_LOCAL: duplicate declaration ${JSON.stringify(name)} in ${scope}`);
    candidate.declarations.add(key);
    const cell = candidate.locals.get(key) ?? render.lookupLocals?.get(key) ?? page.locals.get(key) ?? { value: immutable(initial, 0, true) };
    candidate.locals.set(key, cell);
    for (const collector of render.collectors) collector.set(key, cell);
    const binding = Object.freeze({
      get value() {
        return page.updates.has(cell) ? page.updates.get(cell) : cell.value;
      },
      set(value) {
        outsideRender("state mutation");
        if (page.locals.get(key) !== cell && render.candidate?.locals.get(key) !== cell)
          throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
        page.updates.set(cell, immutable(value, 0, true));
      }
    });
    bindings.set(binding, { key, cell });
    return binding;
  }
  function useLocal(name, initial) {
    return local(name, initial);
  }
  function hasLocalUpdates() {
    return page.updates.size > 0;
  }
  function useView() {
    if (!render.active) throw Error("UI_SCOPE: useView outside render");
    return render.view;
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
    const payload = params[0] === void 0 ? void 0 : immutable(params[0]);
    const resource = own(
      name,
      { pending: false, stale: false, value: null, error: null },
      "request",
      "UI_REQUEST",
      requestResource,
      { action: target?.action, params: payload, key: JSON.stringify(payload) }
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
  function useStream(name, action, options = {}) {
    if (!options || typeof options !== "object" || options.onData !== void 0 && typeof options.onData !== "function")
      throw Error("UI_STREAM: onData must be a function");
    const target = pageAction(action, "UI_STREAM", "stream");
    const resource = own(
      name,
      { status: "idle", error: null, result: null },
      "stream",
      "UI_STREAM",
      streamResource,
      { action: target?.action, onData: options.onData }
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
      start: guard("stream mutation", resource.start),
      cancel: guard("stream mutation", resource.cancel)
    });
  }
  function useChannel(name, action, options) {
    if (!options || typeof options !== "object" || !("params" in options) || options.onData !== void 0 && typeof options.onData !== "function")
      throw Error("UI_CHANNEL: expected { params, onData? }");
    const target = pageAction(action, "UI_CHANNEL", "call");
    const params = options.params === void 0 ? void 0 : immutable(options.params);
    const resource = own(
      name,
      { connecting: false, connected: false, error: null },
      "channel",
      "UI_CHANNEL",
      (state) => channelResource(state, report),
      { topic: target?.name, params, key: JSON.stringify(params), onData: options.onData, channels: render.view.channels }
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
  function useLeaveGuard(name, options) {
    if (typeof options?.enabled !== "boolean") throw Error("UI_LEAVE: expected { enabled: boolean }");
    const resource = own(name, { pending: null, requests: 0, error: null }, "leave", "UI_LEAVE", leaveGuards.resource, { enabled: options.enabled });
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
  function createView(data, env) {
    const { actions: actions2, channels: channels2 } = currentActions();
    return Object.freeze({ data: immutable(data), env: immutable(env), actions: actions2, channels: channels2 });
  }

  // src/runtime/validate.js
  function label(value, code, required = false) {
    if (value === void 0 && !required) return;
    if (typeof value !== "string" || !value.trim() || utf8Bytes(value, 1025) > 1024 || value.includes("\0"))
      throw Error(`${code}: ${required ? "accessibilityLabel is required" : "invalid accessibility label"}`);
  }
  function vector(props, fit) {
    const box = props.viewBox ?? [0, 0, 24, 24];
    if (!Array.isArray(box) || box.length !== 4 || box.some((v) => typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > 16384) || box[2] < 1 / 64 || box[3] < 1 / 64)
      throw Error("UI_VECTOR: invalid viewBox");
    if (!Array.isArray(props.paths) || !props.paths.length || props.paths.length > 32) throw Error("UI_VECTOR: invalid paths");
    for (const path of props.paths) {
      if (!path || typeof path !== "object" || Array.isArray(path) || Object.keys(path).some((k) => !["d", "fill", "stroke", "strokeWidth", "strokeDasharray", "vectorEffect"].includes(k)) || typeof path.d !== "string" || !path.d.length || path.d.length > 16384) throw Error("UI_VECTOR: invalid path");
      for (const key of ["fill", "stroke"]) if (path[key] !== void 0 && (typeof path[key] !== "string" || !["none", "currentColor"].includes(path[key]) && !/^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(path[key]))) throw Error("UI_VECTOR: invalid color");
      if (path.vectorEffect !== void 0 && !["none", "non-scaling-stroke"].includes(path.vectorEffect)) throw Error("UI_VECTOR: invalid vector effect");
      if (path.strokeDasharray !== void 0 && (!Array.isArray(path.strokeDasharray) || path.strokeDasharray.length > 16 || path.strokeDasharray.some((v) => typeof v !== "number" || !Number.isFinite(v) || v < 1 / 64 || v > 1024))) throw Error("UI_VECTOR: invalid dash pattern");
      if (path.strokeWidth !== void 0 && (typeof path.strokeWidth !== "number" || !Number.isFinite(path.strokeWidth) || path.strokeWidth < 0 || path.strokeWidth > 1024))
        throw Error("UI_VECTOR: invalid stroke width");
    }
    const source = JSON.stringify({ viewBox: box, paths: props.paths, ...fit === void 0 ? {} : { fit } });
    if (source.length > 32768) throw Error("UI_VECTOR: source too large");
    return source;
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
    if (!Array.isArray(props.colors) || !props.colors.length || props.colors.length > 16 || props.colors.some((c) => typeof c !== "string" || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(c)))
      throw Error("UI_PLOT: expected 1..16 hex colors");
  }
  function sliderValue({ min, max, step }, value) {
    value = Math.max(min, Math.min(max, value));
    const snapped = Math.max(min, Math.min(max, min + Math.floor((value - min) / step + 0.5) * step));
    return Math.abs(max - value) < Math.abs(snapped - value) ? max : snapped;
  }
  function slider(props) {
    for (const [name, fallback] of [["min", 0], ["max", 100], ["step", 1]]) if (props[name] === void 0) props[name] = fallback;
    if (props.value === void 0) props.value = props.min;
    const { min, max, step, value } = props;
    if (![min, max, step, value].every((v) => typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= 1e9) || max <= min || step <= 0 || step > max - min || (max - min) / step > 1e6 || min + step === min || max - step === max)
      throw Error("UI_SLIDER: invalid range or precision");
    label(props.accessibilityLabel, "UI_SLIDER", true);
    props.value = sliderValue(props, value);
  }
  function image(props) {
    if (typeof props.src !== "string" || !props.src.length || props.src.length > 8192 || props.src.includes("\0")) throw Error("UI_IMAGE: invalid source");
    if (props.alt !== void 0 && (typeof props.alt !== "string" || utf8Bytes(props.alt, 1024) > 1024 || props.alt.includes("\0") || props.alt.length > 0 && !props.alt.trim())) throw Error("UI_IMAGE: invalid alternative text");
    if (props.retryKey !== void 0 && (typeof props.retryKey !== "string" || utf8Bytes(props.retryKey, 257) > 256 || props.retryKey.includes("\0")))
      throw Error("UI_IMAGE: invalid retry key");
    for (const name of ["onLoad", "onError"]) if (props[name] !== void 0 && typeof props[name] !== "function") throw Error("UI_IMAGE: invalid callback");
  }
  function listKeys(props) {
    if (!Number.isSafeInteger(props.count) || props.count < 0 || props.count > 1e5 || typeof props.row !== "function" || props.height !== void 0 && typeof props.height !== "function") throw Error("UI_LIST: invalid row description");
    if (props.itemKeys !== void 0) {
      if (!Array.isArray(props.itemKeys) || props.itemKeys.length !== props.count) throw Error("UI_LIST: itemKeys must match count");
      const seen = /* @__PURE__ */ new Set();
      props.itemKeys = Array.from(props.itemKeys, (key) => {
        if (typeof key !== "string" || utf8Bytes(key, 256) > 256 || utf8Bytes(`key:${JSON.stringify(key)}`, 256) > 256 || seen.has(key))
          throw Error("UI_LIST: invalid or duplicate itemKey");
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
  }

  // src/runtime/elements.js
  var common = /* @__PURE__ */ new Set(["key", "testId", "style", "className", "children"]);
  var editors = /* @__PURE__ */ new Set(["input", "textarea", "search"]);
  var controls = /* @__PURE__ */ new Set(["button", "checkbox", "switch", "radio"]);
  var composedContent = /* @__PURE__ */ new Set(["view", "column", "row", "text", "spacer", "image", "grid", "stack", "vector"]);
  var bindCodes = { input: "UI_INPUT", textarea: "UI_INPUT", search: "UI_INPUT", checkbox: "UI_CHECK", switch: "UI_CHECK", slider: "UI_SLIDER", radio: "UI_RADIO" };
  var optionStyle = ["fontSize", "fontWeight", "fontFamily", "lineHeight", "color", "background"];
  var componentIds = /* @__PURE__ */ new WeakMap();
  var nextComponent = 1;
  var nextHandle = 1;
  var handle = () => nextHandle++;
  var callbackOf = (callback, kind = "void", isolated = false) => ({ callback, kind, isolated });
  function admit2(tag, element, props) {
    for (const key of Object.keys(props)) {
      if (common.has(key) || has(element.props, key)) continue;
      if (element.binding && (key === "bind" || key === "value" || key === element.binding.event)) continue;
      throw Error(`UI_PROP: ${key} is not a property of <${tag}>`);
    }
    for (const key of ["key", "testId"]) if (props[key] !== void 0 && typeof props[key] !== "string") throw Error(`UI_PROP: ${key}`);
    if (props.disabled !== void 0 && typeof props.disabled !== "boolean") throw Error("UI_PROP: disabled");
    if (props.style !== void 0 && (!props.style || typeof props.style !== "object" || Array.isArray(props.style))) throw Error("UI_PROP: style");
  }
  function bind(tag, element, props) {
    const { type, event } = element.binding;
    const code = bindCodes[tag];
    if (props.bind !== void 0) {
      const cell = cellOf(props.bind);
      if (!cell || render.candidate.locals.get(cell.key) !== cell.cell || typeof props.bind.value !== type || props.value !== void 0 || props[event] !== void 0)
        throw Error(`${code}: bind requires a mounted ${type} useLocal cell without value/${event}`);
      const binding = props.bind;
      props.value = binding.value;
      props[event] = (value) => binding.set(value);
    }
    if (props[event] !== void 0 && typeof props[event] !== "function") throw Error(`${code}: ${event}`);
  }
  function nodeStyle(tag, props, extra, inherited) {
    const env = render.view?.env;
    const classes = resolveClasses(props.className, env);
    const disabled = props.disabled === true;
    const author = mergeStyle(extra, disabled && classes?.disabled ? classes.disabled : classes?.base, props.style);
    const { fields, typography } = lowerStyle(tag, author, env, inherited);
    const states2 = {};
    const state = (name, wire, allowed, requirement) => {
      const delta = classes?.[name];
      if (!delta || !Object.keys(delta).length) return;
      if (!allowed) throw Error(`UI_STYLE: ${name}: requires ${requirement}, not <${tag}>`);
      paintOnly(name, tag, delta);
      states2[wire] = lowerStyle(tag, delta, env, { fontSize: typography.fontSize }).fields;
    };
    state("focus", "focusStyle", editors.has(tag), "an editor");
    if (!disabled) {
      state("active", "pressedStyle", controls.has(tag), "a control");
      state("hover", "hoverStyle", true);
    }
    return { fields, states: states2, typography, children: classes?.children };
  }
  function resolveNode(node, identity, depth = 0, parent = {}) {
    if (depth > 128) throw Error("UI_LIMIT: children too deep");
    if (node == null || typeof node === "boolean" || node === "") return null;
    if (typeof node === "string" || typeof node === "number") node = h("text", {}, node);
    if (!isNode(node)) throw Error("UI_ROOT: expected one node");
    const [tag, original, children, location] = readNode(node);
    if (typeof tag === "function") {
      if (!componentIds.has(tag)) componentIds.set(tag, nextComponent++);
      const previous = render.scope;
      render.scope = `${identity}/component:${componentIds.get(tag)}`;
      const supplied = children.length === 0 ? void 0 : children.length === 1 ? children[0] : children;
      try {
        return resolveNode(tag({ ...original, children: supplied }), render.scope, depth + 1, parent);
      } catch (error) {
        throw located(error, `${tag.name || "<anonymous>"}${location ? ` (${location})` : ""}`);
      } finally {
        render.scope = previous;
      }
    }
    try {
      return resolveElement(tag, original, children, identity, depth, parent);
    } catch (error) {
      throw located(error, `<${tag}>${location ? ` (${location})` : ""}`);
    }
  }
  function resolveElement(tag, original, children, identity, depth, parent) {
    if (typeof tag !== "string" || !has(elements, tag)) throw Error(`UI_TAG: ${String(tag)}`);
    const element = elements[tag];
    const props = { ...original };
    admit2(tag, element, props);
    if (parent.inControl && !composedContent.has(tag)) throw Error("UI_BUTTON: descendants must be noninteractive content");
    if (element.binding) bind(tag, element, props);
    const style2 = nodeStyle(tag, props, parent.extra, parent.typography ?? { fontSize: 14 });
    const flat = flatten(children);
    const composed = element.children === "content" && flat.some((child) => typeof child !== "string" && typeof child !== "number");
    const leaf = element.children === "text" || element.children === "content" && !composed;
    if (leaf && flat.some((child) => typeof child !== "string" && typeof child !== "number")) throw Error(`UI_CHILD: <${tag}> accepts only text`);
    if (element.children === "none" && flat.length) throw Error(`UI_CHILD: <${tag}> takes no children`);
    const disabled = props.disabled === true;
    const result = {
      tag: element.wire,
      element: tag,
      props,
      fields: style2.fields,
      states: style2.states,
      typography: style2.typography,
      identity,
      text: leaf ? flat.join("") : "",
      wire: [],
      handlers: [],
      children: []
    };
    const wire = (key, value) => result.wire.push([key, value]);
    const on = (key, callback, kind, isolated) => result.handlers.push([key, handle(), callbackOf(callback, kind, isolated)]);
    if (disabled) wire("enabled", 0);
    switch (tag) {
      case "button":
        label(props.accessibilityLabel, "UI_BUTTON");
        if (props.onClick !== void 0 && typeof props.onClick !== "function") throw Error("UI_PROP: onClick");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        if (props.onClick && !disabled) on("onClick", props.onClick);
        break;
      case "checkbox":
      case "switch":
        label(props.accessibilityLabel, "UI_CHECK");
        if (props.value !== void 0 && typeof props.value !== "boolean") throw Error("UI_CHECK: value must be boolean");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        wire("checked", +(props.value ?? false));
        if (props.onChange && !disabled) {
          const { onChange, value } = props;
          on("onCheckedChange", () => onChange(!(value ?? false)));
        }
        break;
      case "slider":
        slider(props);
        for (const [key, name] of [["sliderValue", "value"], ["sliderMin", "min"], ["sliderMax", "max"], ["sliderStep", "step"], ["accessibilityLabel", "accessibilityLabel"]])
          wire(key, props[name]);
        if (props.onChange && !disabled) on("onSliderChange", (value) => {
          const next = sliderValue(props, value);
          if (next !== props.value) props.onChange(next);
        }, "number");
        break;
      case "input":
      case "textarea":
      case "search":
        label(props.accessibilityLabel, "UI_INPUT");
        if (!validInputText(props.value === void 0 ? "" : props.value) || !validInputText(props.placeholder === void 0 ? "" : props.placeholder)) throw Error("UI_INPUT: invalid text");
        for (const name of ["secure", "autoFocus", "submitOnEnter"]) if (props[name] !== void 0 && typeof props[name] !== "boolean") throw Error(`UI_INPUT: ${name}`);
        if (props.keyboard !== void 0 && !["text", "number", "email", "phone"].includes(props.keyboard)) throw Error("UI_INPUT: keyboard");
        if (props.onSubmit !== void 0 && typeof props.onSubmit !== "function") throw Error("UI_INPUT: onSubmit");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        wire("inputValue", props.value ?? "");
        if (props.placeholder !== void 0) wire("placeholder", props.placeholder);
        if (tag === "search") wire("keyboard", "search");
        else if (props.keyboard !== void 0) wire("keyboard", props.keyboard);
        for (const name of ["secure", "autoFocus", "submitOnEnter"]) if (props[name] !== void 0) wire(name, +props[name]);
        if (!disabled) {
          for (const name of ["onInput", "onSubmit"]) if (props[name]) on(name, props[name], "text");
        }
        break;
      case "image":
        image(props);
        wire("imageSrc", props.src);
        if (props.alt !== void 0) wire("imageAlt", props.alt);
        if (props.retryKey !== void 0) wire("imageRetryKey", props.retryKey);
        if (props.onLoad) on("onImageLoad", props.onLoad, "void", true);
        if (props.onError) on("onImageError", props.onError, "void", true);
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
        wire("plotColors", JSON.stringify(props.colors));
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
        result.children = radioOptions(props, identity, result.fields, disabled);
        break;
      case "grid":
        if (result.fields.gridColumns === void 0) throw Error("UI_GRID: <grid> requires style gridTemplateColumns");
        break;
    }
    if (tag === "scroll" || tag === "list") for (const name of ["onReachStart", "onReachEnd"]) {
      if (props[name] === void 0) continue;
      if (typeof props[name] !== "function") throw Error(`UI_PROP: ${name}`);
      on(name, props[name]);
    }
    if (tag === "list") {
      listKeys(props);
      result.list = listState(props, identity);
    } else if (!leaf && tag !== "radio") {
      const keys = /* @__PURE__ */ new Set(), last = flat.length - 1;
      result.children = flat.map((child, position) => {
        const key = nodeKey(child);
        if (key !== void 0 && typeof key !== "string") throw Error("UI_KEY: expected string");
        if (key !== void 0 && keys.has(key)) throw Error("UI_KEY: duplicate sibling");
        if (key !== void 0) keys.add(key);
        const part = key === void 0 ? `position:${position}` : `key:${JSON.stringify(key)}`;
        return resolveNode(child, `${identity}/${tag}/${part}`, depth + 1, {
          tag,
          typography: style2.typography,
          extra: position < last ? style2.children : void 0,
          inControl: parent.inControl || tag === "button" || tag === "checkbox"
        });
      }).filter((child) => child !== null);
    }
    return result;
  }
  function radioOptions(props, identity, fields, disabled) {
    if (!Array.isArray(props.options) || !props.options.length || props.options.length > 64) throw Error("UI_RADIO: expected 1..64 options");
    const values = /* @__PURE__ */ new Set();
    const style2 = Object.fromEntries(Object.entries(fields).filter(([name]) => optionStyle.includes(name)));
    return props.options.map((option2) => {
      if (!option2 || typeof option2 !== "object" || Object.keys(option2).some((k) => !["value", "label", "disabled", "testId"].includes(k)) || typeof option2.value !== "string" || !option2.value.length || option2.value.length > 256 || values.has(option2.value) || typeof option2.label !== "string" || option2.disabled !== void 0 && typeof option2.disabled !== "boolean" || option2.testId !== void 0 && typeof option2.testId !== "string") throw Error("UI_RADIO: invalid or duplicate option");
      values.add(option2.value);
      const { value } = option2, selected = props.value === value, enabled = !disabled && option2.disabled !== true;
      const child = {
        tag: "radio-option",
        element: "radio-option",
        identity: `${identity}/radio/key:${JSON.stringify(value)}`,
        props: { key: value, testId: option2.testId },
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
  function listState(props, identity) {
    const old = page.committed?.lists.get(identity);
    const anchor = props.anchor === void 0 ? null : JSON.stringify(props.anchor);
    const positions = props.itemKeys && new Map(props.itemKeys.map((key, i) => [`key:${JSON.stringify(key)}`, i]));
    const states2 = positions ? new Map([...old?.states ?? []].flatMap(([, state]) => positions.has(state.key) ? [[positions.get(state.key), state]] : [])) : new Map([...old?.states ?? []].filter(([i]) => i < props.count));
    return {
      count: props.count,
      previousCount: old?.count ?? props.count,
      anchor,
      anchorChanged: Boolean(old && old.count === props.count && anchor !== null && anchor !== old.anchor),
      anchorCache: old?.cache ?? null,
      states: states2,
      cache: /* @__PURE__ */ new Map(),
      window: old?.window ? { ...old.window, start: Math.min(old.window.start, props.count), end: Math.min(old.window.end, props.count) } : null
    };
  }

  // src/runtime/encode.js
  var structured = /* @__PURE__ */ new Set(["backgroundGradient", "boxShadow", "animation"]);
  var encodeValue = (key, value) => structured.has(key) && value !== "none" ? JSON.stringify(value) : value;
  function copyTree(node) {
    let children = node.children;
    for (let i = 0; i < children.length; i++) {
      const child = copyTree(node.children[i]);
      if (child === node.children[i]) continue;
      if (children === node.children) children = children.slice();
      children[i] = child;
    }
    if (node.list) return { ...node, children, list: { ...node.list, states: new Map(node.list.states), cache: new Map(node.list.cache) } };
    return children === node.children ? node : { ...node, children };
  }
  function retainCells(cells) {
    for (const [key, cell] of cells) {
      render.candidate.locals.set(key, cell);
      for (const collector of render.collectors) collector.set(key, cell);
    }
  }
  var initialWindow = (count) => ({ start: 0, end: Math.min(1, count), leading: 0, trailing: Math.max(0, count - 1) * 64 });
  function encodeTree(tree, windows) {
    const { candidate } = render;
    const output = [];
    let count = 0, plotWork = 0, animationBytes = 0, itemKeyBytes = 0;
    const write = (node, parent, depth) => {
      if (depth > 128 || ++count > 4096) throw Error("UI_LIMIT: candidate too large");
      const { tag, props, identity } = node, encoded = [];
      for (const [key, value] of Object.entries(node.fields)) {
        if (key === "animation") {
          animationBytes += utf8Bytes(JSON.stringify(value), 32768);
          if (animationBytes > 262144) throw Error("UI_ANIMATION: page payload budget exceeded");
        }
        encoded.push(field[key], encodeValue(key, value));
      }
      for (const [key, style2] of Object.entries(node.states)) {
        const pairs = Object.entries(style2).flatMap(([name, value]) => [field[name], encodeValue(name, value)]);
        if (pairs.length) encoded.push(field[key], JSON.stringify(pairs));
      }
      for (const [key, value] of node.wire) encoded.push(field[key], value);
      for (const [key, id, entry] of node.handlers) {
        candidate.callbacks.set(id, entry);
        encoded.push(field[key], id);
      }
      if (node.plotWork) {
        plotWork += node.plotWork;
        if (plotWork > 16384) throw Error("UI_LIMIT: plot geometry budget exceeded");
      }
      if (node.rowIndex !== void 0) encoded.push(field.listRowIndex, node.rowIndex);
      let window, seededAnchor = false;
      if (node.list) {
        window = windows[identity] ?? node.list.window ?? initialWindow(props.count);
        if (!Number.isSafeInteger(window.start) || !Number.isSafeInteger(window.end) || window.start < 0 || window.end < window.start || window.end > Math.max(props.count, node.list.previousCount) || ![window.leading, window.trailing].every((v) => Number.isFinite(v) && v >= 0 && v <= 16384e5)) throw Error("UI_LIST: invalid window");
        if (window.end > props.count) window = initialWindow(props.count);
        const target = props.anchor === "end" ? props.count - 1 : Math.min(props.anchor?.index ?? 0, props.count - 1);
        if (node.list.anchorChanged && props.count > 0 && (target < window.start || target >= window.end)) {
          node.list.cache = new Map([...node.list.anchorCache].filter(([i, row]) => props.itemKeys === void 0 || row.props.key === `key:${JSON.stringify(props.itemKeys[i])}`));
          seededAnchor = true;
        }
        node.list.anchorChanged = false;
        node.list.anchorCache = null;
        node.list.previousCount = props.count;
        node.list.window = { ...window };
        encoded.push(field.listId, identity, field.listCount, props.count, field.listLeading, window.leading, field.listTrailing, window.trailing);
        if (props.itemKeys !== void 0) {
          const payload = JSON.stringify(props.itemKeys);
          itemKeyBytes += utf8Bytes(payload, 262144);
          if (itemKeyBytes > 262144) throw Error("UI_LIST: itemKeys page budget exceeded");
          encoded.push(field.listItemKeys, payload);
        }
        if (props.anchor !== void 0) encoded.push(field.listAnchor, JSON.stringify(props.anchor));
        candidate.lists.set(identity, node.list);
      }
      const index = output.length;
      output.push(tags[tag], parent, props.key ?? "", props.testId ?? "", node.text, encoded.length / 2, ...encoded);
      if (!node.list) {
        for (const child of node.children) write(child, index, depth + 1);
        return;
      }
      writeRows(node, window, index, depth, write);
      if (seededAnchor) node.list.cache.clear();
    };
    write(tree, -1, 0);
    return output;
  }
  function writeRows(node, window, index, depth, write) {
    const visibleKeys = /* @__PURE__ */ new Set();
    for (const i of node.list.cache.keys()) if (i < window.start || i >= window.end) node.list.cache.delete(i);
    for (let i = window.start; i < window.end; i++) {
      const cells = /* @__PURE__ */ new Map();
      render.collectors.push(cells);
      try {
        let row = node.list.cache.get(i), key = node.list.states.get(i)?.key;
        if (!row) ({ row, key } = renderRow(node, i, cells, depth));
        else {
          row = copyTree(row);
          retainCells(node.list.states.get(i).locals);
        }
        if (visibleKeys.has(key)) throw Error("UI_KEY: duplicate list row");
        visibleKeys.add(key);
        write(row, index, depth + 1);
        node.list.cache.set(i, row);
        node.list.states.set(i, { key, locals: cells });
      } finally {
        render.collectors.pop();
      }
    }
    for (const state of node.list.states.values()) retainCells(state.locals);
  }
  function renderRow(node, i, cells, depth) {
    const { props, identity } = node;
    const suppliedKey = props.itemKeys?.[i];
    const previous = render.scope;
    render.scope = suppliedKey === void 0 ? `${identity}/row:${i}` : `${identity}/row/key:${JSON.stringify(suppliedKey)}`;
    try {
      const value = props.row(i);
      const declared = isNode(value) ? readNode(value)[1].key : void 0;
      if (declared !== void 0 && typeof declared !== "string") throw Error("UI_KEY: expected string");
      if (suppliedKey === void 0 && declared !== void 0 && cells.size > 0)
        throw Error("UI_LIST: keyed row factory hooks require itemKeys; alternatively move hooks into a keyed component");
      if (suppliedKey !== void 0 && declared !== void 0 && suppliedKey !== declared) throw Error("UI_LIST: row key does not match itemKeys");
      const rowKey = suppliedKey ?? declared;
      const key = rowKey === void 0 ? `index:${i}` : `key:${JSON.stringify(rowKey)}`;
      render.scope = `${identity}/row/${key}`;
      const fields = {};
      if (props.height) {
        const height = props.height(i);
        if (!Number.isFinite(height) || height < 0 || height > 16384) throw Error("UI_LIST: invalid row height");
        fields.height = height;
      }
      const content = resolveNode(value, render.scope, depth + 2, { typography: node.typography });
      const row = {
        tag: "column",
        element: "column",
        props: { key },
        fields,
        states: {},
        wire: [],
        handlers: [],
        identity: render.scope,
        rowIndex: i,
        text: "",
        children: content === null ? [] : [content]
      };
      return { row, key };
    } finally {
      render.scope = previous;
    }
  }

  // src/runtime/tree.js
  function List({ items, getKey, renderItem, getItemHeight, ...props }) {
    return h("list", {
      ...props,
      count: items.length,
      itemKeys: items.map(getKey),
      row: (index) => renderItem(items[index], index),
      ...getItemHeight ? { height: (index) => getItemHeight(items[index], index) } : {}
    });
  }
  function runCandidate(view, lookup, build) {
    if (render.active) throw Error("UI_RENDER: recursive render");
    resetArena();
    Object.assign(render, { active: true, view, lookupLocals: lookup });
    render.candidate = { locals: /* @__PURE__ */ new Map(), declarations: /* @__PURE__ */ new Set(), callbacks: /* @__PURE__ */ new Map(), lists: /* @__PURE__ */ new Map(), resources: /* @__PURE__ */ new Map(), resourceNames: /* @__PURE__ */ new Set(), view };
    try {
      render.scope = "root";
      return build(render.candidate);
    } catch (error) {
      discard();
      throw error;
    } finally {
      clearArena();
      Object.assign(render, { active: false, scope: null, view: null, lookupLocals: null, collectors: [] });
    }
  }
  function prepare(renderPage, view, windows = {}) {
    return runCandidate(view, null, (candidate) => {
      candidate.tree = resolveNode(renderPage(view), "root");
      if (!candidate.tree) throw Error("UI_ROOT: page must return one layout node");
      candidate.baseLocals = new Map(candidate.locals);
      return encodeTree(candidate.tree, windows);
    });
  }
  function prepareWindows(windows) {
    const base = render.candidate ?? page.committed;
    if (!base) throw Error("UI_LIST: no retained description");
    return runCandidate(base.view, base.locals, (candidate) => {
      candidate.resources = new Map(base.resources);
      candidate.tree = copyTree(base.tree);
      candidate.baseLocals = new Map(base.baseLocals);
      retainCells(candidate.baseLocals);
      return encodeTree(candidate.tree, windows);
    });
  }
  function commit() {
    const { candidate } = render;
    if (!candidate) throw Error("UI_COMMIT: no candidate");
    const previous = page.committed;
    for (const cell of candidate.locals.values()) if (page.updates.has(cell)) cell.value = page.updates.get(cell);
    page.locals = candidate.locals;
    page.callbacks = candidate.callbacks;
    page.committed = candidate;
    render.candidate = null;
    page.updates.clear();
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
  }
  function discard() {
    render.candidate = null;
    page.updates.clear();
  }
  function reset() {
    outsideRender("reset");
    discard();
    page.committed = null;
    page.locals.clear();
    page.callbacks.clear();
    for (const resource of page.resources.values()) resource.dispose();
    page.resources.clear();
    resetActions();
  }
  var accepts = (kind, value) => kind === "number" ? typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 1e9 : kind === "text" ? validInputText(value) : value === void 0;
  function dispatch(handle2, value) {
    outsideRender("dispatch");
    const entry = page.callbacks.get(handle2);
    if (!entry || !accepts(entry.kind, value)) return false;
    const previous = entry.isolated ? new Map(page.updates) : void 0;
    try {
      const epoch2 = actionEpochNow();
      const result = entry.kind === "void" ? entry.callback() : entry.callback(value);
      if (result && typeof result.then === "function") Promise.resolve(result).catch((error) => {
        if (hasActionEpoch(epoch2)) report(error);
      });
      return true;
    } catch (error) {
      page.updates.clear();
      if (!entry.isolated) throw error;
      for (const [cell, kept] of previous) page.updates.set(cell, kept);
      report(error);
      return true;
    }
  }
  function describe(renderPage, view) {
    const output = prepare(renderPage, view);
    commit();
    return output;
  }

  // src/host.js
  function publishHost(render2, catalog) {
    Object.assign(globalThis, {
      __lingxiaRender: render2,
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
