import {
  getPageActions,
  getPageSnapshot,
  subscribePageSnapshot,
  whenPageReady,
  type ActionMap,
  type Snapshot,
} from "@lingxia/page-runtime";

export { getHost, subscribeHost, type LxHost } from "@lingxia/bridge";
export type { ActionMap, Snapshot } from "@lingxia/page-runtime";

/**
 * Resolves once the page's first state has arrived; rejects if Logic never
 * delivers it. Plain HTML has no mount to gate, so await this before reading
 * `getPage()`.
 */
export function pageReady(options?: { timeoutMs?: number }): Promise<void> {
  return whenPageReady(options);
}

/** This page's Logic state and actions — `this.data` and the page's methods. */
export function getPage<TData = Snapshot, TActions extends ActionMap = ActionMap>(): {
  data: TData;
  actions: TActions;
} {
  return { data: getPageSnapshot<TData>(), actions: getPageActions<TActions>() };
}

/** Follow the page's state. Change-only; read it with `getPage()`. Returns an unsubscribe. */
export function subscribePage(listener: () => void): () => void {
  return subscribePageSnapshot(listener);
}
export {
  registerInlineNativeComponents,
  registerInlineNativeAuthorComponents,
  compileInlineNativeRoot,
  compileInlineNativeForest,
  unwrapNativeEventPayload,
} from "@lingxia/elements";
