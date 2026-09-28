import {
  getPageActions,
  getPageSnapshot,
  subscribePageSnapshot,
  whenPageReady,
  waitForPageState,
  type ActionMap,
  type PageActions,
  type DeepReadonly,
  type Snapshot,
} from "@lingxia/page-runtime";

export { getHost, subscribeHost, type LxHost } from "@lingxia/bridge";
export type { ActionMap, DeepReadonly, Snapshot } from "@lingxia/page-runtime";

/**
 * Resolves once the first state arrives. By default a delayed startup shows
 * a fault panel and keeps waiting, just like React/Vue. An explicit timeout
 * rejects instead; `null` waits without the panel. Await before `getPage()`.
 */
export function pageReady(options?: { timeoutMs?: number | null }): Promise<void> {
  return options?.timeoutMs === undefined ? waitForPageState() : whenPageReady(options);
}

/**
 * This page's Logic state and actions — `this.data` and the page's methods.
 * `data` is readonly: Logic owns it (in a dev session a write throws).
 */
export function getPage<TData = Snapshot, TActions extends ActionMap = ActionMap>(): {
  data: DeepReadonly<TData>;
  actions: PageActions<TActions>;
} {
  return { data: getPageSnapshot<DeepReadonly<TData>>(), actions: getPageActions<TActions>() };
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
