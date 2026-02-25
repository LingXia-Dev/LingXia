import { reactive } from "vue";
import { initBridge } from "./bridge";

type ActionMap = Record<string, (...args: unknown[]) => unknown>;
type Snapshot = Record<string, unknown>;

const reactiveSnapshot = reactive<Snapshot>({});
let subscribed = false;
let subscribeRetryTimer: ReturnType<typeof setTimeout> | null = null;

function replaceReactiveSnapshot(next: unknown): void {
  const normalized: Snapshot =
    next && typeof next === "object" ? (next as Snapshot) : {};

  for (const key of Object.keys(reactiveSnapshot)) {
    if (!Object.prototype.hasOwnProperty.call(normalized, key)) {
      delete reactiveSnapshot[key];
    }
  }
  Object.assign(reactiveSnapshot, normalized);
}

function scheduleSubscribeRetry(): void {
  if (subscribeRetryTimer !== null || subscribed) return;
  subscribeRetryTimer = setTimeout(() => {
    subscribeRetryTimer = null;
    ensureBridgeSubscription();
  }, 10);
}

function ensureBridgeSubscription(): void {
  if (subscribed) return;
  initBridge();
  const bridge = window.LingXiaBridge;
  if (!bridge?.subscribe) {
    scheduleSubscribeRetry();
    return;
  }
  bridge.subscribe((next) => {
    replaceReactiveSnapshot(next);
  });
  subscribed = true;
}

function resolveActions<TActions extends ActionMap>(): TActions {
  const actions: ActionMap = {};
  const pageFunctions = window.__PAGE_FUNCTIONS;
  if (!Array.isArray(pageFunctions)) {
    return actions as TActions;
  }

  for (const name of pageFunctions) {
    if (typeof name !== "string") continue;
    const fn = (window as unknown as Record<string, unknown>)[name];
    if (typeof fn === "function") {
      actions[name] = fn as (...args: unknown[]) => unknown;
    }
  }

  return actions as TActions;
}

export function useLingXia<
  TData = Snapshot,
  TActions extends ActionMap = ActionMap,
>(): { data: TData } & TActions {
  ensureBridgeSubscription();
  return { data: reactiveSnapshot as TData, ...resolveActions<TActions>() };
}
