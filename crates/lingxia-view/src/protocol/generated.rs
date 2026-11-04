// Generated from contracts/view/schema.json; do not edit.
//! Wire ids are append-only; native validation stays independent of the JS runtime.

pub const UI_ABI: u32 = 1;

/// Wire tag ids.
pub mod tag {
    pub const COLUMN: usize = 0;
    pub const ROW: usize = 1;
    pub const TEXT: usize = 2;
    pub const BUTTON: usize = 3;
    pub const SPACER: usize = 4;
    pub const SCROLL: usize = 5;
    pub const LIST: usize = 6;
    pub const IMAGE: usize = 7;
    pub const INPUT: usize = 8;
    pub const TEXTAREA: usize = 9;
    pub const CHECKBOX: usize = 10;
    pub const SWITCH: usize = 11;
    pub const RADIO: usize = 12;
    pub const RADIO_OPTION: usize = 13;
    pub const SEARCH: usize = 14;
    pub const SLIDER: usize = 15;
    pub const GRID: usize = 16;
    pub const STACK: usize = 17;
    pub const PLOT: usize = 18;
    pub const VECTOR: usize = 19;
    pub const PAGE: usize = 20;
    pub const SPAN: usize = 21;
    pub const LINK: usize = 22;
    pub const SELECT: usize = 23;
}

