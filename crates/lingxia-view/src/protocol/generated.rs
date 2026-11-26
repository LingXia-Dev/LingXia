// Generated from contracts/view/schema.json; do not edit.
//! Wire ids are append-only; native validation stays independent of the JS runtime.

pub const UI_ABI: u32 = 1;

/// Wire tag ids.
pub mod tag {
    pub const COLUMN: usize = 0;
    pub const RESERVED_1: usize = 1;
    pub const TEXT: usize = 2;
    pub const BUTTON: usize = 3;
    pub const RESERVED_4: usize = 4;
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
    pub const LINK_BOX: usize = 16;
    pub const RESERVED_17: usize = 17;
    pub const PLOT: usize = 18;
    pub const VECTOR: usize = 19;
    pub const PAGE: usize = 20;
    pub const SPAN: usize = 21;
    pub const LINK: usize = 22;
    pub const SELECT: usize = 23;
    pub const VIDEO: usize = 24;
    pub const WEBVIEW: usize = 25;
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
    pub const ELEMENT_ID: usize = 202;
    pub const ARIA_ACTIONS: usize = 203;
    pub const SNAP_STOP: usize = 204;
    pub const SNAP_INDEX: usize = 205;
    pub const ON_SNAP_CHANGE: usize = 206;
    pub const VIDEO_SRC: usize = 207;
    pub const VIDEO_PLAYER: usize = 208;
    pub const VIDEO_POSTER: usize = 209;
    pub const VIDEO_PAUSED: usize = 210;
    pub const VIDEO_MUTED: usize = 211;
    pub const VIDEO_LOOP: usize = 212;
    pub const VIDEO_RATE: usize = 213;
    pub const VIDEO_VOLUME: usize = 214;
    pub const ON_VIDEO_EVENT: usize = 215;
    pub const VIDEO_SOURCES: usize = 216;
    pub const ARIA_VALUE_NOW: usize = 217;
    pub const ARIA_VALUE_MIN: usize = 218;
    pub const ARIA_VALUE_MAX: usize = 219;
    pub const ARIA_VALUE_TEXT: usize = 220;
    pub const STATE_INVALID: usize = 221;
    pub const STATE_REQUIRED: usize = 222;
    pub const ARIA_DESCRIBED_BY: usize = 223;
    pub const STATE_CHECKED: usize = 224;
    pub const FOCUS_VISIBLE_STYLE: usize = 225;
    pub const TRACK_STYLE: usize = 226;
    pub const RANGE_STYLE: usize = 227;
    pub const SECONDARY_STYLE: usize = 228;
    pub const THUMB_STYLE: usize = 229;
    pub const SLIDER_SECONDARY: usize = 230;
    pub const INTEREST_FOR: usize = 231;
    pub const ON_POINTER_ENTER: usize = 232;
    pub const ON_POINTER_LEAVE: usize = 233;
    pub const RING: usize = 234;
    pub const HINT: usize = 235;
    pub const ON_SLIDER_PRESS: usize = 236;
    pub const CLOSE_AFTER: usize = 237;
    pub const TOUCH_TARGET: usize = 238;
    pub const MIN_WIDTH_CH: usize = 239;
    pub const WIDTH_CH: usize = 240;
    pub const DISPLAY_VALUE: usize = 241;
    pub const SIZE_TEXT: usize = 242;
    pub const ALIGN_SAFE: usize = 243;
    pub const ON_LAYOUT: usize = 244;
    pub const ON_POINTER: usize = 245;
    pub const POINTER_AXIS: usize = 246;
    pub const ON_FOCUS: usize = 247;
    pub const ON_BLUR: usize = 248;
    pub const READ_ONLY: usize = 249;
    pub const KEY_MAP: usize = 250;
    pub const ON_KEY_DOWN: usize = 251;
    pub const ARIA_ACTIVE_DESCENDANT: usize = 252;
    pub const MIXED: usize = 253;
    pub const ON_CONTEXT_MENU: usize = 254;
    pub const ON_PAGE: usize = 255;
    pub const TEXT_DECORATION_STYLE: usize = 256;
    pub const TEXT_DECORATION_COLOR: usize = 257;
    pub const SLIDER_OTHER: usize = 258;
    pub const SLIDER_THUMB: usize = 259;
    pub const WEB_SRC: usize = 260;
    pub const WEB_SESSION: usize = 261;
    pub const ON_WEB_EVENT: usize = 262;
    pub const WEB_HANDLES_DRAGS: usize = 263;
    pub const ON_SCROLL_EDGE: usize = 264;
    pub const ARIA_CONTROLS: usize = 265;
    pub const ARIA_AUTOCOMPLETE: usize = 266;
    pub const SCALE_REFS: usize = 267;
    pub const MIN_WIDTH_KEYWORD: usize = 268;
    pub const MIN_HEIGHT_KEYWORD: usize = 269;
    pub const MAX_WIDTH_KEYWORD: usize = 270;
    pub const MAX_HEIGHT_KEYWORD: usize = 271;
    pub const DEFAULT_VALUE: usize = 272;
    pub const CHECKED_STYLE: usize = 273;
    pub const HAS_CHECKED_STYLE: usize = 274;
    pub const MODAL: usize = 275;
    pub const SIDE_TOP_STYLE: usize = 276;
    pub const SIDE_BOTTOM_STYLE: usize = 277;
    pub const SIDE_START_STYLE: usize = 278;
    pub const SIDE_END_STYLE: usize = 279;
    pub const BLEED: usize = 280;
    pub const PAGE_TITLE: usize = 281;
    pub const PICKER_REQUEST: usize = 282;
    pub const ARIA_LEVEL: usize = 283;
    pub const ARIA_CURRENT: usize = 284;
    pub const KEEPS_FOCUS: usize = 285;
    pub const SELECTION_REQUEST: usize = 286;
    pub const ON_SELECTION_CHANGE: usize = 287;
    pub const ON_VISIBLE_CHANGE: usize = 288;
    pub const ON_PASTE: usize = 289;
}

