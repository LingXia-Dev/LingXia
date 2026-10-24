import { describe, prepare, prepareWindows, commit, discard, reset, dispatch, createView, configureActions,
  takeActionRequests, receiveActionResponse, takeActionErrors, hasLocalUpdates, requestLeave } from './runtime.js';

// The native session calls only these globals; module code stays unreachable from the host.
export function publishHost(render, catalog) {
  Object.assign(globalThis, {
    __lingxiaRender: render, __lingxiaDescribe: describe, __lingxiaPrepare: prepare,
    __lingxiaPrepareWindows: prepareWindows, __lingxiaCommit: commit, __lingxiaDiscard: discard,
    __lingxiaReset: reset, __lingxiaDispatch: dispatch, __lingxiaCreateView: createView,
    __lingxiaConfigureActions: configureActions, __lingxiaTakeActionRequests: takeActionRequests,
    __lingxiaReceiveActionResponse: receiveActionResponse, __lingxiaTakeActionErrors: takeActionErrors,
    __lingxiaHasLocalUpdates: hasLocalUpdates, __lingxiaRequestLeave: requestLeave,
  });
  if (catalog !== undefined) globalThis.__lingxiaActionCatalog = catalog;
}
