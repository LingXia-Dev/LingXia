/**
 * Feed each event of the stream `open` starts to `onEvent` until `signal` aborts (pass the page's `signal`).
 * A stream that fails or ends opens again after a backoff; `onReopen` runs first, for a route whose events
 * assume what came before.
 */
export function watch<E>(
  open: (options: { signal: AbortSignal }) => AsyncIterable<E>,
  signal: AbortSignal,
  onEvent: (event: E) => void,
  onReopen?: () => void,
): void {
  let delay = WATCH_RETRY_MS;
  (async () => {
    for (let opened = false; !signal.aborted; opened = true) {
      if (opened) onReopen?.();
      try {
        for await (const event of open({ signal })) {
          if (signal.aborted) return;
          delay = WATCH_RETRY_MS;
          onEvent(event);
        }
      } catch (error) {
        if (!signal.aborted) console.warn('watch stopped; reopening', error);
      }
      if (signal.aborted) return;
      await new Promise<void>((resolve) => { setTimeout(() => resolve(), delay); });
      delay = Math.min(delay * 2, WATCH_RETRY_MAX_MS);
    }
  })();
}

const WATCH_RETRY_MS = 1_000;
const WATCH_RETRY_MAX_MS = 15_000;
