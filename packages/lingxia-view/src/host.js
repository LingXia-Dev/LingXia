import { render as rendering } from './runtime/state.js';
import { describe, prepare, prepareWindows, takeMovedAnchors, takeThrown, commit, discard, reset, dispatch, probeListRow } from './runtime/tree.js';
import { createView, syncData, hasLocalUpdates, readsHostContext } from './runtime/hooks.js';
import { configureActions, takeViewEvents, receiveActionResponse, takeActionErrors, requestLeave, automationAction, takeAutomationResults } from './runtime/actions.js';
import { viewCall, overlayTimer, takeOverlayTimers, escapeOverlay } from './runtime/overlay.js';
import { openPicker, pickerKey, closePicker, takePickerResults, pickerIdentity } from './runtime/picker.js';
import { takeFonts } from './runtime/fonts.js';
import { takeFrameMeasures, frameMeasured } from './runtime/frames.js';
import { wireAck } from './runtime/encode.js';
import { takeLowered, seedLowered } from './runtime/elements.js';
import { dismissHints, interest } from './runtime/toplayer.js';
import { takeSpans } from './runtime/profile.js';

// The native session calls only these globals; module code stays unreachable from the host.
export function publishHost(render, catalog) {
  Object.assign(globalThis, {
    __lingxiaRender: render, __lingxiaDescribe: describe, __lingxiaPrepare: prepare,
    __lingxiaPrepareWindows: prepareWindows, __lingxiaTakeMovedAnchors: takeMovedAnchors, __lingxiaCommit: commit, __lingxiaDiscard: discard,
    __lingxiaReset: reset, __lingxiaDispatch: dispatch, __lingxiaCreateView: createView, __lingxiaSyncData: syncData,
    __lingxiaConfigureActions: configureActions, __lingxiaTakeViewEvents: takeViewEvents,
    __lingxiaReceiveActionResponse: receiveActionResponse, __lingxiaTakeActionErrors: takeActionErrors,
    __lingxiaHasLocalUpdates: hasLocalUpdates, __lingxiaRequestLeave: requestLeave,
    __lingxiaReadsHostContext: readsHostContext,
    __lingxiaAutomationAction: automationAction, __lingxiaTakeAutomationResults: takeAutomationResults,
    __lingxiaViewCall: viewCall, __lingxiaOverlayTimer: overlayTimer, __lingxiaTakeOverlayTimers: takeOverlayTimers,
    __lingxiaEscape: escapeOverlay, __lingxiaInterest: interest, __lingxiaDismissHints: dismissHints, __lingxiaFonts: takeFonts, __lingxiaProbeListRow: probeListRow,
    __lingxiaOpenPicker: openPicker, __lingxiaPickerKey: pickerKey, __lingxiaClosePicker: closePicker,
    __lingxiaTakePickerResults: takePickerResults, __lingxiaPickerIdentity: pickerIdentity,
    __lingxiaTakeFrameMeasures: takeFrameMeasures, __lingxiaFrameMeasured: frameMeasured, __lingxiaWireAck: wireAck, __lingxiaEvalSpans: takeSpans,
    __lingxiaTakeLowered: takeLowered, __lingxiaSeedLowered: seedLowered,
    // Cleared when a render completes, so it names only a render that never returned.
    __lingxiaSite: () => rendering.site || '',
    __lingxiaThrown: takeThrown,
  });
  if (catalog !== undefined) globalThis.__lingxiaActionCatalog = catalog;
}
