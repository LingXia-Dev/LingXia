/**
 * Guarding a page against being left with unsaved changes.
 *
 * Every user back path ends in one host decision: the navigation bar back
 * button, Android/Harmony system back, and the iOS/Harmony edge swipe. While a
 * page holds the guard, that decision does not pop the page; it delivers a
 * back request here instead, and the page answers — typically by asking, then
 * calling `navigation.navigateBack`, which pops without asking again.
 *
 * Works without Logic: a View-only (`logic: false`) page owns its drafts, so
 * the guard is set from the View.
 *
 * Its only import-time effect is installing the host hook (idempotent, shared
 * across bundle copies). It reaches the host through the already-booted
 * `window.LingXiaBridge` at call time and never imports the bridge module, so
 * the plain-HTML global build can include it without booting a second bridge.
 */

type BackRequestStore = NonNullable<Window['__lxBackRequest']>;

function newStore(): BackRequestStore {
  return { listeners: new Set(), manual: false, holds: 0, sent: false, tail: Promise.resolve() };
}

const fallbackStore = newStore();

function store(): BackRequestStore {
  if (typeof window === 'undefined') return fallbackStore;
  // Shared by every copy of the bridge module in this document.
  if (!window.__lxBackRequest) window.__lxBackRequest = newStore();
  return window.__lxBackRequest;
}

// The host keeps one flag per page. Send it the union of every holder, one
// call at a time so a quick true → false cannot land reversed.
function syncGuard(): Promise<void> {
  const s = store();
  const run = (): Promise<void> => {
    const want = s.manual || s.holds > 0;
    if (want === s.sent) return Promise.resolve();
    s.sent = want;
    return callHost<{ enabled: boolean }>('navigation.setLeaveGuard', { enabled: want }).catch(
      (error) => {
        s.sent = !want;
        throw error;
      },
    );
  };
  const next = s.tail.then(run);
  s.tail = next.catch(() => undefined);
  return next;
}

/** Host entry point: the user tried to leave a guarded page. */
function dispatchBackRequest(): void {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('lxbackrequest'));
  }
  for (const listener of [...store().listeners]) listener();
}

if (typeof window !== 'undefined' && !window.__lingxiaDispatchBackRequest) {
  Object.defineProperty(window, '__lingxiaDispatchBackRequest', {
    configurable: false,
    enumerable: false,
    value: dispatchBackRequest,
  });
}

/**
 * Hold (`true`) or release (`false`) this page against user back. Set it
 * while the page has unsaved changes and clear it once they are saved or
 * discarded. A reloaded document, or a page that has left the stack, starts
 * unguarded.
 */
export function setLeaveGuard(enabled: boolean): Promise<void> {
  store().manual = enabled;
  return syncGuard();
}

/**
 * Hold the page until the returned release is called. Holds count, so
 * independent parts of a page (two forms) guard it without clearing each
 * other; `setLeaveGuard` is one more holder beside them.
 */
export function holdLeaveGuard(): () => void {
  const s = store();
  s.holds += 1;
  void syncGuard().catch(warnLeaveGuard);
  let released = false;
  return () => {
    if (released) return;
    released = true;
    s.holds -= 1;
    void syncGuard().catch(warnLeaveGuard);
  };
}

// A refused guard (bridge not ready, a runtime without the route) leaves the
// page unguarded; say so rather than fail silently.
function warnLeaveGuard(error: unknown): void {
  console.warn('[lingxia] setLeaveGuard failed; the page is not guarded', error);
}

/**
 * Leave the page now — the answer to a back request the user confirmed. Pops
 * this page without asking again, guarded or not.
 */
export function leavePage(): Promise<void> {
  return callHost<{ delta: number }>('navigation.navigateBack', { delta: 1 });
}

// Plain functions rather than `async` ones: this module also builds for ES5.
// A missing bridge still surfaces as a rejection, never a synchronous throw.
function callHost<TInput>(route: string, input: TInput): Promise<void> {
  const bridge = typeof window !== 'undefined' ? window.LingXiaBridge : undefined;
  if (!bridge) return Promise.reject(new Error('LingXiaBridge is not available'));
  try {
    return bridge.invoke<void, TInput>(route, input);
  } catch (error) {
    return Promise.reject(error);
  }
}

/**
 * Called when the user tries to leave the page while it holds the guard. The
 * page stays where it is; confirm, then leave with `navigation.navigateBack`.
 * Returns an unsubscribe. The same moment is also a `lxbackrequest` event on
 * `window`.
 */
export function subscribeBackRequest(listener: () => void): () => void {
  const listeners = store().listeners;
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
