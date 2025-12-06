// Video component
export {
  registerVideoComponent,
  LxVideoElement,
  type LxVideoAttributes
} from "./video.js";

// Platform detection utilities
export {
  isHarmony,
  isIOS,
  isAndroid,
  getOS
} from "./platform.js";

// Shared utilities for SameLevel custom elements
export {
  ensureComponentId,
  rectEquals,
  SameLevelUpdateState,
  type Rect
} from "./component.js";