/// Wire field ids.
pub mod field {
    pub const WIDTH: usize = 0;
    pub const HEIGHT: usize = 1;
    pub const GAP: usize = 2;
    pub const PADDING: usize = 3;
    pub const FONT_SIZE: usize = 4;
    pub const COLOR: usize = 5;
    pub const BACKGROUND: usize = 6;
    pub const GROW: usize = 7;
    pub const ON_CLICK: usize = 8;
    pub const ENABLED: usize = 9;
    pub const RADIUS: usize = 10;
    pub const LIST_ID: usize = 11;
    pub const LIST_COUNT: usize = 12;
    pub const LIST_LEADING: usize = 13;
    pub const LIST_TRAILING: usize = 14;
    pub const LIST_ROW_INDEX: usize = 15;
    pub const LIST_ANCHOR: usize = 16;
    pub const ON_REACH_START: usize = 17;
    pub const ON_REACH_END: usize = 18;
    pub const IMAGE_SRC: usize = 19;
    pub const IMAGE_FIT: usize = 20;
    pub const INPUT_VALUE: usize = 21;
    pub const PLACEHOLDER: usize = 22;
    pub const SECURE: usize = 23;
    pub const KEYBOARD: usize = 24;
    pub const AUTO_FOCUS: usize = 25;
    pub const ON_INPUT: usize = 26;
    pub const ON_SUBMIT: usize = 27;
    pub const MIN_WIDTH: usize = 28;
    pub const MIN_HEIGHT: usize = 29;
    pub const MAX_WIDTH: usize = 30;
    pub const MAX_HEIGHT: usize = 31;
    pub const SHRINK: usize = 32;
    pub const ALIGN: usize = 33;
    pub const JUSTIFY: usize = 34;
    pub const ALIGN_SELF: usize = 35;
    pub const FONT_WEIGHT: usize = 36;
    pub const LINE_HEIGHT: usize = 37;
    pub const FONT_FAMILY: usize = 38;
    pub const CHECKED: usize = 39;
    pub const ON_CHECKED_CHANGE: usize = 40;
    pub const ACCESSIBILITY_LABEL: usize = 41;
    pub const SLIDER_VALUE: usize = 42;
    pub const SLIDER_MIN: usize = 43;
    pub const SLIDER_MAX: usize = 44;
    pub const SLIDER_STEP: usize = 45;
    pub const ON_SLIDER_CHANGE: usize = 46;
    pub const GRID_COLUMNS: usize = 47;
    pub const OFFSET_X: usize = 48;
    pub const OFFSET_Y: usize = 49;
    pub const TEXT_LINES: usize = 50;
    pub const PLOT_VALUES: usize = 51;
    pub const PLOT_KIND: usize = 52;
    pub const PLOT_COLORS: usize = 53;
    pub const PLOT_SMOOTH: usize = 54;
    pub const PLOT_AREA: usize = 55;
    pub const ON_PLOT_SELECT: usize = 56;
    pub const PLOT_SELECTED_INDEX: usize = 57;
    pub const OPACITY: usize = 58;
    pub const PADDING_TOP: usize = 59;
    pub const PADDING_RIGHT: usize = 60;
    pub const PADDING_BOTTOM: usize = 61;
    pub const PADDING_LEFT: usize = 62;
    pub const WORD_BREAK: usize = 63;
    pub const OVERFLOW_WRAP: usize = 64;
    pub const BORDER_WIDTH: usize = 65;
    pub const BORDER_COLOR: usize = 66;
    pub const BORDER_TOP_WIDTH: usize = 67;
    pub const BORDER_RIGHT_WIDTH: usize = 68;
    pub const BORDER_BOTTOM_WIDTH: usize = 69;
    pub const BORDER_LEFT_WIDTH: usize = 70;
    pub const TEXT_ALIGN: usize = 71;
    pub const BACKGROUND_GRADIENT: usize = 72;
    pub const BOX_SHADOW: usize = 73;
    pub const WIDTH_PERCENT: usize = 74;
    pub const HEIGHT_PERCENT: usize = 75;
    pub const MIN_WIDTH_PERCENT: usize = 76;
    pub const MIN_HEIGHT_PERCENT: usize = 77;
    pub const MAX_WIDTH_PERCENT: usize = 78;
    pub const MAX_HEIGHT_PERCENT: usize = 79;
    pub const VECTOR_SOURCE: usize = 80;
    pub const TEXT_DECORATION_LINE: usize = 81;
    pub const HEIGHT_AUTO: usize = 82;
    pub const SUBMIT_ON_ENTER: usize = 83;
    pub const FONT_STYLE: usize = 84;
    pub const PLOT_DOMAIN_MIN: usize = 85;
    pub const PLOT_DOMAIN_MAX: usize = 86;
    pub const FLEX_WRAP: usize = 87;
    pub const PLOT_PIE_LAYOUT: usize = 88;
    pub const LETTER_SPACING: usize = 89;
    pub const RADIUS_TOP_LEFT: usize = 90;
    pub const RADIUS_TOP_RIGHT: usize = 91;
    pub const RADIUS_BOTTOM_RIGHT: usize = 92;
    pub const RADIUS_BOTTOM_LEFT: usize = 93;
    pub const PLOT_MARKER_EMPHASIS_SCALE: usize = 94;
    pub const BACKDROP_BLUR: usize = 95;
    pub const BACKDROP_SATURATION: usize = 96;
    pub const PLACEHOLDER_COLOR: usize = 97;
    pub const FOCUS_STYLE: usize = 98;
    pub const FLEX_BASIS: usize = 99;
    pub const SCROLL_AXIS: usize = 100;
    pub const ANIMATION: usize = 101;
    pub const LIST_ITEM_KEYS: usize = 102;
    pub const FLEX_DIRECTION: usize = 103;
    pub const DISPLAY: usize = 104;
    pub const IMAGE_ALT: usize = 105;
    pub const IMAGE_RETRY_KEY: usize = 106;
    pub const ON_IMAGE_LOAD: usize = 107;
    pub const ON_IMAGE_ERROR: usize = 108;
    pub const MARGIN_TOP: usize = 109;
    pub const MARGIN_RIGHT: usize = 110;
    pub const MARGIN_BOTTOM: usize = 111;
    pub const MARGIN_LEFT: usize = 112;
    pub const POSITION: usize = 113;
    pub const INSET_TOP: usize = 114;
    pub const INSET_RIGHT: usize = 115;
    pub const INSET_BOTTOM: usize = 116;
    pub const INSET_LEFT: usize = 117;
    pub const Z_INDEX: usize = 118;
    pub const OVERFLOW: usize = 119;
    pub const ROW_GAP: usize = 120;
    pub const COLUMN_GAP: usize = 121;
    pub const TEXT_TRANSFORM: usize = 122;
    pub const WHITE_SPACE: usize = 123;
    pub const TEXT_OVERFLOW: usize = 124;
    pub const PRESSED_STYLE: usize = 125;
    pub const HOVER_STYLE: usize = 126;
    pub const WIDTH_KEYWORD: usize = 127;
    pub const HEIGHT_KEYWORD: usize = 128;
    pub const BORDER_STYLE: usize = 129;
    pub const TRANSFORM: usize = 130;
    pub const FONT_VARIANT_NUMERIC: usize = 131;
    pub const OVERSCROLL: usize = 132;
    pub const ASPECT_RATIO: usize = 133;
    pub const MAX_WIDTH_CH: usize = 134;
    pub const GRID_TEMPLATE_COLUMNS: usize = 135;
    pub const GRID_TEMPLATE_ROWS: usize = 136;
    pub const GRID_AUTO_COLUMNS: usize = 137;
    pub const GRID_AUTO_ROWS: usize = 138;
    pub const GRID_AUTO_FLOW: usize = 139;
    pub const GRID_COLUMN: usize = 140;
    pub const GRID_ROW: usize = 141;
    pub const JUSTIFY_ITEMS: usize = 142;
    pub const JUSTIFY_SELF: usize = 143;
    pub const ALIGN_CONTENT: usize = 144;
    pub const ORDER: usize = 145;
    pub const DIRECTION: usize = 146;
    pub const MARGIN_INLINE_START: usize = 147;
    pub const MARGIN_INLINE_END: usize = 148;
    pub const PADDING_INLINE_START: usize = 149;
    pub const PADDING_INLINE_END: usize = 150;
    pub const INSET_INLINE_START: usize = 151;
    pub const INSET_INLINE_END: usize = 152;
    pub const BORDER_INLINE_START_WIDTH: usize = 153;
    pub const BORDER_INLINE_END_WIDTH: usize = 154;
    pub const RADIUS_START_START: usize = 155;
    pub const RADIUS_START_END: usize = 156;
    pub const RADIUS_END_START: usize = 157;
    pub const RADIUS_END_END: usize = 158;
    pub const ROLE: usize = 159;
    pub const HEADING_LEVEL: usize = 160;
    pub const ACCESSIBILITY_HIDDEN: usize = 161;
    pub const ACCESSIBILITY_LIVE_REGION: usize = 162;
    pub const MAX_LENGTH: usize = 163;
    pub const AUTO_COMPLETE: usize = 164;
    pub const ENTER_KEY_HINT: usize = 165;
    pub const AUTO_CAPITALIZE: usize = 166;
    pub const SPELL_CHECK: usize = 167;
    pub const INPUT_MIN: usize = 168;
    pub const INPUT_MAX: usize = 169;
    pub const INPUT_STEP: usize = 170;
    pub const TRANSITION: usize = 171;
    pub const USER_SELECT: usize = 172;
    pub const FOCUS_REQUEST: usize = 173;
    pub const STATE_PRESSED: usize = 174;
    pub const STATE_SELECTED: usize = 175;
    pub const STATE_EXPANDED: usize = 176;
    pub const STATE_BUSY: usize = 177;
    pub const SELECT_OPTIONS: usize = 178;
    pub const ON_SELECT: usize = 179;
    pub const ROWS: usize = 180;
    pub const MAX_ROWS: usize = 181;
    pub const IMAGE_PLACEHOLDER: usize = 182;
    pub const IMAGE_POSITION: usize = 183;
    pub const CHILD_SLOT: usize = 184;
    pub const STARTING_STYLE: usize = 185;
    pub const EXIT_STYLE: usize = 186;
    pub const FOCUS_WITHIN_STYLE: usize = 187;
    pub const DATA_ATTRIBUTES: usize = 188;
    pub const BACKGROUND_CLIP: usize = 189;
    pub const MASK_IMAGE: usize = 190;
    pub const FILTER_BLUR: usize = 191;
    pub const GROUP_PRESSED_STYLE: usize = 192;
    pub const ANCHOR_TO: usize = 193;
    pub const SCROLL_SNAP: usize = 194;
    pub const SNAP_ALIGN: usize = 195;
    pub const TOP_LAYER: usize = 196;
    pub const ANCHOR_AREA: usize = 197;
    pub const ANCHOR_NAME: usize = 198;
    pub const DRAG: usize = 199;
    pub const ON_DRAG_EVENT: usize = 200;
    pub const DRAG_SCRIM: usize = 201;
}

