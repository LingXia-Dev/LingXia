/** The part of `AbortSignal` Jinshu uses; Logic has it, the package's ES lib does not declare it. */
export interface AbortSignalLike {
  readonly aborted: boolean;
  addEventListener(type: 'abort', listener: () => void, options?: { readonly once?: boolean }): void;
  removeEventListener(type: 'abort', listener: () => void): void;
}

declare const AbortSignal: { timeout(ms: number): AbortSignalLike };

/** A signal that aborts after `ms`. */
export function timeoutSignal(ms: number): AbortSignalLike {
  return AbortSignal.timeout(ms);
}

/**
 * Aborts when any of `signals` does. It listens to them only while something listens to it, so a
 * long-lived signal, such as a tool call's, does not keep one listener per wait.
 */
export function anySignal(...signals: readonly AbortSignalLike[]): AbortSignalLike {
  const listeners = new Set<() => void>();
  let aborted = signals.some(s => s.aborted);
  let attached = false;
  const detach = () => {
    if (!attached) return;
    attached = false;
    for (const s of signals) s.removeEventListener('abort', fire);
  };
  function fire() {
    if (aborted) return;
    aborted = true;
    detach();
    for (const fn of [...listeners]) fn();
    listeners.clear();
  }
  return {
    get aborted() {
      return aborted || signals.some(s => s.aborted);
    },
    addEventListener(_type, listener) {
      if (aborted || signals.some(s => s.aborted)) {
        aborted = true;
        listener();
        return;
      }
      listeners.add(listener);
      if (!attached) {
        attached = true;
        for (const s of signals) s.addEventListener('abort', fire, { once: true });
      }
    },
    removeEventListener(_type, listener) {
      listeners.delete(listener);
      if (listeners.size === 0) detach();
    },
  };
}