pub const TAGS: &[&str] = &["column", "reserved-1", "text", "button", "reserved-4", "scroll", "list", "image", "input", "textarea", "checkbox", "switch", "radio", "radio-option", "search", "slider", "link-box", "reserved-17", "plot", "vector", "page", "span", "link", "select", "video", "webview"];

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
    ("columns", "reserved"),
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
    ("elementId", "element-id"),
    ("ariaActions", "element-ids"),
    ("snapStop", "snap-stop"),
    ("snapIndex", "snap-index"),
    ("onSnapChange", "handle"),
    ("src", "video-source"),
    ("player", "video-player"),
    ("poster", "image-src"),
    ("paused", "video-flag"),
    ("muted", "video-flag"),
    ("loop", "video-flag"),
    ("playbackRate", "video-number"),
    ("volume", "video-number"),
    ("onVideoEvent", "handle"),
    ("sources", "video-sources"),
    ("ariaValueNow", "number"),
    ("ariaValueMin", "number"),
    ("ariaValueMax", "number"),
    ("ariaValueText", "string"),
    ("stateInvalid", "semantic-state"),
    ("stateRequired", "semantic-state"),
    ("ariaDescribedBy", "element-ids"),
    ("stateChecked", "semantic-state"),
    ("focusVisibleStyle", "state-style"),
    ("trackStyle", "part-style"),
    ("rangeStyle", "part-style"),
    ("secondaryStyle", "part-style"),
    ("thumbStyle", "part-style"),
    ("secondaryValue", "slider-number"),
    ("interestFor", "element-id"),
    ("onPointerEnter", "handle"),
    ("onPointerLeave", "handle"),
    ("ring", "ring"),
    ("hint", "flag"),
    ("onPressChange", "handle"),
    ("closeAfter", "close-after"),
    ("touchTarget", "touch-target"),
    ("minWidthCh", "dimension-ch"),
    ("widthCh", "dimension-ch"),
    ("displayValue", "input-text"),
    ("sizeText", "input-text"),
    ("alignSafe", "align-safe"),
    ("onLayout", "handle"),
    ("onPointer", "handle"),
    ("pointerAxis", "pointer-axis"),
    ("onFocus", "handle"),
    ("onBlur", "handle"),
    ("readOnly", "input-flag"),
    ("keyMap", "key-map"),
    ("onKeyDown", "input-handle"),
    ("ariaActiveDescendant", "element-id"),
    ("mixed", "flag"),
    ("onContextMenu", "handle"),
    ("onPage", "handle"),
    ("textDecorationStyle", "border-style"),
    ("textDecorationColor", "string"),
    ("sliderOther", "slider-number"),
    ("sliderThumb", "slider-thumb"),
    ("src", "web-source"),
    ("session", "web-session"),
    ("onWebEvent", "handle"),
    ("handlesDrags", "flag"),
    ("onScrollEdge", "handle"),
    ("ariaControls", "element-ids"),
    ("ariaAutocomplete", "auto-complete-mode"),
    ("scaleRefs", "flag"),
    ("minWidthKeyword", "sizing-keyword"),
    ("minHeightKeyword", "sizing-keyword"),
    ("maxWidthKeyword", "sizing-keyword"),
    ("maxHeightKeyword", "sizing-keyword"),
    ("defaultValue", "default-value"),
    ("checkedStyle", "state-style"),
    ("hasCheckedStyle", "state-style"),
    ("modal", "flag"),
    ("sideTopStyle", "state-style"),
    ("sideBottomStyle", "state-style"),
    ("sideStartStyle", "state-style"),
    ("sideEndStyle", "state-style"),
    ("bleed", "bleed"),
    ("title", "string"),
    ("pickerRequest", "picker-request"),
    ("ariaLevel", "aria-level"),
    ("ariaCurrent", "aria-current"),
    ("keepsFocus", "flag"),
    ("selectionRequest", "selection-request"),
    ("onSelectionChange", "handle"),
    ("onVisibleChange", "handle"),
    ("onPaste", "handle"),
];