pub const TAGS: &[&str] = &["column", "row", "text", "button", "spacer", "scroll", "list", "image", "input", "textarea", "checkbox", "switch", "radio", "radio-option", "search", "slider", "grid", "stack", "plot", "vector", "page", "span", "link", "select"];

/// Wire name and value kind, indexed by field id.
pub const FIELDS: &[(&str, &str)] = &[
    ("width", "number"),
    ("height", "number"),
    ("gap", "number"),
    ("padding", "number"),
    ("fontSize", "number"),
    ("color", "string"),
    ("background", "string"),
    ("grow", "number"),
    ("onClick", "handle"),
    ("enabled", "flag"),
    ("radius", "number"),
    ("listId", "list-id"),
    ("listCount", "list-count"),
    ("leading", "list-extent"),
    ("trailing", "list-extent"),
    ("rowIndex", "list-index"),
    ("anchor", "list-anchor"),
    ("onReachStart", "reach-handle"),
    ("onReachEnd", "reach-handle"),
    ("src", "image-source"),
    ("fit", "image-fit"),
    ("value", "input-text"),
    ("placeholder", "input-text"),
    ("secure", "input-flag"),
    ("keyboard", "input-keyboard"),
    ("autoFocus", "input-flag"),
    ("onInput", "input-handle"),
    ("onSubmit", "input-handle"),
    ("minWidth", "number"),
    ("minHeight", "number"),
    ("maxWidth", "number"),
    ("maxHeight", "number"),
    ("shrink", "number"),
    ("align", "alignment"),
    ("justify", "justification"),
    ("alignSelf", "alignment"),
    ("fontWeight", "number"),
    ("lineHeight", "number"),
    ("fontFamily", "string"),
    ("checked", "check-flag"),
    ("onChange", "check-handle"),
    ("accessibilityLabel", "radio-label"),
    ("value", "slider-number"),
    ("min", "slider-number"),
    ("max", "slider-number"),
    ("step", "slider-number"),
    ("onChange", "slider-handle"),
    ("columns", "grid-columns"),
    ("offsetX", "reserved"),
    ("offsetY", "reserved"),
    ("lines", "text-lines"),
    ("values", "plot-values"),
    ("kind", "plot-kind"),
    ("colors", "plot-colors"),
    ("smooth", "plot-flag"),
    ("area", "plot-flag"),
    ("onSelect", "plot-handle"),
    ("selectedIndex", "plot-index"),
    ("opacity", "number"),
    ("paddingTop", "number"),
    ("paddingRight", "number"),
    ("paddingBottom", "number"),
    ("paddingLeft", "number"),
    ("wordBreak", "word-break"),
    ("overflowWrap", "overflow-wrap"),
    ("borderWidth", "number"),
    ("borderColor", "string"),
    ("borderTopWidth", "number"),
    ("borderRightWidth", "number"),
    ("borderBottomWidth", "number"),
    ("borderLeftWidth", "number"),
    ("textAlign", "text-alignment"),
    ("backgroundGradient", "gradient"),
    ("boxShadow", "shadow"),
    ("widthPercent", "dimension-percent"),
    ("heightPercent", "dimension-percent"),
    ("minWidthPercent", "dimension-percent"),
    ("minHeightPercent", "dimension-percent"),
    ("maxWidthPercent", "dimension-percent"),
    ("maxHeightPercent", "dimension-percent"),
    ("vector", "vector-source"),
    ("textDecorationLine", "text-decoration-line"),
    ("heightAuto", "dimension-auto"),
    ("submitOnEnter", "input-flag"),
    ("fontStyle", "font-style"),
    ("domainMin", "plot-domain"),
    ("domainMax", "plot-domain"),
    ("flexWrap", "flex-wrap"),
    ("pieLayout", "plot-layout"),
    ("letterSpacing", "number"),
    ("radiusTopLeft", "number"),
    ("radiusTopRight", "number"),
    ("radiusBottomRight", "number"),
    ("radiusBottomLeft", "number"),
    ("markerEmphasisScale", "plot-marker-scale"),
    ("backdropBlur", "number"),
    ("backdropSaturation", "number"),
    ("placeholderColor", "string"),
    ("focusStyle", "focus-style"),
    ("flexBasis", "flex-basis"),
    ("scrollAxis", "scroll-axis"),
    ("animation", "animation"),
    ("itemKeys", "list-keys"),
    ("flexDirection", "flex-direction"),
    ("display", "display"),
    ("alt", "image-alt"),
    ("retryKey", "image-retry"),
    ("onLoad", "image-handle"),
    ("onError", "image-handle"),
    ("marginTop", "margin"),
    ("marginRight", "margin"),
    ("marginBottom", "margin"),
    ("marginLeft", "margin"),
    ("position", "position"),
    ("insetTop", "inset"),
    ("insetRight", "inset"),
    ("insetBottom", "inset"),
    ("insetLeft", "inset"),
    ("zIndex", "z-index"),
    ("overflow", "overflow"),
    ("rowGap", "number"),
    ("columnGap", "number"),
    ("textTransform", "text-transform"),
    ("whiteSpace", "white-space"),
    ("textOverflow", "text-overflow"),
    ("pressedStyle", "state-style"),
    ("hoverStyle", "state-style"),
    ("widthKeyword", "sizing-keyword"),
    ("heightKeyword", "sizing-keyword"),
    ("borderStyle", "border-style"),
    ("transform", "transform"),
    ("fontVariantNumeric", "font-variant-numeric"),
    ("overscroll", "overscroll"),
    ("aspectRatio", "aspect-ratio"),
    ("maxWidthCh", "dimension-ch"),
    ("gridTemplateColumns", "grid-template"),
    ("gridTemplateRows", "grid-template"),
    ("gridAutoColumns", "grid-auto"),
    ("gridAutoRows", "grid-auto"),
    ("gridAutoFlow", "grid-auto-flow"),
    ("gridColumn", "grid-line"),
    ("gridRow", "grid-line"),
    ("justifyItems", "alignment"),
    ("justifySelf", "alignment"),
    ("alignContent", "align-content"),
    ("order", "order"),
    ("direction", "direction"),
    ("marginInlineStart", "margin"),
    ("marginInlineEnd", "margin"),
    ("paddingInlineStart", "number"),
    ("paddingInlineEnd", "number"),
    ("insetInlineStart", "inset"),
    ("insetInlineEnd", "inset"),
    ("borderInlineStartWidth", "number"),
    ("borderInlineEndWidth", "number"),
    ("radiusStartStart", "number"),
    ("radiusStartEnd", "number"),
    ("radiusEndStart", "number"),
    ("radiusEndEnd", "number"),
    ("role", "role"),
    ("headingLevel", "heading-level"),
    ("accessibilityHidden", "hidden-flag"),
    ("accessibilityLiveRegion", "live-region"),
    ("maxLength", "input-max-length"),
    ("autoComplete", "auto-complete"),
    ("enterKeyHint", "enter-key-hint"),
    ("autoCapitalize", "auto-capitalize"),
    ("spellCheck", "input-flag"),
    ("min", "input-bound"),
    ("max", "input-bound"),
    ("step", "input-step"),
    ("transition", "transition"),
    ("userSelect", "user-select"),
    ("focusRequest", "focus-request"),
    ("statePressed", "semantic-state"),
    ("stateSelected", "semantic-state"),
    ("stateExpanded", "semantic-state"),
    ("stateBusy", "semantic-state"),
    ("selectOptions", "select-options"),
    ("onSelect", "select-handle"),
    ("rows", "textarea-rows"),
    ("maxRows", "textarea-rows"),
    ("placeholderSrc", "image-src"),
    ("objectPosition", "image-position"),
    ("slot", "child-slot"),
    ("startingStyle", "state-style"),
    ("exitStyle", "state-style"),
    ("focusWithinStyle", "state-style"),
    ("dataAttributes", "string"),
    ("backgroundClip", "background-clip"),
    ("maskImage", "string"),
    ("filterBlur", "number"),
    ("groupPressedStyle", "state-style"),
    ("anchorTo", "anchor"),
    ("scrollSnap", "scroll-snap"),
    ("snapAlign", "snap-align"),
    ("topLayer", "flag"),
    ("anchorArea", "anchor-area"),
    ("anchorName", "anchor"),
    ("drag", "drag"),
    ("onDragEvent", "handle"),
    ("dragScrim", "flag"),
];

