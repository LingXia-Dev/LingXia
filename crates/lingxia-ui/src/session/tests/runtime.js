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
    configureSourceMap: () => configureSourceMap,
    createView: () => createView,
    describe: () => describe,
    discard: () => discard,
    dispatch: () => dispatch,
    enableHotSwap: () => enableHotSwap,
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
  var render = { active: false, candidate: null, scope: null, view: null, lookupLocals: null, collectors: [], site: null };
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
  var elements = { "view": { "wire": "column", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "accessibilityLabel": false } }, "column": { "wire": "column", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "accessibilityLabel": false } }, "row": { "wire": "row", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "accessibilityLabel": false } }, "page": { "wire": "page", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "flexDirection", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems"], "props": { "accessibilityLabel": false } }, "text": { "wire": "text", "children": "runs", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "lineClamp", "textOverflow"], "props": { "accessibilityLabel": false } }, "button": { "wire": "button", "children": "content", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "onClick": false, "disabled": false, "accessibilityLabel": false } }, "spacer": { "wire": "spacer", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display"], "props": {} }, "scroll": { "wire": "scroll", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "overflowX", "overflowY", "overscrollBehavior"], "props": { "onReachStart": false, "onReachEnd": false } }, "list": { "wire": "list", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "overscrollBehavior"], "props": { "count": true, "row": true, "height": false, "anchor": false, "itemKeys": false, "onReachStart": false, "onReachEnd": false } }, "image": { "wire": "image", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "objectFit", "objectPosition"], "props": { "src": true, "placeholder": false, "alt": false, "retryKey": false, "onLoad": false, "onError": false } }, "input": { "wire": "input", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "placeholderColor"], "props": { "accessibilityLabel": false, "placeholder": false, "secure": false, "keyboard": false, "autoFocus": false, "disabled": false, "onSubmit": false, "maxLength": false, "autoComplete": false, "enterKeyHint": false, "autoCapitalize": false, "spellCheck": false, "min": false, "max": false, "step": false }, "binding": { "type": "string", "event": "onInput" } }, "textarea": { "wire": "textarea", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "placeholderColor"], "props": { "rows": false, "maxRows": false, "accessibilityLabel": false, "placeholder": false, "secure": false, "keyboard": false, "autoFocus": false, "disabled": false, "submitOnEnter": false, "onSubmit": false, "maxLength": false, "autoComplete": false, "enterKeyHint": false, "autoCapitalize": false, "spellCheck": false }, "binding": { "type": "string", "event": "onInput" } }, "search": { "wire": "search", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap", "placeholderColor"], "props": { "accessibilityLabel": false, "placeholder": false, "autoFocus": false, "disabled": false, "onSubmit": false, "maxLength": false, "autoComplete": false, "enterKeyHint": false, "autoCapitalize": false, "spellCheck": false }, "binding": { "type": "string", "event": "onInput" } }, "checkbox": { "wire": "checkbox", "children": "content", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "flexWrap", "alignItems", "justifyContent", "alignContent", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "accessibilityLabel": false, "disabled": false }, "binding": { "type": "boolean", "event": "onChange" } }, "switch": { "wire": "switch", "children": "text", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "accessibilityLabel": false, "disabled": false }, "binding": { "type": "boolean", "event": "onChange" } }, "radio": { "wire": "radio", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "options": true, "accessibilityLabel": false, "disabled": false }, "binding": { "type": "string", "event": "onChange" } }, "slider": { "wire": "slider", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color"], "props": { "min": false, "max": false, "step": false, "accessibilityLabel": true, "disabled": false }, "binding": { "type": "number", "event": "onChange" } }, "grid": { "wire": "grid", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "gridTemplateColumns", "gridTemplateRows", "gridAutoColumns", "gridAutoRows", "gridAutoFlow", "justifyItems", "alignItems", "justifyContent", "alignContent", "gap", "rowGap", "columnGap", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": {} }, "stack": { "wire": "stack", "children": "any", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": {} }, "plot": { "wire": "plot", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display"], "props": { "kind": true, "values": true, "colors": true, "accessibilityLabel": true, "smooth": false, "area": false, "selectedIndex": false, "domainMin": false, "domainMax": false, "onSelect": false, "pieLayout": false, "markerEmphasisScale": false } }, "vector": { "wire": "vector", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "objectFit"], "props": { "paths": true, "gradients": false, "viewBox": false } }, "span": { "wire": "span", "children": "runs", "style": ["color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "textDecorationLine", "fontVariantNumeric", "backgroundColor"], "props": {} }, "link": { "wire": "link", "children": "runs", "style": ["color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "textDecorationLine", "fontVariantNumeric", "backgroundColor"], "props": { "onPress": false, "accessibilityLabel": false, "disabled": false } }, "select": { "wire": "select", "children": "none", "style": ["width", "height", "minWidth", "minHeight", "maxWidth", "aspectRatio", "maxHeight", "margin", "marginTop", "marginRight", "marginBottom", "marginLeft", "marginInlineStart", "marginInlineEnd", "position", "inset", "top", "right", "bottom", "left", "insetInlineStart", "insetInlineEnd", "zIndex", "flexGrow", "flexShrink", "flexBasis", "alignSelf", "gridColumn", "gridColumnStart", "gridColumnEnd", "gridRow", "gridRowStart", "gridRowEnd", "justifySelf", "order", "opacity", "overflow", "backgroundColor", "backgroundImage", "borderRadius", "borderTopLeftRadius", "borderTopRightRadius", "borderBottomRightRadius", "borderBottomLeftRadius", "borderStartStartRadius", "borderStartEndRadius", "borderEndStartRadius", "borderEndEndRadius", "borderWidth", "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth", "borderInlineStartWidth", "borderInlineEndWidth", "borderStyle", "borderColor", "boxShadow", "backdropFilter", "translate", "rotate", "scale", "transformOrigin", "animation", "transition", "display", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "paddingInlineStart", "paddingInlineEnd", "color", "fontSize", "fontWeight", "fontFamily", "fontStyle", "lineHeight", "letterSpacing", "fontVariantNumeric", "userSelect", "textAlign", "textDecorationLine", "textTransform", "whiteSpace", "wordBreak", "overflowWrap"], "props": { "options": true, "placeholder": false, "accessibilityLabel": false, "disabled": false }, "binding": { "type": "string", "event": "onChange" } } };
  var style = { "width": { "lower": "dimension", "wire": "width" }, "height": { "lower": "dimension", "wire": "height" }, "minWidth": { "lower": "dimension", "wire": "minWidth" }, "minHeight": { "lower": "dimension", "wire": "minHeight" }, "maxWidth": { "lower": "dimension", "wire": "maxWidth" }, "aspectRatio": { "lower": "aspect-ratio", "wire": "aspectRatio" }, "maxHeight": { "lower": "dimension", "wire": "maxHeight" }, "margin": { "lower": "margin", "wire": ["marginTop", "marginRight", "marginBottom", "marginLeft"] }, "marginTop": { "lower": "margin", "wire": "marginTop" }, "marginRight": { "lower": "margin", "wire": "marginRight" }, "marginBottom": { "lower": "margin", "wire": "marginBottom" }, "marginLeft": { "lower": "margin", "wire": "marginLeft" }, "marginInlineStart": { "lower": "margin", "wire": "marginInlineStart" }, "marginInlineEnd": { "lower": "margin", "wire": "marginInlineEnd" }, "position": { "lower": "option", "wire": "position" }, "inset": { "lower": "inset", "wire": ["insetTop", "insetRight", "insetBottom", "insetLeft"] }, "top": { "lower": "inset", "wire": "insetTop" }, "right": { "lower": "inset", "wire": "insetRight" }, "bottom": { "lower": "inset", "wire": "insetBottom" }, "left": { "lower": "inset", "wire": "insetLeft" }, "insetInlineStart": { "lower": "inset", "wire": "insetInlineStart" }, "insetInlineEnd": { "lower": "inset", "wire": "insetInlineEnd" }, "zIndex": { "lower": "z-index", "wire": "zIndex" }, "flexGrow": { "lower": "number", "wire": "grow" }, "flexShrink": { "lower": "number", "wire": "shrink" }, "flexBasis": { "lower": "basis", "wire": "flexBasis" }, "alignSelf": { "lower": "alignment", "wire": "alignSelf" }, "gridColumn": { "lower": "grid-line", "wire": "gridColumn" }, "gridColumnStart": { "lower": "grid-line", "wire": "gridColumn" }, "gridColumnEnd": { "lower": "grid-line", "wire": "gridColumn" }, "gridRow": { "lower": "grid-line", "wire": "gridRow" }, "gridRowStart": { "lower": "grid-line", "wire": "gridRow" }, "gridRowEnd": { "lower": "grid-line", "wire": "gridRow" }, "justifySelf": { "lower": "alignment", "wire": "justifySelf" }, "order": { "lower": "order", "wire": "order" }, "opacity": { "lower": "opacity", "wire": "opacity", "paint": true }, "overflow": { "lower": "option", "wire": "overflow" }, "backgroundColor": { "lower": "color", "wire": "background", "paint": true }, "backgroundImage": { "lower": "gradient", "wire": "backgroundGradient", "paint": true }, "borderRadius": { "lower": "length", "wire": ["radius", "radiusTopLeft", "radiusTopRight", "radiusBottomRight", "radiusBottomLeft"] }, "borderTopLeftRadius": { "lower": "length", "wire": "radiusTopLeft" }, "borderTopRightRadius": { "lower": "length", "wire": "radiusTopRight" }, "borderBottomRightRadius": { "lower": "length", "wire": "radiusBottomRight" }, "borderBottomLeftRadius": { "lower": "length", "wire": "radiusBottomLeft" }, "borderStartStartRadius": { "lower": "length", "wire": "radiusStartStart" }, "borderStartEndRadius": { "lower": "length", "wire": "radiusStartEnd" }, "borderEndStartRadius": { "lower": "length", "wire": "radiusEndStart" }, "borderEndEndRadius": { "lower": "length", "wire": "radiusEndEnd" }, "borderWidth": { "lower": "length", "wire": ["borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth"] }, "borderTopWidth": { "lower": "length", "wire": "borderTopWidth" }, "borderRightWidth": { "lower": "length", "wire": "borderRightWidth" }, "borderBottomWidth": { "lower": "length", "wire": "borderBottomWidth" }, "borderLeftWidth": { "lower": "length", "wire": "borderLeftWidth" }, "borderInlineStartWidth": { "lower": "length", "wire": "borderInlineStartWidth" }, "borderInlineEndWidth": { "lower": "length", "wire": "borderInlineEndWidth" }, "borderStyle": { "lower": "option", "wire": "borderStyle" }, "borderColor": { "lower": "color", "wire": "borderColor", "paint": true }, "boxShadow": { "lower": "shadow", "wire": "boxShadow", "paint": true }, "backdropFilter": { "lower": "backdrop", "wire": ["backdropBlur", "backdropSaturation"] }, "translate": { "lower": "transform", "wire": "transform", "paint": true }, "rotate": { "lower": "transform", "wire": "transform", "paint": true }, "scale": { "lower": "transform", "wire": "transform", "paint": true }, "transformOrigin": { "lower": "transform", "wire": "transform", "paint": true }, "animation": { "lower": "animation", "wire": "animation", "internal": true }, "transition": { "lower": "transition", "wire": "transition", "internal": true }, "padding": { "lower": "length", "wire": ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"] }, "paddingTop": { "lower": "length", "wire": "paddingTop" }, "paddingRight": { "lower": "length", "wire": "paddingRight" }, "paddingBottom": { "lower": "length", "wire": "paddingBottom" }, "paddingLeft": { "lower": "length", "wire": "paddingLeft" }, "paddingInlineStart": { "lower": "length", "wire": "paddingInlineStart" }, "paddingInlineEnd": { "lower": "length", "wire": "paddingInlineEnd" }, "display": { "lower": "option", "wire": "display" }, "flexDirection": { "lower": "option", "wire": "flexDirection" }, "alignItems": { "lower": "alignment", "wire": "align" }, "justifyContent": { "lower": "justification", "wire": "justify" }, "alignContent": { "lower": "align-content", "wire": "alignContent" }, "flexWrap": { "lower": "option", "wire": "flexWrap" }, "gap": { "lower": "length", "wire": "gap" }, "rowGap": { "lower": "length", "wire": "rowGap" }, "columnGap": { "lower": "length", "wire": "columnGap" }, "gridTemplateColumns": { "lower": "grid-template", "wire": "gridTemplateColumns" }, "gridTemplateRows": { "lower": "grid-template", "wire": "gridTemplateRows" }, "gridAutoColumns": { "lower": "grid-auto", "wire": "gridAutoColumns" }, "gridAutoRows": { "lower": "grid-auto", "wire": "gridAutoRows" }, "gridAutoFlow": { "lower": "grid-flow", "wire": "gridAutoFlow" }, "justifyItems": { "lower": "alignment", "wire": "justifyItems" }, "color": { "lower": "color", "wire": "color", "paint": true }, "fontSize": { "lower": "font-size", "wire": "fontSize" }, "fontWeight": { "lower": "font-weight", "wire": "fontWeight" }, "fontFamily": { "lower": "family", "wire": "fontFamily" }, "fontStyle": { "lower": "option", "wire": "fontStyle" }, "lineHeight": { "lower": "line-height", "wire": "lineHeight" }, "letterSpacing": { "lower": "letter-spacing", "wire": "letterSpacing" }, "fontVariantNumeric": { "lower": "font-variant-numeric", "wire": "fontVariantNumeric" }, "userSelect": { "lower": "option", "wire": "userSelect" }, "textAlign": { "lower": "option", "wire": "textAlign" }, "textDecorationLine": { "lower": "option", "wire": "textDecorationLine" }, "textTransform": { "lower": "option", "wire": "textTransform" }, "whiteSpace": { "lower": "option", "wire": "whiteSpace" }, "wordBreak": { "lower": "option", "wire": "wordBreak" }, "overflowWrap": { "lower": "option", "wire": "overflowWrap" }, "lineClamp": { "lower": "line-clamp", "wire": "textLines" }, "textOverflow": { "lower": "option", "wire": "textOverflow" }, "overflowX": { "lower": "scroll-axis", "wire": "scrollAxis" }, "overflowY": { "lower": "scroll-axis", "wire": "scrollAxis" }, "overscrollBehavior": { "lower": "option", "wire": "overscroll" }, "placeholderColor": { "lower": "color", "wire": "placeholderColor", "paint": true }, "objectFit": { "lower": "fit", "wire": "imageFit" }, "objectPosition": { "lower": "object-position", "wire": "imagePosition" } };
  var limits = { "nodes": 4096, "depth": 128, "animationBytes": 262144, "itemKeyBytes": 262144, "imageBytes": 8388608, "sourceBytes": 8192, "visibleImageBytes": 67108864, "decodedImageBytes": 100663296, "runs": 256, "transitionMs": 1e4, "evalBudgetMs": 100, "evalBudgetDebugMultiplier": 8, "editorTextBytes": 65536, "inputRequestBytes": 524288, "inputBatchEdits": 64, "clipboardItems": 128, "wireValues": 1e5, "textBytes": 65536, "identifierBytes": 256, "listRows": 1e5, "gridTracks": 64, "vectorSourceBytes": 32768, "vectorPathBytes": 16384, "vectorPathWork": 16384, "vectorWork": 32768, "plotWork": 16384, "shadowMaskBytes": 16777216 };
  var admission = { "tags": { "column": { "children": "any", "content": true }, "row": { "children": "any", "content": true }, "text": { "children": "runs", "content": true }, "button": { "children": "content" }, "spacer": { "children": "none", "content": true }, "scroll": { "children": "any" }, "list": { "children": "rows", "requires": ["listId", "listCount", "listLeading", "listTrailing"] }, "image": { "children": "none", "content": true, "requires": ["imageSrc"] }, "input": { "children": "none" }, "textarea": { "children": "none" }, "checkbox": { "children": "content" }, "switch": { "children": "text" }, "radio": { "children": "options" }, "radio-option": { "children": "text", "parent": "radio", "keyed": true, "requires": ["checked"] }, "search": { "children": "none" }, "slider": { "children": "none" }, "grid": { "children": "any", "content": true }, "stack": { "children": "any", "content": true }, "plot": { "children": "none" }, "vector": { "children": "none", "content": true }, "page": { "children": "any", "root": true }, "span": { "children": "text", "content": true, "parent": "text" }, "link": { "children": "text", "parent": "text" }, "select": { "children": "text" } }, "fields": { "onClick": ["button", "link"], "enabled": ["button", "input", "textarea", "checkbox", "switch", "radio", "radio-option", "search", "slider", "select"], "listId": ["list"], "listCount": ["list"], "listLeading": ["list"], "listTrailing": ["list"], "listRowIndex": ["column"], "listAnchor": ["list"], "onReachStart": ["scroll", "list"], "onReachEnd": ["scroll", "list"], "imageSrc": ["image"], "imageFit": ["image"], "inputValue": ["input", "textarea", "search", "select"], "placeholder": ["input", "textarea", "search", "select"], "secure": ["input", "textarea"], "keyboard": ["input", "textarea", "search"], "autoFocus": ["input", "textarea", "search"], "onInput": ["input", "textarea", "search"], "onSubmit": ["input", "textarea", "search"], "checked": ["checkbox", "switch", "radio-option"], "onCheckedChange": ["checkbox", "switch", "radio-option"], "accessibilityLabel": ["button", "input", "textarea", "checkbox", "switch", "radio", "search", "slider", "plot", "link", "column", "row", "page", "text", "select"], "sliderValue": ["slider"], "sliderMin": ["slider"], "sliderMax": ["slider"], "sliderStep": ["slider"], "onSliderChange": ["slider"], "gridColumns": ["grid"], "textLines": ["text"], "plotValues": ["plot"], "plotKind": ["plot"], "plotColors": ["plot"], "plotSmooth": ["plot"], "plotArea": ["plot"], "onPlotSelect": ["plot"], "plotSelectedIndex": ["plot"], "vectorSource": ["vector"], "submitOnEnter": ["textarea"], "plotDomainMin": ["plot"], "plotDomainMax": ["plot"], "plotPieLayout": ["plot"], "plotMarkerEmphasisScale": ["plot"], "focusStyle": ["input", "textarea", "search"], "scrollAxis": ["scroll"], "listItemKeys": ["list"], "flexDirection": ["column", "row", "page"], "imageAlt": ["image"], "imageRetryKey": ["image"], "onImageLoad": ["image"], "onImageError": ["image"], "pressedStyle": ["button", "checkbox", "switch", "radio", "link"], "overscroll": ["scroll", "list"], "maxLength": ["input", "textarea", "search"], "autoComplete": ["input", "textarea", "search"], "enterKeyHint": ["input", "textarea", "search"], "autoCapitalize": ["input", "textarea", "search"], "spellCheck": ["input", "textarea", "search"], "inputMin": ["input"], "inputMax": ["input"], "inputStep": ["input"], "focusRequest": ["input", "textarea", "search"], "selectOptions": ["select"], "onSelect": ["select"], "rows": ["textarea"], "maxRows": ["textarea"], "imagePlaceholder": ["image"], "imagePosition": ["image"] } };
  var options = { "font-family": ["sans", "mono"], "font-style": ["normal", "italic"], "flex-wrap": ["nowrap", "wrap", "wrap-reverse"], "flex-direction": ["row", "column", "row-reverse", "column-reverse"], "display": ["flex", "grid", "none", "block", "inline-flex", "inline-block"], "scroll-axis": ["horizontal", "vertical"], "word-break": ["normal", "break-all", "keep-all"], "overflow-wrap": ["normal", "break-word", "anywhere"], "text-decoration-line": ["none", "line-through", "underline"], "text-alignment": ["start", "end", "left", "center", "right", "justify"], "alignment": ["start", "center", "end", "stretch", "baseline"], "justification": ["start", "center", "end", "between", "around", "evenly"], "position": ["relative", "absolute", "fixed", "sticky"], "overflow": ["visible", "hidden"], "text-transform": ["none", "uppercase", "lowercase", "capitalize"], "white-space": ["normal", "nowrap", "pre", "pre-wrap", "pre-line"], "text-overflow": ["clip", "ellipsis"], "sizing-keyword": ["fit-content", "max-content", "min-content"], "border-style": ["solid", "dashed", "dotted"], "overscroll": ["auto", "contain", "none"], "grid-auto-flow": ["row", "column", "row-dense", "column-dense"], "align-content": ["start", "center", "end", "between", "around", "evenly", "stretch"], "direction": ["ltr", "rtl", "auto"], "role": ["heading", "list", "listitem", "dialog", "alert", "status"], "live-region": ["off", "polite", "assertive"], "input-keyboard": ["text", "number", "email", "phone", "search", "date", "time"], "auto-complete": ["off", "on", "username", "current-password", "new-password", "one-time-code", "email", "tel", "name", "given-name", "family-name", "street-address", "postal-code", "country", "cc-number", "cc-exp", "cc-csc", "bday"], "enter-key-hint": ["enter", "done", "go", "next", "previous", "search", "send"], "auto-capitalize": ["none", "sentences", "words", "characters"], "transition-property": ["opacity", "transform", "bg", "fg", "border", "shadow"], "user-select": ["none", "text", "all"] };
  var commonProps = ["style", "className", "key", "testId", "dir", "children", "role", "headingLevel", "accessibilityHidden", "accessibilityLiveRegion", "accessibilityState"];
  var roles = ["canvas", "surface", "surface-raised", "surface-sunken", "line", "text", "text-muted", "text-faint", "accent", "on-accent", "accent-soft", "on-accent-soft", "danger", "on-danger", "danger-soft", "success", "on-success", "success-soft", "warning", "on-warning", "warning-soft", "focus", "scrim", "chart-1", "chart-2", "chart-3", "chart-4", "chart-5", "line-strong"];

  // src/style/classes.js
  var byName = /* @__PURE__ */ new Map();
  var cache = /* @__PURE__ */ new Map();
  var states = ["focus", "active", "hover", "disabled"];
  var geometryKeys = { linear: ["direction", "angle"], radial: ["shape", "size", "at"], conic: ["from", "at"] };
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
    const { theme, minWidth, maxWidth, direction } = rule.when ?? {};
    if (theme && (env.theme ?? "light") !== theme) return false;
    if (direction && (env.direction ?? "ltr") !== direction) return false;
    if (rule.when?.structure && !(env.structure ?? []).includes(rule.when.structure)) return false;
    if (rule.when?.reducedMotion !== void 0 && env.reducedMotion === true !== rule.when.reducedMotion) return false;
    if (minWidth !== void 0 && !(env.viewport?.width >= minWidth)) return false;
    if (maxWidth !== void 0 && !(env.viewport?.width < maxWidth)) return false;
    return true;
  }
  var pixels = (value) => `${value}px`;
  function cascade(rules, meta) {
    const style2 = {}, gradient = {}, shadow = {}, backdrop2 = {}, transform = {}, motion = {};
    let numeric = {};
    for (const rule of rules) {
      Object.assign(style2, copy(rule.style));
      if (meta && rule.style?.lineHeight !== void 0) meta.pairedLeading = rule.pairedLeading === true;
      Object.assign(motion, rule.motion);
      Object.assign(transform, rule.transform);
      if (rule.numeric?.normal || rule.numeric?.fixed) numeric = { ...rule.numeric };
      else if (rule.numeric) numeric = { ...numeric, ...rule.numeric, normal: false, fixed: void 0 };
      Object.assign(gradient, rule.gradient);
      Object.assign(shadow, rule.shadow);
      Object.assign(backdrop2, rule.backdrop);
    }
    const filter = backdrop2.mode === "fixed" ? backdrop2.fixed : backdrop2.mode === "variables" ? backdrop2 : void 0;
    if (filter && (filter.blur != null || filter.saturation != null))
      style2.backdropFilter = `blur(${pixels(filter.blur ?? 0)}) saturate(${filter.saturation ?? 1})`;
    if (motion.properties?.length) style2.transition = {
      properties: [...motion.properties],
      durationMs: motion.duration ?? motion.durationDefault ?? 150,
      delayMs: motion.delay ?? 0,
      easing: copy(motion.easing ?? motion.easingDefault ?? [0.4, 0, 0.2, 1])
    };
    else if (motion.properties) style2.transition = "none";
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
    const environment = `${env.theme ?? "light"}|${env.viewport?.width}|${env.viewport?.height}|${env.pointer}|${env.direction ?? "ltr"}|${(env.structure ?? []).join(",")}|${env.reducedMotion === true}`;
    const key = `${environment}\0${className}`;
    if (cache.has(key)) return copy(cache.get(key));
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
    const plain2 = own2.filter((rule) => !rule.when?.state);
    const meta = {};
    const base = cascade(plain2, meta);
    const result = { base, ...meta.pairedLeading ? { pairedLeading: true } : {} };
    for (const state of states) {
      const extra = own2.filter((rule) => rule.when?.state === state);
      if (!extra.length || state === "hover" && env.pointer !== "fine") continue;
      const full = cascade([...plain2, ...extra]);
      result[state] = state === "disabled" ? full : changed(full, base);
    }
    const children = matched.filter((rule) => rule.when?.children);
    if (children.length) result.children = cascade(children);
    if (cache.size >= 1024) cache.delete(cache.keys().next().value);
    cache.set(key, copy(result));
    return result;
  }

  // src/generated/wire.js
  var tags = { "column": 0, "row": 1, "text": 2, "button": 3, "spacer": 4, "scroll": 5, "list": 6, "image": 7, "input": 8, "textarea": 9, "checkbox": 10, "switch": 11, "radio": 12, "radio-option": 13, "search": 14, "slider": 15, "grid": 16, "stack": 17, "plot": 18, "vector": 19, "page": 20, "span": 21, "link": 22, "select": 23 };
  var field = { "width": 0, "height": 1, "gap": 2, "padding": 3, "fontSize": 4, "color": 5, "background": 6, "grow": 7, "onClick": 8, "enabled": 9, "radius": 10, "listId": 11, "listCount": 12, "listLeading": 13, "listTrailing": 14, "listRowIndex": 15, "listAnchor": 16, "onReachStart": 17, "onReachEnd": 18, "imageSrc": 19, "imageFit": 20, "inputValue": 21, "placeholder": 22, "secure": 23, "keyboard": 24, "autoFocus": 25, "onInput": 26, "onSubmit": 27, "minWidth": 28, "minHeight": 29, "maxWidth": 30, "maxHeight": 31, "shrink": 32, "align": 33, "justify": 34, "alignSelf": 35, "fontWeight": 36, "lineHeight": 37, "fontFamily": 38, "checked": 39, "onCheckedChange": 40, "accessibilityLabel": 41, "sliderValue": 42, "sliderMin": 43, "sliderMax": 44, "sliderStep": 45, "onSliderChange": 46, "gridColumns": 47, "offsetX": 48, "offsetY": 49, "textLines": 50, "plotValues": 51, "plotKind": 52, "plotColors": 53, "plotSmooth": 54, "plotArea": 55, "onPlotSelect": 56, "plotSelectedIndex": 57, "opacity": 58, "paddingTop": 59, "paddingRight": 60, "paddingBottom": 61, "paddingLeft": 62, "wordBreak": 63, "overflowWrap": 64, "borderWidth": 65, "borderColor": 66, "borderTopWidth": 67, "borderRightWidth": 68, "borderBottomWidth": 69, "borderLeftWidth": 70, "textAlign": 71, "backgroundGradient": 72, "boxShadow": 73, "widthPercent": 74, "heightPercent": 75, "minWidthPercent": 76, "minHeightPercent": 77, "maxWidthPercent": 78, "maxHeightPercent": 79, "vectorSource": 80, "textDecorationLine": 81, "heightAuto": 82, "submitOnEnter": 83, "fontStyle": 84, "plotDomainMin": 85, "plotDomainMax": 86, "flexWrap": 87, "plotPieLayout": 88, "letterSpacing": 89, "radiusTopLeft": 90, "radiusTopRight": 91, "radiusBottomRight": 92, "radiusBottomLeft": 93, "plotMarkerEmphasisScale": 94, "backdropBlur": 95, "backdropSaturation": 96, "placeholderColor": 97, "focusStyle": 98, "flexBasis": 99, "scrollAxis": 100, "animation": 101, "listItemKeys": 102, "flexDirection": 103, "display": 104, "imageAlt": 105, "imageRetryKey": 106, "onImageLoad": 107, "onImageError": 108, "marginTop": 109, "marginRight": 110, "marginBottom": 111, "marginLeft": 112, "position": 113, "insetTop": 114, "insetRight": 115, "insetBottom": 116, "insetLeft": 117, "zIndex": 118, "overflow": 119, "rowGap": 120, "columnGap": 121, "textTransform": 122, "whiteSpace": 123, "textOverflow": 124, "pressedStyle": 125, "hoverStyle": 126, "widthKeyword": 127, "heightKeyword": 128, "borderStyle": 129, "transform": 130, "fontVariantNumeric": 131, "overscroll": 132, "aspectRatio": 133, "maxWidthCh": 134, "gridTemplateColumns": 135, "gridTemplateRows": 136, "gridAutoColumns": 137, "gridAutoRows": 138, "gridAutoFlow": 139, "gridColumn": 140, "gridRow": 141, "justifyItems": 142, "justifySelf": 143, "alignContent": 144, "order": 145, "direction": 146, "marginInlineStart": 147, "marginInlineEnd": 148, "paddingInlineStart": 149, "paddingInlineEnd": 150, "insetInlineStart": 151, "insetInlineEnd": 152, "borderInlineStartWidth": 153, "borderInlineEndWidth": 154, "radiusStartStart": 155, "radiusStartEnd": 156, "radiusEndStart": 157, "radiusEndEnd": 158, "role": 159, "headingLevel": 160, "accessibilityHidden": 161, "accessibilityLiveRegion": 162, "maxLength": 163, "autoComplete": 164, "enterKeyHint": 165, "autoCapitalize": 166, "spellCheck": 167, "inputMin": 168, "inputMax": 169, "inputStep": 170, "transition": 171, "userSelect": 172, "focusRequest": 173, "statePressed": 174, "stateSelected": 175, "stateExpanded": 176, "stateBusy": 177, "selectOptions": 178, "onSelect": 179, "rows": 180, "maxRows": 181, "imagePlaceholder": 182, "imagePosition": 183 };
  var fieldOptions = { "keyboard": ["text", "number", "email", "phone", "search", "date", "time"], "align": ["start", "center", "end", "stretch", "baseline"], "justify": ["start", "center", "end", "between", "around", "evenly"], "alignSelf": ["start", "center", "end", "stretch", "baseline"], "fontFamily": ["sans", "mono"], "wordBreak": ["normal", "break-all", "keep-all"], "overflowWrap": ["normal", "break-word", "anywhere"], "textAlign": ["start", "end", "left", "center", "right", "justify"], "textDecorationLine": ["none", "line-through", "underline"], "fontStyle": ["normal", "italic"], "flexWrap": ["nowrap", "wrap", "wrap-reverse"], "scrollAxis": ["horizontal", "vertical"], "flexDirection": ["row", "column", "row-reverse", "column-reverse"], "display": ["flex", "grid", "none", "block", "inline-flex", "inline-block"], "position": ["relative", "absolute", "fixed", "sticky"], "overflow": ["visible", "hidden"], "textTransform": ["none", "uppercase", "lowercase", "capitalize"], "whiteSpace": ["normal", "nowrap", "pre", "pre-wrap", "pre-line"], "textOverflow": ["clip", "ellipsis"], "widthKeyword": ["fit-content", "max-content", "min-content"], "heightKeyword": ["fit-content", "max-content", "min-content"], "borderStyle": ["solid", "dashed", "dotted"], "overscroll": ["auto", "contain", "none"], "gridAutoFlow": ["row", "column", "row-dense", "column-dense"], "justifyItems": ["start", "center", "end", "stretch", "baseline"], "justifySelf": ["start", "center", "end", "stretch", "baseline"], "alignContent": ["start", "center", "end", "between", "around", "evenly", "stretch"], "direction": ["ltr", "rtl", "auto"], "role": ["heading", "list", "listitem", "dialog", "alert", "status"], "accessibilityLiveRegion": ["off", "polite", "assertive"], "autoComplete": ["off", "on", "username", "current-password", "new-password", "one-time-code", "email", "tel", "name", "given-name", "family-name", "street-address", "postal-code", "country", "cc-number", "cc-exp", "cc-csc", "bday"], "enterKeyHint": ["enter", "done", "go", "next", "previous", "search", "send"], "autoCapitalize": ["none", "sentences", "words", "characters"], "userSelect": ["none", "text", "all"] };

  // src/style/color.js
  var names = new Set(roles);
  function isRole(value) {
    const match = typeof value === "string" && /^([a-z0-9-]+)(?:\/(\d{1,3}))?$/.exec(value);
    return Boolean(match) && names.has(match[1]) && (match[2] === void 0 || Number(match[2]) <= 100);
  }
  function colorProblem(value, where = "a page colour") {
    if (value === "currentColor" || value === "transparent" || /^#[0-9a-fA-F]{6}00$/.test(value) || isRole(value)) return void 0;
    return `UI_COLOR: ${JSON.stringify(value)} is not a role; ${where} uses a role such as 'surface', 'text-muted', 'line', 'accent' or 'accent/12'`;
  }
  function wireColorProblem(value, where) {
    return /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(value) ? void 0 : colorProblem(value, where);
  }
  function seriesProblem(value) {
    return isRole(value) ? void 0 : `UI_COLOR: ${JSON.stringify(value)} is not a role; plot series use 'chart-1' to 'chart-5'`;
  }
  var defaultSeries = ["chart-1", "chart-2", "chart-3", "chart-4", "chart-5"];

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
      if (result.color !== "currentColor" && (typeof result.color !== "string" || !(/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(result.color) || isRole(result.color)))) invalid2();
      return result;
    });
  }

  // src/style/gradient.js
  var hex = /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/;
  var directions = ["top", "right", "bottom", "left", "top-right", "bottom-right", "bottom-left", "top-left"];
  var geometry = { linear: ["direction", "angle"], radial: ["shape", "size", "at"], conic: ["from", "at"] };
  var sizes = ["closest-side", "closest-corner", "farthest-side", "farthest-corner"];
  var angle = (value) => typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 36e3;
  function normalizeGradient(value) {
    if (value === "none") return value;
    const invalid2 = () => {
      throw Error("UI_GRADIENT: invalid gradient");
    };
    if (!value || typeof value !== "object" || Array.isArray(value)) invalid2();
    const kind = value.kind ?? "linear";
    const keys = geometry[kind] ?? invalid2();
    if (Object.keys(value).some((key) => !["kind", "colorSpace", "stops", ...keys].includes(key)) || value.direction !== void 0 && !directions.includes(value.direction) || value.angle !== void 0 && !angle(value.angle) || value.direction !== void 0 && value.angle !== void 0 || value.from !== void 0 && !angle(value.from) || value.shape !== void 0 && !["circle", "ellipse"].includes(value.shape) || value.size !== void 0 && !sizes.includes(value.size) || value.at !== void 0 && (!Array.isArray(value.at) || value.at.length !== 2 || value.at.some((v) => typeof v !== "number" || !Number.isFinite(v) || Math.abs(v) > 16)) || value.colorSpace !== void 0 && !["srgb", "oklab"].includes(value.colorSpace) || !Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 16) invalid2();
    const [low, high] = kind === "linear" ? [-16, 16] : kind === "radial" ? [0, 16] : [0, 1];
    let previous = -Infinity;
    const stops = value.stops.map((stop) => {
      if (!stop || typeof stop !== "object" || Array.isArray(stop) || Object.keys(stop).some((key) => key !== "offset" && key !== "color") || typeof stop.offset !== "number" || !Number.isFinite(stop.offset) || stop.offset < low || stop.offset > high || stop.offset < previous || typeof stop.color !== "string" || !(hex.test(stop.color) || isRole(stop.color))) invalid2();
      previous = stop.offset;
      return { offset: stop.offset, color: stop.color };
    });
    const result = kind === "linear" ? {} : { kind };
    for (const key of keys) if (value[key] !== void 0) result[key] = Array.isArray(value[key]) ? [...value[key]] : value[key];
    return { ...result, colorSpace: value.colorSpace ?? "srgb", stops };
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
    const ch = typeof value === "string" && value.match(/^(\d+(?:\.\d+)?)ch$/);
    if (ch) {
      if (key !== "maxWidth") invalid(name);
      out.maxWidthCh = length(name, Number(ch[1]), { max: 4096 });
      return;
    }
    const viewport = typeof value === "string" && value.match(/^(\d+(?:\.\d+)?)(vw|vh)$/);
    if (viewport) {
      const insets = env?.insets ?? {}, keyboard = env?.keyboard?.height ?? 0;
      const extent = viewport[2] === "vw" ? env?.viewport?.width - (insets.left ?? 0) - (insets.right ?? 0) : env?.viewport?.height - (insets.top ?? 0) - Math.max(insets.bottom ?? 0, keyboard);
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
  function tracks(name, value) {
    if (typeof value !== "string" || value.length > 1024 || !/^[a-z0-9\s().,%-]+$/.test(value)) invalid(name);
    return value.trim().replace(/\s+/g, " ");
  }
  function gridEdge(name, value) {
    const text = typeof value === "number" ? String(value) : typeof value === "string" ? value.trim().replace(/\s+/g, " ") : "";
    if (!/^(?:auto|-?[1-9]\d?|span [1-9]\d?)$/.test(text)) invalid(name);
    return text;
  }
  function gridLine(name, value, lines) {
    const axis = name.startsWith("gridColumn") ? "Column" : "Row";
    const line = lines[axis] ?? (lines[axis] = {});
    if (name.endsWith("Start")) line.start = gridEdge(name, value);
    else if (name.endsWith("End")) line.end = gridEdge(name, value);
    else {
      const parts = String(value).split("/");
      if (parts.length > 2) invalid(name);
      line.shorthand = [gridEdge(name, parts[0]), parts[1] === void 0 ? "auto" : gridEdge(name, parts[1])];
    }
  }
  var translateLength = (value) => finite(value) ? Math.abs(value) <= 16384 : typeof value === "string" && /^-?(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) && Math.abs(parseFloat(value)) <= 1e3;
  function transformPart(name, value, out) {
    const pair = (check) => {
      const parts = Array.isArray(value) ? value : [value, name === "scale" ? value : 0];
      if (parts.length !== 2 || !parts.every(check)) invalid(name);
      return [...parts];
    };
    if (name === "translate") out.translate = pair(translateLength);
    else if (name === "scale") out.scale = pair((v) => finite(v) && Math.abs(v) <= 1e3);
    else if (name === "rotate") {
      if (!finite(value) || Math.abs(value) > 36e3) invalid(name);
      out.rotate = value;
    } else {
      if (!Array.isArray(value) || value.length !== 2 || !value.every((v) => finite(v) && Math.abs(v) <= 16)) invalid(name);
      out.origin = [...value];
    }
  }
  var numericGroups = [["ordinal"], ["slashed-zero"], ["lining-nums", "oldstyle-nums"], ["proportional-nums", "tabular-nums"], ["diagonal-fractions", "stacked-fractions"]];
  function numericVariant(name, value) {
    if (value === "normal") return value;
    const tokens = typeof value === "string" ? value.trim().split(/\s+/) : [];
    const picked = numericGroups.map((group) => tokens.filter((token) => group.includes(token)));
    if (!tokens.length || picked.some((group) => group.length > 1) || picked.flat().length !== tokens.length) invalid(name);
    return picked.flat().join(" ");
  }
  function transitionValue(value) {
    if (value === "none") return void 0;
    const invalid2 = () => {
      throw Error("UI_STYLE_VALUE: transition");
    };
    if (!value || typeof value !== "object" || Object.keys(value).some((key) => !["properties", "durationMs", "delayMs", "easing"].includes(key))) invalid2();
    const { properties, durationMs, delayMs = 0, easing = [0.4, 0, 0.2, 1] } = value;
    if (!Array.isArray(properties) || !properties.length || properties.length > options["transition-property"].length || new Set(properties).size !== properties.length || properties.some((name) => !options["transition-property"].includes(name)) || !finite(durationMs) || durationMs <= 0 || durationMs > limits.transitionMs || !finite(delayMs) || delayMs < 0 || delayMs > limits.transitionMs || !(easing === "linear" || easing === "step-end" || Array.isArray(easing) && easing.length === 4 && easing.every(finite))) invalid2();
    return JSON.stringify({ properties, durationMs, delayMs, easing });
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
    const parts = typeof value === "string" ? value.trim().split(/\s+/) : [];
    const horizontal = { left: 0, right: 1 }, vertical = { top: 0, bottom: 1 };
    const fraction = (part) => {
      if (part === "center") return 0.5;
      const match = /^(-?(?:\d*\.)?\d+)%$/.exec(part);
      if (!match || Math.abs(Number(match[1])) > 1600) invalid("objectPosition");
      return Number(match[1]) / 100;
    };
    if (parts.length === 1) {
      const [part] = parts;
      return part in vertical ? [0.5, vertical[part]] : [part in horizontal ? horizontal[part] : fraction(part), 0.5];
    }
    if (parts.length !== 2) invalid("objectPosition");
    let [x, y] = parts;
    if (x in vertical || y in horizontal) [x, y] = [y, x];
    if (x in vertical || y in horizontal) invalid("objectPosition");
    return [x in horizontal ? horizontal[x] : fraction(x), y in vertical ? vertical[y] : fraction(y)];
  }
  function lowerStyle(tag, style2, env, inherited = { fontSize: 14 }) {
    admitStyle(tag, style2);
    const out = {};
    const fontSize = style2.fontSize === void 0 ? inherited.fontSize : length("fontSize", style2.fontSize);
    if (style2.fontSize !== void 0 && style2.fontSize <= 0) invalid("fontSize");
    let lineHeight = inherited.lineHeight;
    const transform = {}, lines = {};
    const names2 = Object.keys(style2).sort((a, b) => Number(shorthand(b)) - Number(shorthand(a)));
    for (const name of names2) {
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
          if (!Number.isInteger(value) || Math.abs(value) > 1e4) invalid(name);
          out[key] = value;
          break;
        case "align-content":
          out[key] = option(name, key, justification[value] ?? value);
          break;
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
        case "color": {
          if (typeof value !== "string" || !value) invalid(name);
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
        case "aspect-ratio":
          if (value === "auto") break;
          if (!finite(value) || value <= 0 || value > 1e4) invalid(name);
          out[key] = value;
          break;
        case "font-variant-numeric":
          out[key] = numericVariant(name, value);
          break;
        case "basis":
          if (value !== "auto") length(name, percent(value) ?? value);
          out[key] = value;
          break;
        case "option":
          if (name === "userSelect" && ["input", "textarea", "search"].includes(tag)) throw Error("UI_STYLE: editors select their text already; userSelect is for static text");
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
        case "object-position":
          out[key] = JSON.stringify(objectPosition(value));
          break;
        default:
          throw Error(`UI_STYLE: ${name} has no native lowering`);
      }
    }
    if (Object.keys(transform).length) out.transform = JSON.stringify(transform);
    for (const axis of ["Column", "Row"]) {
      const line = lines[axis];
      if (!line) continue;
      const start = line.start ?? line.shorthand?.[0] ?? "auto", end = line.end ?? line.shorthand?.[1] ?? "auto";
      out[`grid${axis}`] = end === "auto" ? start : `${start} / ${end}`;
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
      const site = authorSite(base.stack);
      if (site) base.uiFrames.push(site);
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
  var sourceMap = null;
  var decoded = null;
  function configureSourceMap(map) {
    sourceMap = map;
    decoded = null;
  }
  var base64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  function decode() {
    const lines = [];
    let source = 0, line = 0, column = 0;
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
        source += values[1];
        line += values[2];
        column += values[3];
        segments.push([generated, source, line, column]);
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
    const names2 = metadata?.__names, modes = metadata?.__modes;
    if (!Array.isArray(names2) || names2.length > 256 || !modes || typeof modes !== "object" || names2.some((name) => typeof name !== "string" || !name || name.length > 256 || !has(modes, name) || !["call", "stream"].includes(modes[name])) || new Set(names2).size !== names2.length) throw Error("UI_ACTION: invalid action catalog");
    cancelActions();
    const epoch2 = actionEpoch, built = /* @__PURE__ */ Object.create(null);
    const reportLive = (cause) => {
      if (epoch2 === actionEpoch) report(cause);
    };
    for (const name of names2) built[name] = (...args) => {
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
    for (const name of names2) {
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
      },
      focus() {
        outsideRender("focus");
        if (page.locals.get(key) !== cell) throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
        cell.focus = (cell.focus ?? 0) + 1;
        page.updates.set(cell, page.updates.has(cell) ? page.updates.get(cell) : cell.value);
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
  function useStream(name, action, options2 = {}) {
    if (!options2 || typeof options2 !== "object" || options2.onData !== void 0 && typeof options2.onData !== "function")
      throw Error("UI_STREAM: onData must be a function");
    const target = pageAction(action, "UI_STREAM", "stream");
    const resource = own(
      name,
      { status: "idle", error: null, result: null },
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
  var finiteWithin = (value, bound) => typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= bound;
  var hexColor = (value) => typeof value === "string" && /^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(value);
  var pathKeys = ["d", "fill", "stroke", "strokeWidth", "strokeDasharray", "strokeDashoffset", "strokeLinecap", "strokeLinejoin", "transform", "vectorEffect"];
  var gradientKeys = ["type", "x1", "y1", "x2", "y2", "cx", "cy", "r", "fx", "fy", "fr", "from", "units", "spread", "stops"];
  var plain = (value) => value && typeof value === "object" && !Array.isArray(value);
  function vectorGradient(id, gradient) {
    const invalid2 = () => {
      throw Error(`UI_VECTOR: invalid gradient ${id}`);
    };
    if (!/^[A-Za-z0-9_-]{1,64}$/.test(id) || !plain(gradient) || Object.keys(gradient).some((k) => !gradientKeys.includes(k)) || !["linear", "radial", "conic"].includes(gradient.type) || { linear: ["cx", "cy", "r", "fx", "fy", "fr", "from"], radial: ["x1", "y1", "x2", "y2", "from"], conic: ["x1", "y1", "x2", "y2", "r", "fx", "fy", "fr"] }[gradient.type].some((k) => gradient[k] !== void 0) || gradient.from !== void 0 && !finiteWithin(gradient.from, 36e3) || ["x1", "y1", "x2", "y2", "cx", "cy", "fx", "fy"].some((k) => gradient[k] !== void 0 && !finiteWithin(gradient[k], 16384)) || ["r", "fr"].some((k) => gradient[k] !== void 0 && (!finiteWithin(gradient[k], 16384) || gradient[k] < 0)) || gradient.units !== void 0 && !["objectBoundingBox", "userSpaceOnUse"].includes(gradient.units) || gradient.spread !== void 0 && !["pad", "reflect", "repeat"].includes(gradient.spread) || !Array.isArray(gradient.stops) || !gradient.stops.length || gradient.stops.length > 16 || gradient.stops.some((stop) => !plain(stop) || Object.keys(stop).some((k) => !["offset", "color", "opacity"].includes(k)) || !finiteWithin(stop.offset, 1) || stop.offset < 0 || !(hexColor(stop.color) || isRole(stop.color)) || stop.opacity !== void 0 && (!finiteWithin(stop.opacity, 1) || stop.opacity < 0))) invalid2();
  }
  function vector(props, fit) {
    const box = props.viewBox ?? [0, 0, 24, 24];
    if (!Array.isArray(box) || box.length !== 4 || box.some((v) => !finiteWithin(v, 16384)) || box[2] < 1 / 64 || box[3] < 1 / 64)
      throw Error("UI_VECTOR: invalid viewBox");
    if (!Array.isArray(props.paths) || !props.paths.length || props.paths.length > 32) throw Error("UI_VECTOR: invalid paths");
    const gradients = props.gradients ?? {};
    if (!plain(gradients) || Object.keys(gradients).length > 8) throw Error("UI_VECTOR: at most 8 gradients");
    for (const [id, gradient] of Object.entries(gradients)) vectorGradient(id, gradient);
    for (const path of props.paths) {
      if (!plain(path) || Object.keys(path).some((k) => !pathKeys.includes(k)) || typeof path.d !== "string" || !path.d.length || path.d.length > limits.vectorPathBytes) throw Error("UI_VECTOR: invalid path");
      for (const key of ["fill", "stroke"]) {
        const value = path[key];
        if (value === void 0 || value === "none" || value === "currentColor" || hexColor(value) || isRole(value)) continue;
        const id = typeof value === "string" && value.match(/^url\(#(.+)\)$/)?.[1];
        if (!id) throw Error("UI_VECTOR: invalid color");
        if (!has(gradients, id)) throw Error(`UI_VECTOR: unknown gradient ${id}`);
      }
      if (path.vectorEffect !== void 0 && !["none", "non-scaling-stroke"].includes(path.vectorEffect)) throw Error("UI_VECTOR: invalid vector effect");
      if (path.strokeLinecap !== void 0 && !["butt", "round", "square"].includes(path.strokeLinecap)) throw Error("UI_VECTOR: invalid stroke-linecap");
      if (path.strokeLinejoin !== void 0 && !["miter", "round", "bevel"].includes(path.strokeLinejoin)) throw Error("UI_VECTOR: invalid stroke-linejoin");
      if (path.strokeDashoffset !== void 0 && !finiteWithin(path.strokeDashoffset, 16384)) throw Error("UI_VECTOR: invalid dash offset");
      if (path.transform !== void 0 && (typeof path.transform !== "string" || path.transform.length > 512 || !/^\s*(?:(?:matrix|translate|scale|rotate|skewX|skewY)\s*\([^()]*\)[\s,]*)+$/.test(path.transform))) throw Error("UI_VECTOR: invalid transform");
      if (path.strokeDasharray !== void 0 && (!Array.isArray(path.strokeDasharray) || path.strokeDasharray.length > 16 || path.strokeDasharray.some((v) => typeof v !== "number" || !Number.isFinite(v) || v < 1 / 64 || v > 1024))) throw Error("UI_VECTOR: invalid dash pattern");
      if (path.strokeWidth !== void 0 && (typeof path.strokeWidth !== "number" || !Number.isFinite(path.strokeWidth) || path.strokeWidth < 0 || path.strokeWidth > 1024))
        throw Error("UI_VECTOR: invalid stroke width");
    }
    const source = JSON.stringify({ viewBox: box, paths: props.paths, ...Object.keys(gradients).length ? { gradients } : {}, ...fit === void 0 ? {} : { fit } });
    if (source.length > limits.vectorSourceBytes) throw Error("UI_VECTOR: source too large");
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
    if (props.value === void 0) props.value = props.min;
    const { min, max, step, value } = props;
    if (![min, max, step, value].every((v) => typeof v === "number" && Number.isFinite(v) && Math.abs(v) <= 1e9) || max <= min || step <= 0 || step > max - min || (max - min) / step > 1e6 || min + step === min || max - step === max)
      throw Error("UI_SLIDER: invalid range or precision");
    label(props.accessibilityLabel, "UI_SLIDER", true);
    props.value = sliderValue(props, value);
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
  function listKeys(props) {
    if (!Number.isSafeInteger(props.count) || props.count < 0 || props.count > limits.listRows || typeof props.row !== "function" || props.height !== void 0 && typeof props.height !== "function") throw Error("UI_LIST: invalid row description");
    if (props.itemKeys !== void 0) {
      if (!Array.isArray(props.itemKeys) || props.itemKeys.length !== props.count) throw Error("UI_LIST: itemKeys must match count");
      const seen = /* @__PURE__ */ new Set();
      props.itemKeys = Array.from(props.itemKeys, (key) => {
        if (typeof key !== "string" || utf8Bytes(key, limits.identifierBytes) > limits.identifierBytes || utf8Bytes(`key:${JSON.stringify(key)}`, limits.identifierBytes) > limits.identifierBytes || seen.has(key))
          throw Error("UI_LIST: invalid or duplicate itemKey");
        seen.add(key);
        return key;
      });
      if (utf8Bytes(JSON.stringify(props.itemKeys), limits.itemKeyBytes) > limits.itemKeyBytes) throw Error("UI_LIST: itemKeys budget exceeded");
    }
    if (props.anchor !== void 0 && props.anchor !== "end") {
      const a = props.anchor;
      if (!a || typeof a !== "object" || Object.keys(a).some((k) => !["index", "align", "seq"].includes(k)) || !Number.isSafeInteger(a.index) || a.index < 0 || !["start", "end"].includes(a.align) || a.seq !== void 0 && (!Number.isSafeInteger(a.seq) || a.seq < 0)) throw Error("UI_LIST: invalid anchor");
      props.anchor = { index: a.index, align: a.align, ...a.seq === void 0 ? {} : { seq: a.seq } };
    }
  }
  var iso = { date: /^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/, time: /^([01]\d|2[0-3]):[0-5]\d(:[0-5]\d)?$/ };
  function editor(tag, props) {
    const out = [];
    const keyboard = props.keyboard;
    if (keyboard !== void 0 && (keyboard === "search" || !fieldOptions.keyboard.includes(keyboard))) {
      const allowed = fieldOptions.keyboard.filter((name) => name !== "search").join(", ");
      const hint = keyboard === "tel" ? '; use "phone"' : keyboard === "search" ? "; use <search>" : "";
      throw Error(`UI_INPUT: keyboard ${JSON.stringify(keyboard)} is not one of ${allowed}${hint}`);
    }
    if ((keyboard === "date" || keyboard === "time") && tag !== "input") throw Error(`UI_INPUT: ${keyboard} pickers need <input>`);
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
    const bounded = ["min", "max", "step"].filter((name) => props[name] !== void 0);
    if (bounded.length) {
      if (tag !== "input" || !["number", "date", "time"].includes(keyboard)) throw Error("UI_INPUT: min, max and step need keyboard number, date or time");
      const valid = (value) => keyboard === "number" ? typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 1e15 : typeof value === "string" && iso[keyboard].test(value);
      for (const name of ["min", "max"]) if (props[name] !== void 0 && !valid(props[name])) throw Error(`UI_INPUT: ${name} must be a ${keyboard === "number" ? "number" : `${keyboard} in ISO form`}`);
      if (props.min !== void 0 && props.max !== void 0 && props.min > props.max) throw Error("UI_INPUT: min exceeds max");
      if (props.step !== void 0 && (keyboard !== "number" || typeof props.step !== "number" || !(props.step > 0) || props.step > 1e15))
        throw Error('UI_INPUT: step is a positive number on keyboard="number"');
      for (const [name, key] of [["min", "inputMin"], ["max", "inputMax"], ["step", "inputStep"]]) if (props[name] !== void 0) out.push([key, props[name]]);
    }
    return out;
  }

  // src/runtime/elements.js
  var common = new Set(commonProps);
  var semanticStates = { pressed: "statePressed", selected: "stateSelected", expanded: "stateExpanded", busy: "stateBusy" };
  var accepts = (field2, tag) => admission.fields[field2].includes(elements[tag].wire);
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
  var lowered = /* @__PURE__ */ new Map();
  var loweredEnv = null;
  function envKey(env) {
    return JSON.stringify([env?.theme, env?.viewport, env?.pointer, env?.reducedMotion, env?.insets, env?.keyboard]);
  }
  function nodeStyle(tag, props, extra, inherited, direction, structure2) {
    if (props.style !== void 0 || extra !== void 0 || props.className === void 0) return lowerNodeStyle(tag, props, extra, inherited, direction, structure2);
    const environment = envKey(render.view?.env);
    if (environment !== loweredEnv) {
      lowered.clear();
      loweredEnv = environment;
    }
    const key = `${tag}\0${props.className}\0${props.disabled === true}\0${direction}\0${(structure2 ?? []).join(",")}\0${inherited.fontSize}\0${inherited.lineHeight}`;
    let style2 = lowered.get(key);
    if (!style2) {
      style2 = lowerNodeStyle(tag, props, extra, inherited, direction, structure2);
      if (lowered.size >= 2048) lowered.delete(lowered.keys().next().value);
      lowered.set(key, style2);
    }
    return { ...style2, fields: { ...style2.fields } };
  }
  var fieldTags = /* @__PURE__ */ new Set(["input", "textarea", "search", "select"]);
  var runTags = /* @__PURE__ */ new Set(["span", "link"]);
  var disabledField = { color: "text-muted", backgroundColor: "surface-sunken", borderColor: "line" };
  function lowerNodeStyle(tag, props, extra, inherited, direction, structure2) {
    const env = { ...render.view?.env, direction, structure: structure2 };
    const classes = resolveClasses(props.className, env);
    if (classes?.pairedLeading && runTags.has(tag)) delete classes.base.lineHeight;
    const disabled = props.disabled === true;
    const muted = disabled && !classes?.disabled && fieldTags.has(tag) ? disabledField : void 0;
    const author = mergeStyle(direction, extra, disabled && classes?.disabled ? classes.disabled : classes?.base, muted, props.style);
    const { fields, typography } = lowerStyle(tag, author, env, inherited);
    const states2 = {};
    const state = (name, wire, allowed, requirement) => {
      const delta = classes?.[name];
      if (!delta || !Object.keys(delta).length) return;
      if (!allowed) throw Error(`UI_STYLE: ${name}: requires ${requirement}, not <${tag}>`);
      paintOnly(name, tag, delta);
      states2[wire] = lowerStyle(tag, delta, env, { fontSize: typography.fontSize }).fields;
    };
    state("focus", "focusStyle", accepts("focusStyle", tag), "an editor");
    if (!disabled) {
      state("active", "pressedStyle", accepts("pressedStyle", tag), "a control");
      state("hover", "hoverStyle", true);
    }
    return { fields, states: states2, typography, author, children: classes?.children };
  }
  function resolveNode(node, identity, depth = 0, parent = {}) {
    if (depth > limits.depth) throw Error("UI_LIMIT: children too deep");
    if (node == null || typeof node === "boolean" || node === "") return null;
    if (typeof node === "string" || typeof node === "number") node = h("text", {}, node);
    if (!isNode(node)) throw Error("UI_ROOT: expected one node");
    const [tag, original, children, location] = readNode(node);
    if (location) render.site = `${typeof tag === "function" ? tag.name || "<anonymous>" : `<${tag}>`} (${location})`;
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
    if (parent.inControl && !admission.tags[element.wire].content) throw Error("UI_BUTTON: descendants must be noninteractive content");
    if (element.binding) bind(tag, element, props);
    const editor2 = ["input", "textarea", "search"].includes(tag);
    if (props.dir !== void 0 && !(["ltr", "rtl"].includes(props.dir) || editor2 && props.dir === "auto"))
      throw Error(`UI_PROP: dir must be "ltr" or "rtl"${editor2 ? ' or "auto"' : ""}`);
    const direction = (props.dir === "auto" ? void 0 : props.dir) ?? parent.direction ?? (render.view?.env?.direction === "rtl" ? "rtl" : "ltr");
    const flat = flatten(children);
    const style2 = nodeStyle(tag, props, parent.extra, parent.typography ?? { fontSize: 14 }, direction, structure(parent.sibling, flat));
    const inline = (child) => typeof child === "string" || typeof child === "number";
    if ((tag === "span" || tag === "link") && !parent.inText) throw Error(`UI_TEXT: <${tag}> belongs inside <text>`);
    if (tag === "link" && parent.inLink) throw Error("UI_TEXT: links do not nest");
    const composed = element.children === "content" && flat.some((child) => !inline(child));
    const runs = element.children === "runs" && flat.some((child) => !inline(child));
    const leaf = element.children === "text" || element.children === "content" && !composed || element.children === "runs" && !runs;
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
    if (props.dir !== void 0) wire("direction", props.dir);
    semantics(tag, props, wire);
    switch (tag) {
      case "link":
        label(props.accessibilityLabel, "UI_TEXT");
        if (props.onPress !== void 0 && typeof props.onPress !== "function") throw Error("UI_PROP: onPress");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        if (props.onPress && !disabled) on("onClick", props.onPress);
        break;
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
        const attributes = editor(tag, props);
        if (props.onSubmit !== void 0 && typeof props.onSubmit !== "function") throw Error("UI_INPUT: onSubmit");
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        wire("inputValue", props.value ?? "");
        if (props.placeholder !== void 0) wire("placeholder", props.placeholder);
        if (tag === "search") wire("keyboard", "search");
        else if (props.keyboard !== void 0) wire("keyboard", props.keyboard);
        for (const name of ["secure", "autoFocus", "submitOnEnter"]) if (props[name] !== void 0) wire(name, +props[name]);
        if (props.bind !== void 0 && cellOf(props.bind).cell.focus) wire("focusRequest", cellOf(props.bind).cell.focus);
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
        break;
      case "select": {
        label(props.accessibilityLabel, "UI_SELECT");
        const options2 = selectOptions(props);
        if (props.value !== void 0 && typeof props.value !== "string") throw Error("UI_SELECT: value must be a string");
        if (props.placeholder !== void 0 && !validInputText(props.placeholder)) throw Error("UI_SELECT: invalid placeholder");
        const chosen = options2.find((option2) => option2.value === props.value);
        result.text = chosen?.label ?? props.placeholder ?? "";
        if (props.accessibilityLabel !== void 0) wire("accessibilityLabel", props.accessibilityLabel);
        wire("inputValue", chosen ? chosen.value : "");
        if (props.placeholder !== void 0) wire("placeholder", props.placeholder);
        wire("selectOptions", JSON.stringify(options2));
        if (props.onChange && !disabled) on("onSelect", (value) => {
          if (value !== props.value) props.onChange(value);
        }, "text");
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
        result.children = radioOptions(props, identity, result.fields, disabled);
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
    } else if (runs) {
      const extra = tag === "span" || tag === "link" ? style2.author : void 0;
      result.children = flat.flatMap((child, position) => {
        const run = resolveNode(inline(child) ? h("span", {}, child) : child, `${identity}/${tag}/position:${position}`, depth + 1, {
          tag,
          typography: style2.typography,
          direction,
          extra,
          inText: true,
          inControl: parent.inControl,
          inLink: parent.inLink || tag === "link"
        });
        if (run === null) return [];
        return (run.element === "span" || run.element === "link") && run.children.length ? run.children : [run];
      });
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
      if (tag === "text" && result.children.length > limits.runs) throw Error(`UI_TEXT: more than ${limits.runs} runs`);
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
          direction,
          extra: position < last ? style2.children : void 0,
          sibling: { index: position, count: flat.length },
          inControl: parent.inControl || admission.tags[element.wire].children === "content"
        });
      }).filter((child) => child !== null);
    }
    return result;
  }
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
  function semantics(tag, props, wire) {
    const { role, headingLevel, accessibilityHidden, accessibilityLiveRegion } = props;
    if (role !== void 0) {
      if (!fieldOptions.role.includes(role)) throw Error(`UI_PROP: role must be one of ${fieldOptions.role.join(", ")}`);
      wire("role", role);
    }
    if (headingLevel !== void 0) {
      if (role !== "heading" || !Number.isInteger(headingLevel) || headingLevel < 1 || headingLevel > 6)
        throw Error('UI_PROP: headingLevel is an integer 1..6 on role="heading"');
      wire("headingLevel", headingLevel);
    }
    if (accessibilityHidden !== void 0) {
      if (typeof accessibilityHidden !== "boolean") throw Error("UI_PROP: accessibilityHidden must be boolean");
      if (accessibilityHidden) wire("accessibilityHidden", 1);
    }
    if (accessibilityLiveRegion !== void 0) {
      if (!fieldOptions.accessibilityLiveRegion.includes(accessibilityLiveRegion)) throw Error("UI_PROP: accessibilityLiveRegion must be off, polite or assertive");
      wire("accessibilityLiveRegion", accessibilityLiveRegion);
    }
    const { accessibilityState: state } = props;
    if (state !== void 0) {
      if (!state || typeof state !== "object" || Array.isArray(state)) throw Error("UI_PROP: accessibilityState must be an object");
      for (const [name, value] of Object.entries(state)) {
        if (!has(semanticStates, name)) throw Error(`UI_PROP: accessibilityState.${name}; known: ${Object.keys(semanticStates).join(", ")}`);
        if (value === void 0) continue;
        if (typeof value !== "boolean") throw Error(`UI_PROP: accessibilityState.${name} must be boolean`);
        wire(semanticStates[name], +value);
      }
    }
    if (["view", "column", "row", "page", "text"].includes(tag) && props.accessibilityLabel !== void 0) {
      label(props.accessibilityLabel, "UI_PROP");
      wire("accessibilityLabel", props.accessibilityLabel);
    }
  }
  function selectOptions(props) {
    if (!Array.isArray(props.options) || !props.options.length || props.options.length > 64) throw Error("UI_SELECT: expected 1..64 options");
    const values = /* @__PURE__ */ new Set();
    return props.options.map((option2) => {
      if (!option2 || typeof option2 !== "object" || Object.keys(option2).some((k) => !["value", "label", "disabled", "testId"].includes(k)) || typeof option2.value !== "string" || !option2.value.length || option2.value.length > 256 || values.has(option2.value) || typeof option2.label !== "string" || !validInputText(option2.label) || option2.disabled !== void 0 && typeof option2.disabled !== "boolean")
        throw Error("UI_SELECT: invalid or duplicate option");
      values.add(option2.value);
      return option2.disabled ? { value: option2.value, label: option2.label, disabled: true } : { value: option2.value, label: option2.label };
    });
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
      if (depth > limits.depth || ++count > limits.nodes) throw Error("UI_LIMIT: candidate too large");
      const { tag, props, identity } = node, encoded = [];
      for (const [key, value] of Object.entries(node.fields)) {
        if (key === "animation") {
          animationBytes += utf8Bytes(JSON.stringify(value), 32768);
          if (animationBytes > limits.animationBytes) throw Error("UI_ANIMATION: page payload budget exceeded");
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
          itemKeyBytes += utf8Bytes(payload, limits.itemKeyBytes);
          if (itemKeyBytes > limits.itemKeyBytes) throw Error("UI_LIST: itemKeys page budget exceeded");
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
      const content = resolveNode(value, render.scope, depth + 2, { typography: node.typography, sibling: { index: i, count: props.count } });
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
      render.site = null;
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
  var accepts2 = (kind, value) => kind === "number" ? typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 1e9 : kind === "text" ? validInputText(value) : value === void 0;
  function dispatch(handle2, value) {
    outsideRender("dispatch");
    const entry = page.callbacks.get(handle2);
    if (!entry || !accepts2(entry.kind, value)) return false;
    const previous = entry.isolated ? new Map(page.updates) : void 0;
    try {
      const epoch2 = actionEpochNow();
      const result = entry.kind === "void" ? entry.callback() : entry.callback(value);
      if (result && typeof result.then === "function") Promise.resolve(result).catch((error) => {
        if (hasActionEpoch(epoch2)) report(error);
      });
      return true;
    } catch (thrown) {
      const error = sited(thrown);
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

  // src/runtime/hot-swap.js
  function exportState() {
    const locals = [];
    for (const [key, cell] of page.locals) if (JSON.parse(key).length === 2) locals.push([key, cell.value]);
    return JSON.stringify(locals);
  }
  function importState(text) {
    for (const [key, value] of JSON.parse(text)) page.locals.set(key, { value: immutable(value, 0, true) });
  }
  function enableHotSwap() {
    Object.assign(globalThis, { __lingxiaExportState: exportState, __lingxiaImportState: importState });
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
      __lingxiaRequestLeave: requestLeave,
      // Cleared when a render completes, so it names only a render that never returned.
      __lingxiaSite: () => render.site || ""
    });
    if (catalog !== void 0) globalThis.__lingxiaActionCatalog = catalog;
  }

  // <stdin>
  Object.assign(globalThis, internal_exports, { __lingxiaPublish: publishHost });
})();
