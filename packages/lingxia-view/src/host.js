import { releasePress } from './runtime/press.js';
import { render as rendering, page } from './runtime/state.js';
import { describe, prepare, prepareWindows, takeMovedAnchors, takeThrown, commit, discard, reset, dispatch, probeListRow } from './runtime/tree.js';
import { createView, syncData, hasLocalUpdates, readsHostContext, envReads, envOf } from './runtime/hooks.js';
import { configureActions, takeViewEvents, receiveActionResponse, takeActionErrors, requestLeave, automationAction, takeAutomationResults } from './runtime/actions.js';
import { viewCall, overlayTimer, takeOverlayTimers, escapeOverlay } from './runtime/overlay.js';
import { openPicker, pickerKey, closePicker, takePickerResults, pickerIdentity } from './runtime/picker.js';
import { takeFonts } from './runtime/fonts.js';
import { takeReveals } from './runtime/reveal.js';
import { takeFrameMeasures, frameMeasured } from './runtime/frames.js';
import { wireAck } from './runtime/encode.js';
import { takeLowered, seedLowered } from './runtime/elements.js';
import { closeAfterExpired, closeOrphans, dismissHints, interest, ownedPopovers } from './runtime/toplayer.js';
import { takeSpans } from './runtime/profile.js';
import { fieldText } from './runtime/calendar.js';

// An unbound date or time field's own value in the user's clock and language, as a bound one
// shows it. The core asks while it prepares a frame, before the commit, so its own language and
// hour cycle come first; the committed page's env fills in. Never throws.
function unboundFieldText(kind, iso, opts) {
  if (typeof iso !== 'string') return '';
  try {
    if (kind !== 'date' && kind !== 'time') return iso;
    const env = envOf(page.committed?.view);
    return fieldText(kind, iso, { seconds: opts?.seconds === true, language: opts?.language || (env?.contentLanguage ?? env?.displayLanguage),
      hourCycle: opts?.hourCycle || env?.hourCycle }).displayValue;
  } catch { return iso; }
}

// The native session calls only these globals; module code stays unreachable from the host.
export function publishHost(render, catalog) {
  Object.assign(globalThis, {
    __lingxiaRender: render, __lingxiaDescribe: describe, __lingxiaPrepare: prepare,
    __lingxiaPrepareWindows: prepareWindows, __lingxiaTakeMovedAnchors: takeMovedAnchors, __lingxiaCommit: commit, __lingxiaDiscard: discard,
    __lingxiaReset: reset, __lingxiaDispatch: dispatch, __lingxiaCreateView: createView, __lingxiaSyncData: syncData,
    __lingxiaConfigureActions: configureActions, __lingxiaTakeViewEvents: takeViewEvents,
    __lingxiaReceiveActionResponse: receiveActionResponse, __lingxiaTakeActionErrors: takeActionErrors,
    __lingxiaHasLocalUpdates: hasLocalUpdates, __lingxiaReleasePress: releasePress, __lingxiaRequestLeave: requestLeave,
    __lingxiaReadsHostContext: readsHostContext, __lingxiaEnvReads: envReads,
    __lingxiaAutomationAction: automationAction, __lingxiaTakeAutomationResults: takeAutomationResults,
    __lingxiaViewCall: viewCall, __lingxiaOverlayTimer: overlayTimer, __lingxiaTakeOverlayTimers: takeOverlayTimers,
    __lingxiaEscape: escapeOverlay, __lingxiaInterest: interest, __lingxiaDismissHints: dismissHints, __lingxiaCloseAfter: closeAfterExpired, __lingxiaFonts: takeFonts, __lingxiaProbeListRow: probeListRow,
    __lingxiaOpenPicker: openPicker, __lingxiaPickerKey: pickerKey, __lingxiaClosePicker: closePicker,
    __lingxiaTakePickerResults: takePickerResults, __lingxiaPickerIdentity: pickerIdentity,
    __lingxiaOwnedOverlays: () => JSON.stringify({ picker: pickerIdentity(), popovers: ownedPopovers() }),
    __lingxiaCloseOrphans: (picker, ids) => { if (picker) closePicker(); closeOrphans(ids); },
    __lingxiaTakeFrameMeasures: takeFrameMeasures, __lingxiaFrameMeasured: frameMeasured, __lingxiaWireAck: wireAck, __lingxiaTakeReveals: takeReveals, __lingxiaEvalSpans: takeSpans,
    __lingxiaTakeLowered: takeLowered, __lingxiaSeedLowered: seedLowered, __lingxiaFieldText: unboundFieldText,
    // Cleared when a render completes, so it names only a render that never returned.
    __lingxiaSite: () => rendering.site || '',
    __lingxiaThrown: takeThrown,
  });
  if (catalog !== undefined) globalThis.__lingxiaActionCatalog = catalog;
}
