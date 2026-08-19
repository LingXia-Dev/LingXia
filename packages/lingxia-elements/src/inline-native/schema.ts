/**
 * Machine-readable inline-native contract. Wrappers, the structure compiler,
 * and the host applicator share this object; unknown names are out of contract.
 */
export const NATIVE_ERROR_CODES = [
  "NATIVE_ROOT_UNAVAILABLE",
  "NATIVE_ROOT_INCOMPATIBLE",
  "NATIVE_ROOT_INVALID_STRUCTURE",
  "NATIVE_ROOT_FAILED",
  "NATIVE_ROOT_UNSUPPORTED_LAYOUT",
  "NATIVE_COMPONENT_INVALID_PROPS",
  "NATIVE_COMPONENT_MOUNT_FAILED",
  "NATIVE_COMPONENT_COMMAND_FAILED",
  "NATIVE_ROOT_UNSUPPORTED_STYLE",
  "NATIVE_ROOT_DESTROYED",
] as const;

export const PUBLIC_COMPONENT_NAMES = [
  "LxNativeRoot",
  "LxNativeView",
  "LxNativeCover",
  "LxNativeText",
  "LxNativeButton",
  "LxNativeSlider",
  "LxVideo",
] as const;

export const PUBLIC_ELEMENT_TAGS = [
  "lx-native-root",
  "lx-native-view",
  "lx-native-cover",
  "lx-native-text",
  "lx-native-button",
  "lx-native-slider",
  "lx-video",
] as const;

export const CORE_KINDS = ["root", "view", "text", "tappable", "slider"] as const;

export const HOST_FACTORY_KINDS = ["root", "view", "text", "tappable", "slider", "video"] as const;

export const AUTHOR_COMPONENT_TO_TAG = {
  LxNativeRoot: "lx-native-root",
  LxNativeView: "lx-native-view",
  LxNativeCover: "lx-native-cover",
  LxNativeText: "lx-native-text",
  LxNativeButton: "lx-native-button",
  LxNativeSlider: "lx-native-slider",
  LxVideo: "lx-video",
} as const;

export const TAG_TO_AUTHOR_COMPONENT = {
  "lx-native-root": "LxNativeRoot",
  "lx-native-view": "LxNativeView",
  "lx-native-cover": "LxNativeCover",
  "lx-native-text": "LxNativeText",
  "lx-native-button": "LxNativeButton",
  "lx-native-slider": "LxNativeSlider",
  "lx-video": "LxVideo",
} as const;

export const NATIVE_ACTION_ICONS = [
  "close",
  "play",
  "pause",
  "mute",
  "unmute",
  "fullscreen",
  "more",
] as const;

/** Icons that collide with LxVideo built-in chrome when `controls` is on. */
export const CONFLICTING_CONTROL_ICONS = [
  "play",
  "pause",
  "mute",
  "unmute",
  "fullscreen",
] as const;

export const POINTER_EVENTS_VALUES = ["auto", "none", "box-only", "box-none"] as const;

export const HIDDEN_TRANSITION_VALUES = ["none", "fade"] as const;

export const FULLSCREEN_SCOPE_VALUES = ["root", "none"] as const;

export const COVER_SCRIM_VALUES = ["none", "top", "bottom", "full"] as const;

export const STRUCTURE_ROLE_VALUES = [
  "group",
  "region",
  "status",
  "presentation",
  "none",
] as const;

export const BUTTON_INTENT_VALUES = ["neutral", "accent", "destructive"] as const;

export const BUTTON_EMPHASIS_VALUES = ["primary", "secondary", "quiet"] as const;

export const BUTTON_SIZE_VALUES = ["compact", "regular"] as const;

export const BUTTON_ICON_POSITION_VALUES = ["start", "end"] as const;

export const SLIDER_VALUE_LABEL_VALUES = ["none", "value", "time"] as const;

export const VIDEO_OBJECT_FIT_VALUES = ["contain", "cover", "fill", "none"] as const;

export const VIDEO_CONTENT_ROTATE_VALUES = [0, 90, 180, 270] as const;

export const WATERMARK_CORNER_VALUES = [
  "top-start",
  "top-end",
  "bottom-start",
  "bottom-end",
] as const;

/** Author NativeStyle fields consumed by the DOM anchor. Anything else is rejected. */
export const NATIVE_STYLE_LAYOUT_FIELDS = [
  "display",
  "position",
  "top",
  "right",
  "bottom",
  "left",
  "inset",
  "width",
  "height",
  "minWidth",
  "minHeight",
  "maxWidth",
  "maxHeight",
  "flex",
  "flexGrow",
  "flexShrink",
  "flexBasis",
  "flexDirection",
  "flexWrap",
  "alignItems",
  "alignSelf",
  "justifyContent",
  "gap",
  "rowGap",
  "columnGap",
  "gridTemplateColumns",
  "gridTemplateRows",
  "gridColumn",
  "gridRow",
  "margin",
  "marginTop",
  "marginRight",
  "marginBottom",
  "marginLeft",
  "padding",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "overflow",
  "aspectRatio",
  "boxSizing",
] as const;

