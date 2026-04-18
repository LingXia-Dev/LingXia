export {
  registerVideoComponent,
  LxVideoElement,
  type LxVideoAttributes,
} from "./video.js";

export {
  registerPickerComponent,
  LxPickerElement,
  type LxPickerAttributes,
  type LxPickerColumn,
  type LxPickerCascadingColumns,
  type LxPickerEvent,
  type LxPickerEventDetail,
} from "./picker.js";

export {
  registerInputComponent,
  LxInputElement,
  type LxInputAttributes,
  type LxInputEvent,
  type LxInputEventDetail,
} from "./input.js";

export {
  registerTextareaComponent,
  LxTextareaElement,
  type LxTextareaAttributes,
  type LxTextareaEvent,
  type LxTextareaEventDetail,
} from "./textarea.js";

export {
  LxNavigatorElement,
  type LxNavigatorAttributes,
  type LxNavigatorEvent,
  type NavigatorEnvVersion,
  type NavigatorOpenType,
  type NavigatorQuery,
  type NavigatorQueryValue,
  type NavigatorTarget,
} from "./navigator.js";

export {
  buildVideoNativeAttrs,
  buildNavigatorNativeAttrs,
  buildPickerNativeAttrs,
  getPickerDisplayText,
  getPickerValueFromIndex,
  VIDEO_DOM_EVENT_MAP,
} from "./native_component_wrapper_shared.js";

export {
  buildInputNativeAttrs,
  buildTextareaNativeAttrs,
} from "./text_component_native_attrs.js";

export {
  isHarmony,
  isIOS,
  isAndroid,
  isMacOS,
  isDesktop,
  getOS,
} from "./platform.js";

export {
  ensureComponentId,
  rectEquals,
  NativeComponentUpdateState,
  type Rect,
} from "./component.js";