/// Enumerated values accepted for an option-valued field kind.
pub fn options(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "font-family" => Some(&["sans", "mono", "serif"]),
        "font-style" => Some(&["normal", "italic"]),
        "flex-wrap" => Some(&["nowrap", "wrap", "wrap-reverse"]),
        "flex-direction" => Some(&["row", "column", "row-reverse", "column-reverse"]),
        "display" => Some(&["flex", "grid", "none", "block", "inline-flex", "inline-block"]),
        "scroll-axis" => Some(&["horizontal", "vertical"]),
        "word-break" => Some(&["normal", "break-all", "keep-all"]),
        "overflow-wrap" => Some(&["normal", "break-word", "anywhere"]),
        "text-decoration-line" => Some(&["none", "line-through", "underline"]),
        "text-alignment" => Some(&["start", "end", "left", "center", "right", "justify"]),
        "alignment" => Some(&["start", "center", "end", "stretch", "baseline"]),
        "justification" => Some(&["start", "center", "end", "between", "around", "evenly"]),
        "position" => Some(&["relative", "absolute", "fixed", "sticky"]),
        "overflow" => Some(&["visible", "hidden"]),
        "text-transform" => Some(&["none", "uppercase", "lowercase", "capitalize"]),
        "background-clip" => Some(&["border-box", "text"]),
        "white-space" => Some(&["normal", "nowrap", "pre", "pre-wrap", "pre-line"]),
        "text-overflow" => Some(&["clip", "ellipsis"]),
        "sizing-keyword" => Some(&["fit-content", "max-content", "min-content"]),
        "border-style" => Some(&["solid", "dashed", "dotted"]),
        "overscroll" => Some(&["auto", "contain", "none"]),
        "scroll-snap" => Some(&["none", "x", "y"]),
        "snap-align" => Some(&["none", "start", "center", "end"]),
        "grid-auto-flow" => Some(&["row", "column", "row-dense", "column-dense"]),
        "align-content" => Some(&["start", "center", "end", "between", "around", "evenly", "stretch"]),
        "direction" => Some(&["ltr", "rtl", "auto"]),
        "role" => Some(&["heading", "list", "listitem", "dialog", "alert", "status", "grid", "gridcell", "listbox", "option", "menu", "menuitem"]),
        "anchor-area" => Some(&["bottom span-inline-end", "bottom span-inline-start", "bottom", "top span-inline-end", "top span-inline-start", "top"]),
        "live-region" => Some(&["off", "polite", "assertive"]),
        "input-keyboard" => Some(&["text", "number", "email", "phone", "search", "date", "time"]),
        "auto-complete" => Some(&["off", "on", "username", "current-password", "new-password", "one-time-code", "email", "tel", "name", "given-name", "family-name", "street-address", "postal-code", "country", "cc-number", "cc-exp", "cc-csc", "bday"]),
        "enter-key-hint" => Some(&["enter", "done", "go", "next", "previous", "search", "send"]),
        "auto-capitalize" => Some(&["none", "sentences", "words", "characters"]),
        "transition-property" => Some(&["opacity", "transform", "bg", "fg", "border", "shadow", "layout"]),
        "user-select" => Some(&["none", "text", "all"]),
        _ => None,
    }
}

