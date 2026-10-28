import { render as rendering } from './runtime/state.js';
import { describe, prepare, prepareWindows, commit, discard, reset, dispatch } from './runtime/tree.js';
import { createView, hasLocalUpdates, readsHostContext } from './runtime/hooks.js';
import { configureActions, takeActionRequests, receiveActionResponse, takeActionErrors, requestLeave, automationAction, takeAutomationResults } from './runtime/actions.js';

// The native session calls only these globals; module code stays unreachable from the host.
export function publishHost(render, catalog) {
  Object.assign(globalThis, {
    __lingxiaRender: render, __lingxiaDescribe: describe, __lingxiaPrepare: prepare,
    __lingxiaPrepareWindows: prepareWindows, __lingxiaCommit: commit, __lingxiaDiscard: discard,
    __lingxiaReset: reset, __lingxiaDispatch: dispatch, __lingxiaCreateView: createView,
    __lingxiaConfigureActions: configureActions, __lingxiaTakeActionRequests: takeActionRequests,
    __lingxiaReceiveActionResponse: receiveActionResponse, __lingxiaTakeActionErrors: takeActionErrors,
    __lingxiaHasLocalUpdates: hasLocalUpdates, __lingxiaRequestLeave: requestLeave,
    __lingxiaReadsHostContext: readsHostContext,
    __lingxiaAutomationAction: automationAction, __lingxiaTakeAutomationResults: takeAutomationResults,
    // Cleared when a render completes, so it names only a render that never returned.
    __lingxiaSite: () => rendering.site || '',
  });
  if (catalog !== undefined) globalThis.__lingxiaActionCatalog = catalog;
}
