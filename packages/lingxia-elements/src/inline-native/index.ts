export {
  INLINE_NATIVE_SCHEMA,
  NATIVE_ERROR_CODES,
  PUBLIC_COMPONENT_NAMES,
  PUBLIC_ELEMENT_TAGS,
  CORE_KINDS,
  HOST_FACTORY_KINDS,
  AUTHOR_COMPONENT_TO_TAG,
  TAG_TO_AUTHOR_COMPONENT,
  NATIVE_ACTION_ICONS,
  CONFLICTING_CONTROL_ICONS,
  POINTER_EVENTS_VALUES,
  HIDDEN_TRANSITION_VALUES,
  FULLSCREEN_SCOPE_VALUES,
  COVER_SCRIM_VALUES,
  type NativeErrorCode,
  type PublicComponentName,
  type PublicElementTag,
  type CoreKind,
  type HostFactoryKind,
  type NativeActionIcon,
} from "./schema.js";

export {
  compileInlineNativeRoot,
  compileInlineNativeForest,
  collectAuthorTreeFromElement,
  normalizeAuthorType,
  parseBooleanAttr,
  isFallbackElement,
  findInlineNativeRoot,
} from "./structure.js";

export { unwrapNativeEventPayload, bindPayloadHandler, isDomEvent } from "./unwrap.js";
export { identifyCompiledRoot, nextOpaqueKey, type IdentifiedRoot, type IdentifiedNode } from "./identity.js";
export { buildRootCommit, type NativeRootCommitJson } from "./commit.js";
export {
  emptyViewLease,
  viewApplyGrant,
  viewAcceptLease,
  viewMarkActive,
  viewCanShowFallback,
  type ViewLeaseState,
} from "./lease.js";
export { buildGeometrySnapshot, type NativeGeometrySnapshotJson } from "./geometry.js";
export {
  VIDEO_COMMANDS,
  buildVideoCommandRequest,
  collectVideoResourceUrls,
  videoCommandUrls,
  validateControlsSnapshot,
  type VideoCommand,
  type VideoCommandRequest,
  type VideoControlsSemanticSnapshot,
} from "./video.js";

export { nativeError, isNativeErrorCode } from "./errors.js";

export {
  EMPTY_ROOT_REF,
  type RootRef,
  type NodeRef,
  type NativeError,
  type AuthorNode,
  type AuthorChild,
  type CoreNode,
  type CompiledNativeRoot,
  type CompileInlineNativeResult,
  type CompileInlineNativeOptions,
  type PressPayload,
  type ValuePayload,
  type FocusPayload,
  type PointerPayload,
  type NativeHandler,
} from "./types.js";

export {
  LxNativeRootElement,
  LxNativeViewElement,
  LxNativeCoverElement,
  LxNativeTextElement,
  LxNativeButtonElement,
  LxNativeSliderElement,
  registerNativeRootComponent,
  registerNativeViewComponent,
  registerNativeCoverComponent,
  registerNativeTextComponent,
  registerNativeButtonComponent,
  registerNativeSliderComponent,
  registerInlineNativeAuthorComponents,
} from "./elements.js";

import { registerInlineNativeAuthorComponents } from "./elements.js";
import { registerVideoComponent } from "../video.js";

export function registerInlineNativeComponents(): void {
  registerInlineNativeAuthorComponents();
  registerVideoComponent();
}