/// Page-wide bounds both sides of the wire enforce.
pub mod limits {
    pub const NODES: usize = 4096;
    pub const DEPTH: usize = 128;
    pub const ANIMATION_BYTES: usize = 262144;
    pub const ITEM_KEY_BYTES: usize = 262144;
    pub const IMAGE_BYTES: usize = 8388608;
    pub const SOURCE_BYTES: usize = 8192;
    pub const VISIBLE_IMAGE_BYTES: usize = 67108864;
    pub const DECODED_IMAGE_BYTES: usize = 100663296;
    pub const RUNS: usize = 256;
    pub const TRANSITION_MS: usize = 10000;
    pub const EVAL_BUDGET_MS: usize = 100;
    pub const EVAL_BUDGET_DEBUG_MULTIPLIER: usize = 8;
    pub const EDITOR_TEXT_BYTES: usize = 65536;
    pub const INPUT_REQUEST_BYTES: usize = 524288;
    pub const INPUT_BATCH_EDITS: usize = 64;
    pub const CLIPBOARD_ITEMS: usize = 128;
    pub const WIRE_VALUES: usize = 100000;
    pub const TEXT_BYTES: usize = 65536;
    pub const IDENTIFIER_BYTES: usize = 256;
    pub const LIST_ROWS: usize = 100000;
    pub const GRID_TRACKS: usize = 64;
    pub const VECTOR_SOURCE_BYTES: usize = 32768;
    pub const VECTOR_PATH_BYTES: usize = 16384;
    pub const VECTOR_PATH_WORK: usize = 16384;
    pub const VECTOR_WORK: usize = 32768;
    pub const PLOT_WORK: usize = 16384;
    pub const SHADOW_MASK_BYTES: usize = 16777216;
    pub const COMPOSITE_FRAMES: usize = 6;
    pub const LEASE_FRAMES: usize = 6;
    pub const COMPOSITE_BYTES: usize = 268435456;
    pub const OUTLINE_GLYPHS: usize = 4096;
    pub const GHOSTS: usize = 64;
    pub const LAYOUT_MOTIONS: usize = 256;
    pub const DATA_ATTRIBUTE_BYTES: usize = 65536;
    pub const BLUR_LAYERS: usize = 8;
    pub const TOP_LAYER: usize = 16;
    pub const DRAG_SNAP_POINTS: usize = 8;
}

