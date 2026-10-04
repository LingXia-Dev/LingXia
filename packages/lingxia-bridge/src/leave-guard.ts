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
 * Side-effect-safe apart from installing the host hook: it reaches the host
 * through the already-booted `window.LingXiaBridge` at call time, so the
 * plain-HTML global build can include it without booting a second bridge.
 */

type BackRequestStore = { listeners: Set<() => void> };

const fallbackStore: BackRequestStore = { listeners: new Set() };

function store(): BackRequestStore {
  if (typeof window === 'undefined') return fallbackStore;
  // Shared by every copy of the bridge module in this document.
  if (!window.__lxBackRequest) window.__lxBackRequest = { listeners: new Set() };
  return window.__lxBackRequest;
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
  const bridge = typeof window !== 'undefined' ? window.LingXiaBridge : undefined;
  if (!bridge) return Promise.reject(new Error('LingXiaBridge is not available'));
  return bridge.invoke<void, { enabled: boolean }>('navigation.setLeaveGuard', { enabled });
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
