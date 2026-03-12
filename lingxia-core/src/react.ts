import * as React from "react";
import type {} from "./types";

type ActionMap = Record<string, (...args: unknown[]) => unknown>;
type Snapshot = Record<string, unknown>;
type Listener = () => void;

let snapshot: Snapshot = {};
let subscribed = false;
let subscribeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let initialSnapshotResolved = false;
let snapshotRequestInFlight = false;
const listeners = new Set<Listener>();

function notifyListeners(): void {
  listeners.forEach((listener) => {
    try {
      listener();
    } catch {
      // Ignore listener errors to avoid breaking state fanout.
    }
  });
}

function updateSnapshot(next: unknown): void {
  if (next && typeof next === "object") {
    snapshot = next as Snapshot;
  } else {
    snapshot = {};
  }
  notifyListeners();
}

function scheduleSubscribeRetry(): void {
  if (subscribeRetryTimer !== null || subscribed) return;
  subscribeRetryTimer = setTimeout(() => {
    subscribeRetryTimer = null;
    ensureBridgeSubscription();
  }, 10);
}

function requestInitialSnapshot(bridge: Window["LingXiaBridge"] | undefined): void {
  if (initialSnapshotResolved || snapshotRequestInFlight) return;
  if (!bridge?.call) return;
  snapshotRequestInFlight = true;
  bridge
    .call("state.getSnapshot", { scope: "page" })
    .then(() => {
      initialSnapshotResolved = true;
    })
    .catch(() => {
      scheduleSubscribeRetry();
    })
    .finally(() => {
      snapshotRequestInFlight = false;
    });
}

function ensureBridgeSubscription(): void {
  if (subscribed) return;
  const bridge = window.LingXiaBridge;
  if (!bridge?.subscribe) {
    scheduleSubscribeRetry();
    return;
  }
  bridge.subscribe((next) => {
    updateSnapshot(next);
  });
  subscribed = true;
  requestInitialSnapshot(bridge);
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
  const [, setVersion] = React.useState(0);

  React.useEffect(() => {
    ensureBridgeSubscription();
    const listener: Listener = () => setVersion((v) => v + 1);
    listeners.add(listener);
    // Pull latest snapshot that may arrive before this component subscribes.
    setVersion((v) => v + 1);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  const actions = React.useMemo(() => resolveActions<TActions>(), []);
  return { data: snapshot as TData, ...actions };
}