/// Environment keys and where each comes from: `device` facts, a host `push`, or the `session`.
pub const ENVIRONMENT_KEYS: &[(&str, &str)] = &[
    ("theme", "device"),
    ("viewport", "session"),
    ("insets", "device"),
    ("safeArea", "device"),
    ("keyboard", "device"),
    ("pointer", "device"),
    ("os", "device"),
    ("formFactor", "device"),
    ("sizeClass", "push"),
    ("displayLanguage", "device"),
    ("reducedMotion", "device"),
    ("highContrast", "device"),
    ("animatorDurationScale", "device"),
    ("hourCycle", "device"),
    ("firstWeekday", "device"),
    ("direction", "session"),
    ("canDock", "push"),
    ("renderer", "device"),
    ("textScale", "device"),
    ("palette", "push"),
];

/// Whether a native environment snapshot (device facts plus pushes) is well formed.
pub fn check_environment(value: &serde_json::Value) -> bool {
    value.is_object()
        && matches!(value["theme"].as_str(), Some("light") | Some("dark"))
        && ["top", "right", "bottom", "left"].iter().all(|edge| value["insets"][edge].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n)))
        && (value.get("safeArea").is_none() || ["top", "right", "bottom", "left"].iter().all(|edge| value["safeArea"][edge].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))))
        && value["keyboard"]["height"].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))
        && matches!(value["pointer"].as_str(), Some("fine") | Some("coarse"))
        && value["os"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 128)
        && value["formFactor"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 128)
        && (value.get("sizeClass").is_none() || matches!(value["sizeClass"].as_str(), Some("compact") | Some("regular")))
        && value["displayLanguage"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 128)
        && (value.get("reducedMotion").is_none() || value["reducedMotion"].is_boolean())
        && (value.get("highContrast").is_none() || value["highContrast"].is_boolean())
        && (value.get("animatorDurationScale").is_none() || value["animatorDurationScale"].as_f64().is_some_and(|n| n.is_finite() && (0.0..=10.0).contains(&n)))
        && (value.get("hourCycle").is_none() || matches!(value["hourCycle"].as_str(), Some("h12") | Some("h23")))
        && (value.get("firstWeekday").is_none() || value["firstWeekday"].as_f64().is_some_and(|n| n.is_finite() && (1.0..=7.0).contains(&n)))
        && value["canDock"].is_boolean()
        && (value.get("renderer").is_none() || (matches!(value["renderer"]["fallback"].as_str(), None | Some("driver")) && value["renderer"].as_object().is_some_and(|r| r.len() == 1)))
}

