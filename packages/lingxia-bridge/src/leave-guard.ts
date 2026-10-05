/**
 * Guarding a page against being left with unsaved changes.
 *
 * While a page holds the guard, a user back or home (navigation bar buttons,
 * Android/Harmony system back, the iOS/Harmony edge swipe) does not leave it.
 * The host asks here instead, each holder answers, and when all agree the
 * host carries out what the user asked for.
 *
 * Works without Logic: a View-only (`logic: false`) page owns its drafts, so
 * the guard is held from the View.
 *
 * Its only import-time effect is installing the host hook (idempotent, shared
 * across bundle copies). It reaches the host through the already-booted
 * `window.LingXiaBridge` at call time and never imports the bridge module, so
 * the plain-HTML global build can include it without booting a second bridge.
 */

/** What the user did: `back` (bar, system, or swipe) or the bar's `home`. */
export type LxLeaveReason = 'back' | 'home';

export interface LxLeaveRequest {
  reason: LxLeaveReason;
}

/**
 * Answers a leave request: `true` lets the user leave, `false` keeps the
 * page. May be async — show a dialog and resolve with the choice.
 */
export type LxLeaveHandler = (request: LxLeaveRequest) => boolean | Promise<boolean>;

type GuardStore = {
  holders: LxLeaveHandler[];
  /** What the host was last told. */
  sent: boolean;
  /** Serializes host calls. */
  tail: Promise<void>;
  /** A request is being answered; further ones are dropped. */
  asking: boolean;
};

type GuardWindow = Window & { __lxLeaveGuard?: GuardStore };

function newStore(): GuardStore {
  return { holders: [], sent: false, tail: Promise.resolve(), asking: false };
}

const fallbackStore = newStore();

function store(): GuardStore {
  if (typeof window === 'undefined') return fallbackStore;
  // Shared by every copy of the bridge module in this document.
  const host = window as GuardWindow;
  if (!host.__lxLeaveGuard) host.__lxLeaveGuard = newStore();
  return host.__lxLeaveGuard;
}

// The host keeps one flag per page. Tell it whether anyone holds, one call at
// a time so a quick hold → release cannot land reversed.
function syncGuard(): void {
  const s = store();
  s.tail = s.tail.then(() => {
    const want = s.holders.length > 0;
    if (want === s.sent) return undefined;
    s.sent = want;
    return callHost('navigation.setLeaveGuard', { enabled: want }).catch((error) => {
      s.sent = !want;
      // A refused guard (bridge not ready, a runtime without the route)
      // leaves the page unguarded; say so rather than fail silently.
      console.warn('[lingxia] leave guard not applied; the page is not guarded', error);
    });
  });
}

/** Host entry point: the user tried to leave a guarded page. */
function dispatchLeaveRequest(reason: LxLeaveReason): void {
  const s = store();
  // One question at a time: a second back while a dialog is open is dropped.
  if (s.asking) return;
  s.asking = true;
  const request: LxLeaveRequest = { reason };
  // Newest holder first; the first refusal keeps the page.
  const holders = s.holders.slice().reverse();
  let agreed: Promise<boolean> = Promise.resolve(true);
  holders.forEach((handler) => {
    agreed = agreed.then((leave) => (leave ? handler(request) : false));
  });
  agreed
    .then(
      (leave) => leave === true,
      (error) => {
        console.error('[lingxia] leave guard handler failed; staying on the page', error);
        return false;
      },
    )
    .then((leave) => {
      s.asking = false;
      if (!leave) return undefined;
      return callHost('navigation.leave', { reason }).catch((error) => {
        console.warn('[lingxia] leaving the page failed', error);
      });
    });
}

if (typeof window !== 'undefined' && !window.__lingxiaDispatchLeaveRequest) {
  Object.defineProperty(window, '__lingxiaDispatchLeaveRequest', {
    configurable: false,
    enumerable: false,
    value: dispatchLeaveRequest,
  });
}

/**
 * Hold this page against user back and home until the returned function is
 * called. Hold while there are unsaved changes; `onRequest` is asked when the
 * user tries to leave and returns whether they may.
 *
 * Holds are independent: each part of a page holds for itself, the page stays
 * guarded while any hold is live, and every holder must agree to leave. A
 * reloaded document, or a page that has left the stack, starts unguarded.
 */
export function holdLeaveGuard(onRequest: LxLeaveHandler): () => void {
  const s = store();
  // A wrapper, so the same function held twice is two holds.
  const handler: LxLeaveHandler = (request) => onRequest(request);
  s.holders.push(handler);
  syncGuard();
  return () => {
    const index = s.holders.indexOf(handler);
    if (index < 0) return;
    s.holders.splice(index, 1);
    syncGuard();
  };
}

// A plain function rather than an `async` one: this module also builds for
// ES5. A missing bridge surfaces as a rejection, never a synchronous throw.
function callHost<TInput>(route: string, input: TInput): Promise<void> {
  const bridge = typeof window !== 'undefined' ? window.LingXiaBridge : undefined;
  if (!bridge) return Promise.reject(new Error('LingXiaBridge is not available'));
  try {
    return bridge.invoke<void, TInput>(route, input);
  } catch (error) {
    return Promise.reject(error);
  }
}
