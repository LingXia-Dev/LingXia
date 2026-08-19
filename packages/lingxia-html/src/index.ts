export {
  getPageChromeLayout,
  getPageActions as getActions,
  getPageSnapshot as getSnapshot,
  getPageStateInfo as getStateInfo,
  subscribePageData as subscribe,
  subscribePageChromeLayout,
  subscribePageSnapshot as subscribeSnapshot,
  type ActionMap,
  type Snapshot,
} from "@lingxia/page-runtime";
export type {
  LxPageChrome,
  PageChromeLayoutListener,
  PageChromeLayoutSnapshot,
  PageChromeRect,
} from "@lingxia/page-runtime";
export {
  getDisplayLanguage,
  subscribeDisplayLanguage,
} from "@lingxia/bridge";
export {
  registerInlineNativeComponents,
  registerInlineNativeAuthorComponents,
  compileInlineNativeRoot,
  compileInlineNativeForest,
  unwrapNativeEventPayload,
} from "@lingxia/elements";