/// What a wire tag may contain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Children {
    /// A leaf.
    None,
    /// A leaf carrying inline text.
    Text,
    /// Inline text, or noninteractive content descendants.
    Content,
    Any,
    /// Virtual list row wrappers.
    Rows,
    /// Only tags whose admission names this tag as parent.
    Options,
    /// Inline text, or only run tags whose admission names this tag as parent.
    Runs,
}

#[derive(Clone, Copy, Debug)]
pub struct TagAdmission {
    pub children: Children,
    /// May sit inside a `Children::Content` control.
    pub content: bool,
    pub parent: Option<usize>,
    pub root: bool,
    /// Requires a non-empty key.
    pub keyed: bool,
    pub requires: &'static [usize],
}

/// Indexed by tag id.
pub const TAG_ADMISSION: &[TagAdmission] = &[
    TagAdmission { children: Children::Any, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Any, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Runs, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Content, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Any, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Rows, content: false, parent: None, root: false, keyed: false, requires: &[field::LIST_ID, field::LIST_COUNT, field::LIST_LEADING, field::LIST_TRAILING] },
    TagAdmission { children: Children::None, content: true, parent: None, root: false, keyed: false, requires: &[field::IMAGE_SRC] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Content, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Options, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: false, parent: Some(tag::RADIO), root: false, keyed: true, requires: &[field::CHECKED] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Any, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Any, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Any, content: false, parent: None, root: true, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: true, parent: Some(tag::TEXT), root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: false, parent: Some(tag::TEXT), root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: false, parent: None, root: false, keyed: false, requires: &[] },
];

