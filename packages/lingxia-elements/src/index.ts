export {
  registerVideoComponent,
  LxVideoElement,
  type LxVideoAttributes,
} from "./video.js";

export {
  registerMediaSwiperComponent,
  LxMediaSwiperElement,
  type LxMediaSwiperAttributes,
  type LxMediaSwiperItem,
  type LxMediaSwiperChangeEvent,
  type LxMediaSwiperChangeEventDetail,
  type LxMediaSwiperTransitionEndEvent,
  type LxMediaSwiperTransitionEndEventDetail,
  type LxMediaSwiperItemEvent,
  type LxMediaSwiperEventDetail,
  type LxMediaSwiperEndReachedEvent,
  type LxMediaSwiperEndReachedEventDetail,
  type LxMediaSwiperErrorEvent,
  type LxMediaSwiperErrorEventDetail,
  type LxMediaSwiperChangeSource,
  type LxMediaSwiperErrorCode,
} from "./media_swiper.js";

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
  buildMediaSwiperNativeAttrs,
  buildNavigatorNativeAttrs,
  buildPickerNativeAttrs,
  getPickerDisplayText,
  getPickerValueFromIndex,
  VIDEO_DOM_EVENT_MAP,
  MEDIA_SWIPER_DOM_EVENT_MAP,
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
