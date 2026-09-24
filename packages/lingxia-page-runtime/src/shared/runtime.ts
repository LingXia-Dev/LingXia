import { type StateInfo } from "@lingxia/bridge";

export type ActionMap = Record<string, (...args: never[]) => unknown>;
export type Snapshot = Record<string, unknown>;
export type Listener = () => void;
type BridgeMode = "notify" | "call" | "stream";
type PageBridgeMetadata = {
  __names: string[];
  __modes?: Record<string, BridgeMode>;
  [key: string]: unknown;
};

let snapshot: Snapshot = {};
let stateInfo: StateInfo = { rev: -1, initial: true };
let subscribed = false;
let subscribeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let snapshotRetryTimer: ReturnType<typeof setTimeout> | null = null;
let snapshotRetries = 0;
/**
 * The host pushes a page's first state at bridge-ready, so the request is only
 * a fallback: a few tries, backing off, then stop. A page with no Logic never
 * answers, and must not be asked for the life of the page.
 */
const MAX_SNAPSHOT_RETRIES = 3;
let initialSnapshotResolved = false;
let actions: ActionMap | null = null;
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

function updateSnapshot(next: unknown, info: StateInfo): void {
  snapshot = next && typeof next === "object" ? (next as Snapshot) : {};
  stateInfo = info;
  notifyListeners();
}

function scheduleSubscribeRetry(): void {
  if (subscribeRetryTimer !== null || subscribed) return;
  subscribeRetryTimer = setTimeout(() => {
    subscribeRetryTimer = null;
    ensurePageBridgeSubscription();
  }, 10);
}

/**
 * Retry the snapshot request itself. The subscription is already in place, so
 * retrying it (as this used to) returned at once and never asked again; the
 * page then relied on the host's own push at bridge-ready.
 */
function scheduleSnapshotRetry(): void {
  if (snapshotRetryTimer !== null || snapshotRetries >= MAX_SNAPSHOT_RETRIES) return;
  snapshotRetries += 1;
  snapshotRetryTimer = setTimeout(() => {
    snapshotRetryTimer = null;
    requestInitialSnapshot(window.LingXiaBridge);
  }, 250 * 2 ** (snapshotRetries - 1));
}

function requestInitialSnapshot(bridge: Window["LingXiaBridge"] | undefined): void {
  if (stateInfo.rev >= 0) initialSnapshotResolved = true;
  if (initialSnapshotResolved || snapshotRequestInFlight) return;
  if (!bridge?.raw?.call) {
    scheduleSnapshotRetry();
    return;
  }
  snapshotRequestInFlight = true;
  bridge
    .raw.call("state.getSnapshot", { scope: "page" })
    .then(() => {
      initialSnapshotResolved = true;
    })
    .catch(() => {
      scheduleSnapshotRetry();
    })
    .finally(() => {
      snapshotRequestInFlight = false;
    });
}

export function ensurePageBridgeSubscription(): void {
  if (subscribed) return;
  const bridge = window.LingXiaBridge;
  const subscribeState = bridge?.state?.subscribe;
  if (!subscribeState) {
    scheduleSubscribeRetry();
    return;
  }
  subscribeState((next, info) => {
    updateSnapshot(next, info);
  });
  subscribed = true;
  requestInitialSnapshot(bridge);
}

export function subscribePageSnapshot(listener: Listener): () => void {
  ensurePageBridgeSubscription();
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Whether the page's first state has arrived. Flips once per document. */
export function isPageReady(): boolean {
  ensurePageBridgeSubscription();
  return stateInfo.rev >= 0;
}

/**
 * Resolves once the page's first state has arrived — the host pushes it at
 * bridge-ready, before `onLoad`, carrying `Page({ data })`'s defaults — so a
 * page can mount with its data whole. With `timeoutMs`, rejects if Logic has
 * not delivered it by then (it failed to load, or threw first); `null` waits
 * without limit.
 */
export function whenPageReady(options: { timeoutMs?: number | null } = {}): Promise<void> {
  if (isPageReady()) return Promise.resolve();
  const timeoutMs = options.timeoutMs === undefined ? 10_000 : options.timeoutMs;
  return new Promise((resolve, reject) => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const check = () => {
      if (stateInfo.rev < 0) return;
      if (timer !== null) clearTimeout(timer);
      listeners.delete(check);
      resolve();
    };
    if (timeoutMs !== null) {
      timer = setTimeout(() => {
        listeners.delete(check);
        reject(new Error(`Page Logic did not deliver the page state within ${timeoutMs} ms`));
      }, timeoutMs);
    }
    listeners.add(check);
  });
}