/// Enumerated values accepted for an option-valued field kind.
pub fn options(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "snap-stop" => Some(&["normal", "always"]),
        "aria-current" => Some(&["page", "step", "true"]),
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
        "slider-thumb" => Some(&["lower", "upper"]),
        "overscroll" => Some(&["auto", "contain", "none"]),
        "scroll-snap" => Some(&["none", "x", "y"]),
        "snap-align" => Some(&["none", "start", "center", "end"]),
        "grid-auto-flow" => Some(&["row", "column", "row-dense", "column-dense"]),
        "align-content" => Some(&["start", "center", "end", "between", "around", "evenly", "stretch"]),
        "direction" => Some(&["ltr", "rtl", "auto"]),
        "role" => Some(&["heading", "list", "listitem", "dialog", "alert", "status", "grid", "gridcell", "listbox", "option", "menu", "menuitem", "progressbar", "alertdialog", "group", "tooltip", "tablist", "tab", "tabpanel", "radiogroup", "radio", "toolbar", "tree", "treeitem", "row", "menuitemcheckbox", "menuitemradio", "img", "combobox", "spinbutton"]),
        "anchor-area" => Some(&["bottom span-inline-end", "bottom span-inline-start", "bottom", "top span-inline-end", "top span-inline-start", "top", "inline-end span-block-end", "inline-end span-block-start", "inline-end", "inline-start span-block-end", "inline-start span-block-start", "inline-start"]),
        "live-region" => Some(&["off", "polite", "assertive"]),
        "input-keyboard" => Some(&["text", "number", "email", "phone", "search", "date", "time", "decimal"]),
        "auto-complete-mode" => Some(&["none", "list"]),
        "auto-complete" => Some(&["off", "on", "username", "current-password", "new-password", "one-time-code", "email", "tel", "name", "given-name", "family-name", "street-address", "postal-code", "country", "cc-number", "cc-exp", "cc-csc", "bday"]),
        "enter-key-hint" => Some(&["enter", "done", "go", "next", "previous", "search", "send"]),
        "auto-capitalize" => Some(&["none", "sentences", "words", "characters"]),
        "transition-property" => Some(&["opacity", "transform", "bg", "fg", "border", "shadow", "layout", "size", "d"]),
        "user-select" => Some(&["none", "text", "all"]),
        "touch-target" => Some(&["auto", "none"]),
        "pointer-axis" => Some(&["x", "y", "both"]),
        "bleed" => Some(&["all", "top", "bottom", "horizontal"]),
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
    pub const TRANSITION_MS: usize = 10000;
    pub const EVAL_BUDGET_MS: usize = 100;
    pub const EVAL_BUDGET_DEBUG_MULTIPLIER: usize = 8;
    pub const EVAL_WARMUP_MULTIPLIER: usize = 2;
    pub const EVAL_LIMIT_MULTIPLIER: usize = 4;
    pub const EVAL_OVERRUN_FRAMES: usize = 3;
    pub const EDITOR_TEXT_BYTES: usize = 65536;
    pub const INPUT_REQUEST_BYTES: usize = 524288;
    pub const PASTE_HTML_BYTES: usize = 2097152;
    pub const PASTE_REQUEST_BYTES: usize = 6291456;
    pub const INPUT_BATCH_EDITS: usize = 64;
    pub const CLIPBOARD_ITEMS: usize = 128;
    pub const WIRE_VALUES: usize = 100000;
    pub const TEXT_BYTES: usize = 65536;
    pub const IDENTIFIER_BYTES: usize = 256;
    pub const LIST_ROWS: usize = 100000;
    pub const GRID_TRACKS: usize = 64;
    pub const VECTOR_SOURCE_BYTES: usize = 32768;
    pub const VECTOR_PATH_BYTES: usize = 16384;
    pub const VECTOR_PATH_SEGMENTS: usize = 1024;
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
    pub const WEB_MESSAGE_BYTES: usize = 65536;
    pub const WEB_MESSAGE_QUEUE: usize = 64;
    pub const STYLE_NUMBER: usize = 16384;
    pub const LINE_HEIGHT_MULTIPLIER: usize = 4;
}

