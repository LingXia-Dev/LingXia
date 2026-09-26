/**
 * A fixture wait or budget ran out: `t.waitFor`, `waitForCall`, a driver
 * call raced against an action budget, cleanup, or the spec itself.
 */
export class TimeoutError extends Error {
  override readonly name = "TimeoutError";
  readonly code = "E_TIMEOUT" as const;
}

/**
 * The budget of one retrying action or assertion. The requested timeout is
 * clamped to what is left of the spec (or cleanup) budget, so an action never
 * outlives the spec silently, and every driver call is raced against what is
 * left of it, so a call that never returns fails the action by name instead of
 * surfacing as an anonymous spec timeout.
 *
 * JS can only stop waiting: a native call that blocks the JS thread cannot be
 * preempted, and a call abandoned here may still complete in the app.
 */
export class ActionDeadline {
  readonly started = Date.now();
  readonly timeout: number;
  /** The timeout the caller asked for, when the spec budget cut it. */
  readonly clampedFrom: number | undefined;

  constructor(requested: number, room: number = Number.POSITIVE_INFINITY) {
    const available = Math.max(1, Math.floor(room));
    this.timeout = requested > available ? available : requested;
    this.clampedFrom = requested > available ? requested : undefined;
  }

  elapsed(): number {
    return Date.now() - this.started;
  }

  remaining(): number {
    return this.timeout - this.elapsed();
  }

  expired(): boolean {
    return this.remaining() <= 0;
  }

  /** One line explaining a clamp, or `undefined` when the timeout is as asked. */
  clampNote(): string | undefined {
    return this.clampedFrom === undefined
      ? undefined
      : `timeout ${this.clampedFrom}ms was clamped to ${this.timeout}ms, the time left in the spec's budget`;
  }

  /**
   * Race `op` against the rest of this budget. On expiry the rejection names
   * the call and `context()` (the action, its target and location); the
   * abandoned promise is left to settle on its own.
   */
  async call<T>(label: string, op: () => T | Promise<T>, context: () => string): Promise<T> {
    const ms = Math.max(1, this.remaining());
    const task = Promise.resolve().then(op);
    let handle: ReturnType<typeof setTimeout> | undefined;
    const expiry = new Promise<never>((_, reject) => {
      handle = setTimeout(() => {
        task.catch(() => {});
        reject(new TimeoutError([
          `${label} did not return within ${ms}ms, the rest of the ${this.timeout}ms action budget.`,
          this.clampNote(),
          context(),
        ].filter(Boolean).join("\n")));
      }, ms);
    });
    try {
      return await Promise.race([task, expiry]);
    } finally {
      if (handle !== undefined) clearTimeout(handle);
    }
  }
}

/** The stable `code` of a driver rejection, when it carries one. */
export function errorCode(error: unknown): string | undefined {
  const code = error && typeof error === "object" ? (error as { code?: unknown }).code : undefined;
  return typeof code === "string" ? code : undefined;
}

/** Page-readiness codes: the target was not reached, so a retry is safe. */
const PAGE_NOT_READY_CODES = new Set(["E_PAGE_NOT_ACTIVE", "E_PAGE_NOT_READY"]);

/**
 * Errors the page drivers raise while a page is being replaced: the target is
 * not the active instance yet, or its WebView is not attached. They occur
 * before anything reaches the page, so retrying them is safe. Matched by code
 * (`E_PAGE_NOT_ACTIVE`, `E_PAGE_NOT_READY`), plus WebView2's
 * `ERROR_INVALID_STATE` (`0x8007139F`) while a navigation replaces the
 * document, which carries no code of its own.
 */
export function isTransientPageError(error: unknown): boolean {
  const code = errorCode(error);
  if (code !== undefined && PAGE_NOT_READY_CODES.has(code)) return true;
  return isWebView2InvalidState(error);
}

function isWebView2InvalidState(error: unknown): boolean {
  const message = error instanceof Error ? error.message : typeof error === "string" ? error : "";
  return /0x8007139f/i.test(message);
}

/**
 * The subset of transient errors that are raised while resolving the target
 * page, before input dispatch. A WebView2 script error during dispatch is
 * ambiguous (the input may have landed), so it is not in this set.
 */
export function isPreDispatchPageError(error: unknown): boolean {
  const code = errorCode(error);
  return code !== undefined && PAGE_NOT_READY_CODES.has(code);
}

/**
 * The element was missing or not interactable at dispatch: the native side
 * refused before any input reached the page, so a retry is safe.
 */
export function isElementRefusal(error: unknown): boolean {
  const code = errorCode(error);
  return code === "E_ELEMENT_NOT_FOUND" || code === "E_ELEMENT_NOT_INTERACTABLE";
}

/**
 * The transport between the test runtime and the app dropped a call: the
 * socket would block or was reset, or the channel closed under it. Only an
 * idempotent read may be retried on it; for input it is ambiguous, since the
 * call may have landed before the reply was lost.
 */
export function isTransientTransportError(error: unknown): boolean {
  if (errorCode(error) === "E_TRANSPORT") return true;
  const message = error instanceof Error ? error.message : typeof error === "string" ? error : "";
  if (!message) return false;
  return /os error (?:11|35|54|104)\b|resource temporarily unavailable|connection reset|broken pipe|channel closed|websocket (?:closed|disconnected|frame)/i
    .test(message);
}
