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

/** Aborts when any of `signals` does. */
export function anySignal(...signals: readonly AbortSignalLike[]): AbortSignalLike {
  const listeners = new Set<() => void>();
  let aborted = signals.some(s => s.aborted);
  const fire = () => {
    if (aborted) return;
    aborted = true;
    for (const fn of [...listeners]) fn();
    listeners.clear();
  };
  for (const s of signals) s.addEventListener('abort', fire, { once: true });
  return {
    get aborted() {
      return aborted;
    },
    addEventListener(_type, listener) {
      if (aborted) listener();
      else listeners.add(listener);
    },
    removeEventListener(_type, listener) {
      listeners.delete(listener);
    },
  };
}