/// Environment keys and where each comes from: `device` facts, a host `push`, or the `session`.
pub const ENVIRONMENT_KEYS: &[(&str, &str)] = &[
    ("appearance", "device"),
    ("viewport", "session"),
    ("insets", "device"),
    ("safeArea", "device"),
    ("hinge", "device"),
    ("keyboard", "device"),
    ("capsule", "push"),
    ("pointer", "device"),
    ("os", "device"),
    ("sizeClass", "push"),
    ("language", "device"),
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
    ("scales", "push"),
];

/// Whether a native environment snapshot (device facts plus pushes) is well formed.
pub fn check_environment(value: &serde_json::Value) -> bool {
    value.is_object()
        && matches!(value["appearance"].as_str(), Some("light") | Some("dark"))
        && ["top", "right", "bottom", "left"].iter().all(|edge| value["insets"][edge].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n)))
        && (value.get("safeArea").is_none() || ["top", "right", "bottom", "left"].iter().all(|edge| value["safeArea"][edge].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))))
        && (value.get("hinge").is_none() || (value["hinge"].is_null() || (value["hinge"].is_object() && ["x", "y", "width", "height"].iter().all(|k| value["hinge"][k].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))) && matches!(value["hinge"]["orientation"].as_str(), Some("vertical" | "horizontal")) && matches!(value["hinge"]["posture"].as_str(), Some("flat" | "half-opened")))))
        && value["keyboard"]["height"].as_f64().is_some_and(|n| n.is_finite() && (0.0..=16384.0).contains(&n))
        && (value.get("capsule").is_none() || (value["capsule"].is_null() || (["x", "y", "width", "height"].iter().all(|side| value["capsule"][side].as_f64().is_some_and(|n| n.is_finite() && (-16384.0..=16384.0).contains(&n))) && ["width", "height"].iter().all(|side| value["capsule"][side].as_f64().is_some_and(|n| n >= 0.0)))))
        && matches!(value["pointer"].as_str(), Some("fine") | Some("coarse"))
        && matches!(value["os"].as_str(), Some("ios") | Some("macos") | Some("android") | Some("windows") | Some("harmony") | Some("unknown"))
        && (value.get("sizeClass").is_none() || matches!(value["sizeClass"].as_str(), Some("compact") | Some("regular")))
        && value["language"].as_str().is_some_and(|s| !s.is_empty() && s.len() <= 128)
        && (value.get("reducedMotion").is_none() || value["reducedMotion"].is_boolean())
        && (value.get("highContrast").is_none() || value["highContrast"].is_boolean())
        && (value.get("animatorDurationScale").is_none() || value["animatorDurationScale"].as_f64().is_some_and(|n| n.is_finite() && (0.0..=10.0).contains(&n)))
        && (value.get("hourCycle").is_none() || matches!(value["hourCycle"].as_str(), Some("h12") | Some("h23")))
        && (value.get("firstWeekday").is_none() || value["firstWeekday"].as_f64().is_some_and(|n| n.is_finite() && (1.0..=7.0).contains(&n)))
        && value["canDock"].is_boolean()
        && (value.get("renderer").is_none() || (matches!(value["renderer"]["fallback"].as_str(), None | Some("driver")) && value["renderer"].as_object().is_some_and(|r| r.len() == 1)))
}

