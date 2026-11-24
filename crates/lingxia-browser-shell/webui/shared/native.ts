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

/** Feed each event to `onEvent` until `signal` aborts (pass the page's `signal`). */
export function watch<E>(route: string, signal: AbortSignal, onEvent: (event: E) => void): void {
  const task = dispatch().stream<E>(route, undefined, { signal });
  (async () => {
    for await (const event of task) {
      if (signal.aborted) break;
      onEvent(event);
    }
  })().catch((error) => {
    if (!signal.aborted) console.warn(`${route} stopped`, error);
  });
}