export const NATIVE_STYLE_PAINT_FIELDS = [
  "backgroundColor",
  "opacity",
  "border",
  "borderWidth",
  "borderColor",
  "borderStyle",
  "borderRadius",
  "borderTopLeftRadius",
  "borderTopRightRadius",
  "borderBottomRightRadius",
  "borderBottomLeftRadius",
] as const;

export const NATIVE_STYLE_UNSUPPORTED_LAYOUT_FIELDS = [
  "transform",
  "translate",
  "rotate",
  "scale",
  "perspective",
  "clipPath",
  "maskImage",
] as const;

export const NATIVE_STYLE_UNSUPPORTED_PAINT_FIELDS = [
  "boxShadow",
  "filter",
  "backdropFilter",
  "mixBlendMode",
  "backgroundImage",
  "background",
  "animation",
  "transition",
  "textDecoration",
  "fontStyle",
  "letterSpacing",
  "textShadow",
] as const;

export const TEXT_STYLE_PROP_FIELDS = [
  "fontSize",
  "fontWeight",
  "lineHeight",
  "textAlign",
  "color",
] as const;

export const INTERACTIVE_AUTHOR_COMPONENTS = [
  "LxNativeButton",
  "LxNativeSlider",
  "LxVideo",
] as const;

export const CONTAINER_AUTHOR_COMPONENTS = [
  "LxNativeRoot",
  "LxNativeView",
  "LxNativeCover",
] as const;

export const INLINE_NATIVE_SCHEMA = {
  version: 1,
  publicComponents: PUBLIC_COMPONENT_NAMES,
  publicElementTags: PUBLIC_ELEMENT_TAGS,
  coreKinds: CORE_KINDS,
  hostFactoryKinds: HOST_FACTORY_KINDS,
  errorCodes: NATIVE_ERROR_CODES,
  authorComponentToTag: AUTHOR_COMPONENT_TO_TAG,
  tagToAuthorComponent: TAG_TO_AUTHOR_COMPONENT,
  nativeActionIcons: NATIVE_ACTION_ICONS,
  conflictingControlIcons: CONFLICTING_CONTROL_ICONS,
  pointerEvents: POINTER_EVENTS_VALUES,
  hiddenTransition: HIDDEN_TRANSITION_VALUES,
  fullscreenScope: FULLSCREEN_SCOPE_VALUES,
  coverScrim: COVER_SCRIM_VALUES,
  structureRoles: STRUCTURE_ROLE_VALUES,
  buttonIntent: BUTTON_INTENT_VALUES,
  buttonEmphasis: BUTTON_EMPHASIS_VALUES,
  buttonSize: BUTTON_SIZE_VALUES,
  buttonIconPosition: BUTTON_ICON_POSITION_VALUES,
  sliderValueLabel: SLIDER_VALUE_LABEL_VALUES,
  videoObjectFit: VIDEO_OBJECT_FIT_VALUES,
  videoContentRotate: VIDEO_CONTENT_ROTATE_VALUES,
  watermarkCorners: WATERMARK_CORNER_VALUES,
  nativeStyle: {
    layout: NATIVE_STYLE_LAYOUT_FIELDS,
    paint: NATIVE_STYLE_PAINT_FIELDS,
    unsupportedLayout: NATIVE_STYLE_UNSUPPORTED_LAYOUT_FIELDS,
    unsupportedPaint: NATIVE_STYLE_UNSUPPORTED_PAINT_FIELDS,
    textPropsNotStyle: TEXT_STYLE_PROP_FIELDS,
  },
  interactiveAuthorComponents: INTERACTIVE_AUTHOR_COMPONENTS,
  containerAuthorComponents: CONTAINER_AUTHOR_COMPONENTS,
  recipes: {
    LxNativeCover: { expandsTo: "view", hostKind: false },
    LxNativeButton: { expandsTo: "tappable", hostKind: false },
  },
  capabilityLeaves: {
    LxVideo: { hostKind: "video", mustBeDirectRootChild: true },
  },
} as const;

export type NativeErrorCode = (typeof NATIVE_ERROR_CODES)[number];
export type PublicComponentName = (typeof PUBLIC_COMPONENT_NAMES)[number];
export type PublicElementTag = (typeof PUBLIC_ELEMENT_TAGS)[number];
export type CoreKind = (typeof CORE_KINDS)[number];
export type HostFactoryKind = (typeof HOST_FACTORY_KINDS)[number];
export type NativeActionIcon = (typeof NATIVE_ACTION_ICONS)[number];
export type ConflictingControlIcon = (typeof CONFLICTING_CONTROL_ICONS)[number];