/// The native environment snapshot (device facts plus host pushes) as `view.env` carries it.
pub mod env {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Environment {
        /// The effective appearance: the lxapp's fixed one, else the user's preference, else the product default, else the system.
        pub appearance: Appearance,
        /// System bars and host chrome over the surface, logical pixels.
        pub insets: Edges,
        /// The system bars alone (status bar, cutout, home indicator), as CSS `env(safe-area-inset-*)`; a dialog covering host chrome pads by these, logical pixels.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub safe_area: Option<Edges>,
        /// Physical display hinge/crease intersecting this page canvas in local logical pixels; null when absent.
        #[serde(default)]
        pub hinge: Option<Hinge>,
        pub keyboard: Keyboard,
        /// The host's capsule (the ··· and close control) where it floats over the page, in page coordinates, logical pixels; `null` when the host shows none or it sits in a nav bar. The same box Web pages read as `--lx-page-chrome-capsule-*`.
        #[serde(default)]
        pub capsule: Option<Capsule>,
        /// `fine` pointers (mouse, trackpad) hover; `coarse` ones (touch) do not.
        #[serde(default)]
        pub pointer: Pointer,
        /// The host OS, the same value as Logic's `lx.host.info.os` and `lx.device.info.osName`.
        pub os: Os,
        /// `compact` below 600 logical pixels of page width, else `regular`, with 24 pixels of hysteresis: compact holds until 624, regular until below 576. Near the edge it can disagree with `min-[600px]:`; use `sizeClass` for structure and `min-[…]:` for spacing and type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub size_class: Option<SizeClass>,
        /// The language the page's text is in: the host's display language narrowed to the `languages` its lxapp.json lists. `t`, built-in words, direction and glyph forms follow it.
        pub language: String,
        /// The system asks for reduced motion; `motion-reduce:`/`motion-safe:` follow it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reduced_motion: Option<bool>,
        /// The system asks for increased contrast; the palette resolves its high-contrast variant.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub high_contrast: Option<bool>,
        /// The platform's animation duration multiplier (0..10, default 1); 0 also sets `reducedMotion`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub animator_duration_scale: Option<f64>,
        /// 12- or 24-hour clock: the user's system setting, else the custom of the system language.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub hour_cycle: Option<HourCycle>,
        /// The first day of the week, a `Weekday` (1 = Monday … 7 = Sunday): the user's system setting, else the custom of the system language.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub first_weekday: Option<u8>,
        /// Whether the window can dock an `as: 'aside'` beside the main; false until the host reports it.
        pub can_dock: bool,
        /// `fallback: "driver"` when this device's GPU driver failed and pages render on the CPU.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub renderer: Option<Renderer>,
        /// How much the system's text size grows body text: 1 at the default size, 2 when it is twice as large, after the lxapp's `maxTextScale`. Sizes already follow it; read it only for a layout that must change shape, such as fewer columns.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text_scale: Option<serde_json::Value>,
        /// The resolved theme: `{ revision, colors }`, one #rrggbbaa per role slot; the page never sees it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub palette: Option<serde_json::Value>,
        /// The resolved pack scales: `{ revision, values }`, one value per scale slot (points, ms for durations, a timing function for easings); the page never sees it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub scales: Option<serde_json::Value>,
    }

