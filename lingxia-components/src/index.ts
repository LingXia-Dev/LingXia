// Video component
export {
  registerVideoComponent,
  LxVideoElement,
  type LxVideoAttributes
} from "./video.js";

// Picker component
export {
  registerPickerComponent,
  LxPickerElement,
  type LxPickerAttributes,
  type LxPickerColumn,
  type LxPickerCascadingColumns,
  type LxPickerEvent,
  type LxPickerEventDetail
} from "./picker.js";

// Input component
export {
  registerInputComponent,
  LxInputElement,
  type LxInputAttributes,
  type LxInputEvent,
  type LxInputEventDetail
} from "./input.js";

// Textarea component
export {
  registerTextareaComponent,
  LxTextareaElement,
  type LxTextareaAttributes,
  type LxTextareaEvent,
  type LxTextareaEventDetail
} from "./textarea.js";

// Platform detection utilities
export {
  isHarmony,
  isIOS,
  isAndroid,
  isMacOS,
  isDesktop,
  getOS
} from "./platform.js";

// Shared utilities for native component custom elements
export {
  ensureComponentId,
  rectEquals,
  NativeComponentUpdateState,
  type Rect
} from "./component.js";