/// The tags a field applies to (`None`: every tag) and the parent tag it requires.
#[derive(Clone, Copy, Debug)]
pub struct FieldAdmission {
    pub tags: Option<&'static [usize]>,
    pub parent: Option<usize>,
}

/// Indexed by field id.
pub const FIELD_ADMISSION: &[FieldAdmission] = &[
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::LINK]), parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::INPUT, tag::TEXTAREA, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::RADIO_OPTION, tag::SEARCH, tag::SLIDER, tag::SELECT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: Some(tag::LIST) },
    FieldAdmission { tags: Some(&[tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL, tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL, tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::IMAGE]), parent: None },
    FieldAdmission { tags: Some(&[tag::IMAGE]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH, tag::SELECT]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH, tag::SELECT]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::CHECKBOX, tag::SWITCH, tag::RADIO_OPTION]), parent: None },
    FieldAdmission { tags: Some(&[tag::CHECKBOX, tag::SWITCH, tag::RADIO_OPTION]), parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::INPUT, tag::TEXTAREA, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::SEARCH, tag::SLIDER, tag::PLOT, tag::LINK, tag::COLUMN, tag::ROW, tag::PAGE, tag::TEXT, tag::SELECT]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::GRID]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::TEXT]), parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::VECTOR]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::TEXTAREA]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::PLOT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH, tag::SELECT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::ROW, tag::PAGE, tag::BUTTON]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::IMAGE]), parent: None },
    FieldAdmission { tags: Some(&[tag::IMAGE]), parent: None },
    FieldAdmission { tags: Some(&[tag::IMAGE]), parent: None },
    FieldAdmission { tags: Some(&[tag::IMAGE]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::LINK]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL, tag::LIST]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::SELECT]), parent: None },
    FieldAdmission { tags: Some(&[tag::SELECT]), parent: None },
    FieldAdmission { tags: Some(&[tag::TEXTAREA]), parent: None },
    FieldAdmission { tags: Some(&[tag::TEXTAREA]), parent: None },
    FieldAdmission { tags: Some(&[tag::IMAGE]), parent: None },
    FieldAdmission { tags: Some(&[tag::IMAGE]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::ROW, tag::STACK, tag::GRID]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL, tag::LIST]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::ROW, tag::STACK, tag::GRID]), parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::ROW, tag::STACK, tag::GRID, tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::ROW, tag::STACK, tag::GRID, tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON]), parent: None },
];
