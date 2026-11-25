/** A native stream: iterate its events; `cancel()` stops it. */
export interface NativeTask<E> extends AsyncIterable<E> {
  readonly result: Promise<unknown>;
  cancel(): void;
}

interface LogicNativeDispatch {
  invoke<R>(route: string, input?: unknown): Promise<R>;
  stream<E>(route: string, input: unknown, options: { signal: AbortSignal }): NativeTask<E>;
}

// Framework routes are not in the host's generated client, so call the Logic dispatch directly.
function dispatch(): LogicNativeDispatch {
  const host = (globalThis as { __lingxiaNative?: LogicNativeDispatch }).__lingxiaNative;
  if (!host) throw new Error('LingXia Logic native dispatch is not installed');
  return host;
}

export function invoke<R = void>(route: string, input?: unknown): Promise<R> {
  return dispatch().invoke<R>(route, input);
}

/**
 * Feed each event to `onEvent` until `signal` aborts (pass the page's `signal`). A stream that fails or
 * ends opens again after a backoff; `onReopen` runs first, for a route whose events assume what came before.
 */
export function watch<E>(route: string, signal: AbortSignal, onEvent: (event: E) => void, onReopen?: () => void): void {
  let delay = WATCH_RETRY_MS;
  (async () => {
    for (let opened = false; !signal.aborted; opened = true) {
      if (opened) onReopen?.();
      try {
        for await (const event of dispatch().stream<E>(route, undefined, { signal })) {
          if (signal.aborted) return;
          delay = WATCH_RETRY_MS;
          onEvent(event);
        }
      } catch (error) {
        if (!signal.aborted) console.warn(`${route} stopped; reopening`, error);
      }
      if (signal.aborted) return;
      await new Promise<void>((resolve) => { setTimeout(() => resolve(), delay); });
      delay = Math.min(delay * 2, WATCH_RETRY_MAX_MS);
    }
  })();
}

const WATCH_RETRY_MS = 1_000;
const WATCH_RETRY_MAX_MS = 15_000;