export function getPageSnapshot<TData = Snapshot>(): TData {
  ensurePageBridgeSubscription();
  return snapshot as TData;
}

/**
 * The page's actions: one object for the whole page, so it is a stable
 * dependency. Built on first use — the page's action names arrive with its
 * bridge metadata, after the page module has evaluated.
 */
export function getPageActions<TActions extends ActionMap>(): TActions {
  if (actions) return actions as TActions;
  const bridge = window.__pageBridge as PageBridgeMetadata | undefined;
  if (!bridge?.__names) {
    return {} as TActions;
  }
  const built: ActionMap = {};
  for (const name of bridge.__names) {
    if (typeof name !== "string") continue;
    const fn = getOrCreatePageAction(bridge, name);
    if (typeof fn === "function") {
      built[name] = fn;
    }
  }
  actions = built;
  return actions as TActions;
}

function getOrCreatePageAction(
  bridge: PageBridgeMetadata,
  name: string,
): ((...args: unknown[]) => unknown) | undefined {
  const existing = bridge[name];
  if (typeof existing === "function") {
    return existing as (...args: unknown[]) => unknown;
  }

  const mode = resolvePageActionMode(bridge, name);
  const created = definePageBridgeAction(name, mode);
  bridge[name] = created;
  return created;
}

function resolvePageActionMode(
  bridge: PageBridgeMetadata,
  name: string,
): BridgeMode {
  const mode = bridge.__modes?.[name];
  return mode === "call" || mode === "stream" ? mode : "notify";
}

function definePageBridgeAction(
  name: string,
  mode: BridgeMode,
): (...args: unknown[]) => unknown {
  function action(...args: unknown[]): unknown {
    const payload = filterPayload(name, args);
    const bridge = window.LingXiaBridge;
    if (!bridge) {
      throw new Error(`LingXiaBridge is not ready for page action '${name}'`);
    }
    if (mode === "stream") {
      const handle = bridge.raw.stream(name, payload);
      if (handle && handle.result && typeof handle.result.catch === "function") {
        handle.result.catch((err: unknown) => {
          console.warn(`[PageFunc] ${name} failed:`, err instanceof Error ? err.message : err);
        });
      }
      return handle;
    }
    if (mode === "call") {
      const promise = bridge.raw.call(name, payload);
      if (promise && typeof promise.catch === "function") {
        promise.catch((err: unknown) => {
          console.warn(`[PageFunc] ${name} failed:`, err instanceof Error ? err.message : err);
        });
      }
      return promise;
    }
    bridge.raw.notify(name, payload);
    return undefined;
  }

  Object.assign(action, {
    __logicFunc: true,
    __funcName: name,
    __bridgeMode: mode,
  });
  return action;
}

function filterPayload(name: string, args: unknown[]): unknown {
  const clean: unknown[] = [];
  for (const value of args) {
    // CustomEvent carries serializable data on `.detail`, but the DOM Event
    // wrapper itself is not portable across the bridge. Repackage as a plain
    // `{detail, type}` so page actions bound directly to DOM listeners (e.g.
    // `onVideoEnded={action}`) keep the familiar `event.detail` shape without
    // forwarding the live Event instance. Without this rewrite the bare Event
    // was stripped wholesale, producing `event = undefined` on the receiving
    // side — surfaced in the showcase as "video ended undefined".
    if (typeof CustomEvent !== "undefined" && value instanceof CustomEvent) {
      clean.push({ type: value.type, detail: value.detail });
      continue;
    }
    // Some framework wrappers / WebView realms do not preserve
    // `instanceof CustomEvent`, but still expose the portable event payload
    // shape. Keep it before the generic Event stripping path so page actions
    // receive `event.detail` consistently.
    const maybeEvent = value as { type?: unknown; detail?: unknown } | null;
    if (maybeEvent && typeof maybeEvent === "object" && typeof maybeEvent.type === "string" && "detail" in maybeEvent) {
      clean.push({
        type: maybeEvent.type,
        detail: maybeEvent.detail,
      });
      continue;
    }
    // Generic Event / event-like objects with non-serializable methods stay
    // stripped — there's no portable payload to extract.
    if (value instanceof Event) continue;
    if (
      value &&
      typeof value === "object" &&
      "stopPropagation" in value &&
      typeof (value as { stopPropagation?: unknown }).stopPropagation === "function"
    ) {
      continue;
    }
    clean.push(value);
  }
  if (clean.length > 1) {
    throw new Error(`Page action '${name}' accepts at most one payload argument`);
  }
  return clean[0];
}