    /// Top, right, bottom and left, logical pixels.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct Edges {
        pub top: f64,
        pub right: f64,
        pub bottom: f64,
        pub left: f64,
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct Keyboard {
        pub height: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Capsule {
        pub x: f64,
        pub y: f64,
        pub width: f64,
        pub height: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Hinge {
        pub x: f64,
        pub y: f64,
        pub width: f64,
        pub height: f64,
        pub orientation: HingeOrientation,
        pub posture: HingePosture,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Appearance {
        #[serde(rename = "light")]
        Light,
        #[serde(rename = "dark")]
        Dark,
    }

    impl Appearance {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Light => "light",
                Self::Dark => "dark",
            }
        }

        pub fn parse(value: &str) -> Option<Self> {
            match value {
                "light" => Some(Self::Light),
                "dark" => Some(Self::Dark),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Pointer {
        #[serde(rename = "fine")]
        Fine,
        #[serde(rename = "coarse")]
        #[default]
        Coarse,
    }

    impl Pointer {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Fine => "fine",
                Self::Coarse => "coarse",
            }
        }

        pub fn parse(value: &str) -> Option<Self> {
            match value {
                "fine" => Some(Self::Fine),
                "coarse" => Some(Self::Coarse),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Os {
        #[serde(rename = "ios")]
        Ios,
        #[serde(rename = "macos")]
        Macos,
        #[serde(rename = "android")]
        Android,
        #[serde(rename = "windows")]
        Windows,
        #[serde(rename = "harmony")]
        Harmony,
        #[serde(rename = "unknown")]
        Unknown,
    }

    impl Os {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Ios => "ios",
                Self::Macos => "macos",
                Self::Android => "android",
                Self::Windows => "windows",
                Self::Harmony => "harmony",
                Self::Unknown => "unknown",
            }
        }

        pub fn parse(value: &str) -> Option<Self> {
            match value {
                "ios" => Some(Self::Ios),
                "macos" => Some(Self::Macos),
                "android" => Some(Self::Android),
                "windows" => Some(Self::Windows),
                "harmony" => Some(Self::Harmony),
                "unknown" => Some(Self::Unknown),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum SizeClass {
        #[serde(rename = "compact")]
        Compact,
        #[serde(rename = "regular")]
        Regular,
    }

    impl SizeClass {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Compact => "compact",
                Self::Regular => "regular",
            }
        }

        pub fn parse(value: &str) -> Option<Self> {
            match value {
                "compact" => Some(Self::Compact),
                "regular" => Some(Self::Regular),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum HourCycle {
        #[serde(rename = "h12")]
        H12,
        #[serde(rename = "h23")]
        H23,
    }

    impl HourCycle {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::H12 => "h12",
                Self::H23 => "h23",
            }
        }

        pub fn parse(value: &str) -> Option<Self> {
            match value {
                "h12" => Some(Self::H12),
                "h23" => Some(Self::H23),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum HingeOrientation {
        #[serde(rename = "vertical")]
        Vertical,
        #[serde(rename = "horizontal")]
        Horizontal,
    }

    impl HingeOrientation {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Vertical => "vertical",
                Self::Horizontal => "horizontal",
            }
        }

        pub fn parse(value: &str) -> Option<Self> {
            match value {
                "vertical" => Some(Self::Vertical),
                "horizontal" => Some(Self::Horizontal),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum HingePosture {
        #[serde(rename = "flat")]
        Flat,
        #[serde(rename = "half-opened")]
        HalfOpened,
    }

    impl HingePosture {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Flat => "flat",
                Self::HalfOpened => "half-opened",
            }
        }

        pub fn parse(value: &str) -> Option<Self> {
            match value {
                "flat" => Some(Self::Flat),
                "half-opened" => Some(Self::HalfOpened),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum RendererFallback {
        #[serde(rename = "driver")]
        Driver,
    }

    impl RendererFallback {
        pub fn as_str(self) -> &'static str {
            match self {
                Self::Driver => "driver",
            }
        }

        pub fn parse(value: &str) -> Option<Self> {
            match value {
                "driver" => Some(Self::Driver),
                _ => None,
            }
        }
    }

    #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
    pub struct Renderer {
        pub fallback: Option<RendererFallback>,
    }

    impl Environment {
        /// The snapshot as the session's `check_environment` and the View read it.
        pub fn to_json(&self) -> serde_json::Value {
            serde_json::to_value(self).unwrap_or_default()
        }
    }
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
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Runs, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Content, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Any, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Rows, content: false, parent: None, root: false, keyed: false, requires: &[field::LIST_ID, field::LIST_COUNT, field::LIST_LEADING, field::LIST_TRAILING] },
    TagAdmission { children: Children::None, content: true, parent: None, root: false, keyed: false, requires: &[field::IMAGE_SRC] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Content, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Content, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Options, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: false, parent: Some(tag::RADIO), root: false, keyed: true, requires: &[field::CHECKED] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Content, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Any, content: false, parent: None, root: true, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: true, parent: Some(tag::TEXT), root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: false, parent: Some(tag::TEXT), root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::Text, content: false, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: true, parent: None, root: false, keyed: false, requires: &[] },
    TagAdmission { children: Children::None, content: true, parent: None, root: false, keyed: false, requires: &[field::WEB_SRC, field::WEB_SESSION, field::ACCESSIBILITY_LABEL] },
];

/// The tags a field applies to (`None`: every tag) and the parent tag it requires.
#[derive(Clone, Copy, Debug)]
pub struct FieldAdmission {
    pub tags: Option<&'static [usize]>,
    pub parent: Option<usize>,
}

/// Indexed by field id.
/// Scale slots by wire index, with their family and their JSON pointer into a serialized pack
/// (empty for none); append-only like field ids.
pub const SCALES: &[(&str, &str, &str)] = &[
    ("text.xs", "type", "/type/scale/xs"),
    ("text.sm", "type", "/type/scale/sm"),
    ("text.base", "type", "/type/scale/base"),
    ("text.lg", "type", "/type/scale/lg"),
    ("text.xl", "type", "/type/scale/xl"),
    ("text.2xl", "type", "/type/scale/2xl"),
    ("text.3xl", "type", "/type/scale/3xl"),
    ("text.4xl", "type", "/type/scale/4xl"),
    ("leading.xs", "leading", "/type/leading/xs"),
    ("leading.sm", "leading", "/type/leading/sm"),
    ("leading.base", "leading", "/type/leading/base"),
    ("leading.lg", "leading", "/type/leading/lg"),
    ("leading.xl", "leading", "/type/leading/xl"),
    ("leading.2xl", "leading", "/type/leading/2xl"),
    ("leading.3xl", "leading", "/type/leading/3xl"),
    ("leading.4xl", "leading", "/type/leading/4xl"),
    ("radius.sm", "radius", "/shape/radius/sm"),
    ("radius.DEFAULT", "radius", "/shape/radius/DEFAULT"),
    ("radius.md", "radius", "/shape/radius/md"),
    ("radius.lg", "radius", "/shape/radius/lg"),
    ("radius.xl", "radius", "/shape/radius/xl"),
    ("radius.2xl", "radius", "/shape/radius/2xl"),
    ("radius.3xl", "radius", "/shape/radius/3xl"),
    ("space", "space", "/space/unit"),
    ("duration.fast", "duration", "/motion/fast"),
    ("duration.base", "duration", "/motion/base"),
    ("duration.slow", "duration", "/motion/slow"),
    ("ease.standard", "ease", "/motion/easing"),
    ("ease.enter", "ease", "/motion/enter"),
    ("ease.exit", "ease", "/motion/exit"),
    ("em", "em", ""),
];

/// What a slider part record may carry: the wire fields of the `part` style group.
pub const PART_FIELDS: &[usize] = &[field::WIDTH, field::HEIGHT, field::BACKGROUND, field::RADIUS, field::BORDER_COLOR, field::BORDER_TOP_WIDTH, field::BORDER_RIGHT_WIDTH, field::BORDER_BOTTOM_WIDTH, field::BORDER_LEFT_WIDTH, field::BOX_SHADOW, field::RADIUS_TOP_LEFT, field::RADIUS_TOP_RIGHT, field::RADIUS_BOTTOM_RIGHT, field::RADIUS_BOTTOM_LEFT, field::TRANSFORM, field::RING];
/// Paint-only fields a state delta may carry: the wire fields of every `paint` style.
pub const FOCUS_PAINT_FIELDS: &[usize] = &[field::COLOR, field::BACKGROUND, field::OPACITY, field::BORDER_COLOR, field::BACKGROUND_GRADIENT, field::BOX_SHADOW, field::PLACEHOLDER_COLOR, field::TRANSFORM, field::TRACK_STYLE, field::RANGE_STYLE, field::SECONDARY_STYLE, field::THUMB_STYLE, field::RING];

/// Fields that take a `[slot, times, plus]` reference, by the family of the slot.
pub const SCALED_FIELDS: &[(&str, &[usize])] = &[
    ("type", &[field::FONT_SIZE, field::LINE_HEIGHT, field::LETTER_SPACING]),
    ("leading", &[field::LINE_HEIGHT]),
    ("radius", &[field::RADIUS, field::RADIUS_TOP_LEFT, field::RADIUS_TOP_RIGHT, field::RADIUS_BOTTOM_RIGHT, field::RADIUS_BOTTOM_LEFT, field::RADIUS_START_START, field::RADIUS_START_END, field::RADIUS_END_START, field::RADIUS_END_END]),
    ("space", &[field::LINE_HEIGHT, field::PADDING_TOP, field::PADDING_RIGHT, field::PADDING_BOTTOM, field::PADDING_LEFT, field::PADDING_INLINE_START, field::PADDING_INLINE_END, field::GAP, field::ROW_GAP, field::COLUMN_GAP, field::MARGIN_TOP, field::MARGIN_RIGHT, field::MARGIN_BOTTOM, field::MARGIN_LEFT, field::MARGIN_INLINE_START, field::MARGIN_INLINE_END, field::INSET_TOP, field::INSET_RIGHT, field::INSET_BOTTOM, field::INSET_LEFT, field::INSET_INLINE_START, field::INSET_INLINE_END, field::FLEX_BASIS, field::WIDTH, field::HEIGHT, field::MIN_WIDTH, field::MIN_HEIGHT, field::MAX_WIDTH, field::MAX_HEIGHT]),
    ("em", &[field::WIDTH, field::HEIGHT, field::MIN_WIDTH, field::MIN_HEIGHT, field::MAX_WIDTH, field::MAX_HEIGHT, field::PADDING_TOP, field::PADDING_RIGHT, field::PADDING_BOTTOM, field::PADDING_LEFT, field::PADDING_INLINE_START, field::PADDING_INLINE_END, field::GAP, field::ROW_GAP, field::COLUMN_GAP, field::MARGIN_TOP, field::MARGIN_RIGHT, field::MARGIN_BOTTOM, field::MARGIN_LEFT, field::MARGIN_INLINE_START, field::MARGIN_INLINE_END, field::INSET_TOP, field::INSET_RIGHT, field::INSET_BOTTOM, field::INSET_LEFT, field::INSET_INLINE_START, field::INSET_INLINE_END]),
    ("duration", &[field::TRANSITION]),
    ("ease", &[field::TRANSITION]),
];
/// Every field a scaled family lists; decode resolves array values of these only.
pub const SCALE_STEP_FIELDS: &[usize] = &[field::WIDTH, field::HEIGHT, field::GAP, field::FONT_SIZE, field::RADIUS, field::MIN_WIDTH, field::MIN_HEIGHT, field::MAX_WIDTH, field::MAX_HEIGHT, field::LINE_HEIGHT, field::PADDING_TOP, field::PADDING_RIGHT, field::PADDING_BOTTOM, field::PADDING_LEFT, field::LETTER_SPACING, field::RADIUS_TOP_LEFT, field::RADIUS_TOP_RIGHT, field::RADIUS_BOTTOM_RIGHT, field::RADIUS_BOTTOM_LEFT, field::FLEX_BASIS, field::MARGIN_TOP, field::MARGIN_RIGHT, field::MARGIN_BOTTOM, field::MARGIN_LEFT, field::INSET_TOP, field::INSET_RIGHT, field::INSET_BOTTOM, field::INSET_LEFT, field::ROW_GAP, field::COLUMN_GAP, field::MARGIN_INLINE_START, field::MARGIN_INLINE_END, field::PADDING_INLINE_START, field::PADDING_INLINE_END, field::INSET_INLINE_START, field::INSET_INLINE_END, field::RADIUS_START_START, field::RADIUS_START_END, field::RADIUS_END_START, field::RADIUS_END_END, field::TRANSITION];

pub const FIELD_ADMISSION: &[FieldAdmission] = &[
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::LINK, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::INPUT, tag::TEXTAREA, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::RADIO_OPTION, tag::SEARCH, tag::SLIDER, tag::SELECT, tag::LINK, tag::LINK_BOX]), parent: None },
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
    FieldAdmission { tags: Some(&[tag::IMAGE, tag::VIDEO]), parent: None },
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
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::INPUT, tag::TEXTAREA, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::SEARCH, tag::SLIDER, tag::PLOT, tag::LINK, tag::COLUMN, tag::PAGE, tag::TEXT, tag::SELECT, tag::VIDEO, tag::WEBVIEW, tag::LIST, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: None, parent: None },
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
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::PAGE, tag::BUTTON, tag::LINK_BOX]), parent: None },
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
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::LINK, tag::SLIDER, tag::LINK_BOX]), parent: None },
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
    FieldAdmission { tags: Some(&[tag::IMAGE, tag::VIDEO]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL, tag::LIST]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::TEXT, tag::BUTTON, tag::SCROLL, tag::LIST, tag::IMAGE, tag::INPUT, tag::TEXTAREA, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::SEARCH, tag::SLIDER, tag::PLOT, tag::VECTOR, tag::PAGE, tag::SELECT, tag::VIDEO, tag::WEBVIEW, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL, tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: Some(&[tag::VIDEO]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::TEXT, tag::BUTTON, tag::LINK, tag::SCROLL, tag::LIST, tag::IMAGE, tag::INPUT, tag::TEXTAREA, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::SEARCH, tag::SLIDER, tag::PLOT, tag::VECTOR, tag::PAGE, tag::SELECT, tag::VIDEO, tag::WEBVIEW, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::CHECKBOX, tag::SWITCH, tag::RADIO, tag::SLIDER, tag::LINK, tag::SELECT, tag::INPUT, tag::TEXTAREA, tag::SEARCH, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::LINK, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::BUTTON, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::BUTTON, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::TEXT, tag::BUTTON, tag::VECTOR, tag::PLOT, tag::SCROLL]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::BUTTON, tag::VECTOR]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::BUTTON, tag::VECTOR]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::CHECKBOX, tag::BUTTON, tag::COLUMN]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::BUTTON, tag::LINK, tag::TEXT, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::SLIDER]), parent: None },
    FieldAdmission { tags: Some(&[tag::WEBVIEW]), parent: None },
    FieldAdmission { tags: Some(&[tag::WEBVIEW]), parent: None },
    FieldAdmission { tags: Some(&[tag::WEBVIEW]), parent: None },
    FieldAdmission { tags: Some(&[tag::WEBVIEW]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL, tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::PAGE]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::CHECKBOX, tag::SWITCH, tag::SLIDER, tag::SELECT]), parent: None },
    FieldAdmission { tags: Some(&[tag::CHECKBOX, tag::SWITCH, tag::RADIO_OPTION]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::COLUMN, tag::SCROLL, tag::LIST, tag::IMAGE, tag::VIDEO, tag::WEBVIEW, tag::VECTOR]), parent: Some(tag::PAGE) },
    FieldAdmission { tags: Some(&[tag::PAGE]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT]), parent: None },
    FieldAdmission { tags: None, parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON, tag::LINK, tag::LINK_BOX]), parent: None },
    FieldAdmission { tags: Some(&[tag::BUTTON]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
    FieldAdmission { tags: Some(&[tag::SCROLL, tag::LIST]), parent: None },
    FieldAdmission { tags: Some(&[tag::INPUT, tag::TEXTAREA, tag::SEARCH]), parent: None },
];
