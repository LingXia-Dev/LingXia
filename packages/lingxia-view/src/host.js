import { render as rendering } from './runtime/state.js';
import { describe, prepare, prepareWindows, takeMovedAnchors, commit, discard, reset, dispatch, probeListRow } from './runtime/tree.js';
import { createView, hasLocalUpdates, readsHostContext } from './runtime/hooks.js';
import { configureActions, takeActionRequests, receiveActionResponse, takeActionErrors, requestLeave, automationAction, takeAutomationResults } from './runtime/actions.js';
import { viewRequest, overlayTimer, takeOverlayTimers, escapeOverlay } from './runtime/overlay.js';
import { openPicker, pickerKey, closePicker, takePickerResults, pickerIdentity } from './runtime/picker.js';
import { takeFonts } from './runtime/fonts.js';
import { takeFrameMeasures, frameMeasured } from './runtime/frames.js';
import { wireAck } from './runtime/encode.js';

// The native session calls only these globals; module code stays unreachable from the host.
export function publishHost(render, catalog) {
  Object.assign(globalThis, {
    __lingxiaRender: render, __lingxiaDescribe: describe, __lingxiaPrepare: prepare,
    __lingxiaPrepareWindows: prepareWindows, __lingxiaTakeMovedAnchors: takeMovedAnchors, __lingxiaCommit: commit, __lingxiaDiscard: discard,
    __lingxiaReset: reset, __lingxiaDispatch: dispatch, __lingxiaCreateView: createView,
    __lingxiaConfigureActions: configureActions, __lingxiaTakeActionRequests: takeActionRequests,
    __lingxiaReceiveActionResponse: receiveActionResponse, __lingxiaTakeActionErrors: takeActionErrors,
    __lingxiaHasLocalUpdates: hasLocalUpdates, __lingxiaRequestLeave: requestLeave,
    __lingxiaReadsHostContext: readsHostContext,
    __lingxiaAutomationAction: automationAction, __lingxiaTakeAutomationResults: takeAutomationResults,
    __lingxiaViewRequest: viewRequest, __lingxiaOverlayTimer: overlayTimer, __lingxiaTakeOverlayTimers: takeOverlayTimers,
    __lingxiaEscape: escapeOverlay, __lingxiaFonts: takeFonts, __lingxiaProbeListRow: probeListRow,
    __lingxiaOpenPicker: openPicker, __lingxiaPickerKey: pickerKey, __lingxiaClosePicker: closePicker,
    __lingxiaTakePickerResults: takePickerResults, __lingxiaPickerIdentity: pickerIdentity,
    __lingxiaTakeFrameMeasures: takeFrameMeasures, __lingxiaFrameMeasured: frameMeasured, __lingxiaWireAck: wireAck,
    // Cleared when a render completes, so it names only a render that never returned.
    __lingxiaSite: () => rendering.site || '',
  });
  if (catalog !== undefined) globalThis.__lingxiaActionCatalog = catalog;
}
