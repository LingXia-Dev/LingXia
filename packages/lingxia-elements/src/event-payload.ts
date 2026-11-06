/**
 * Payload-first unwrap used by React wrappers. HTML keeps CustomEvent.
 *
 * A DOM Event (or an event-like `{ type, detail }`) yields `detail`; a bare
 * payload object is returned as-is.
 */
export function unwrapNativeEventPayload<TPayload>(event: unknown): TPayload {
  if (isDomEvent(event) || isEventLike(event)) {
    const detail = (event as { detail?: TPayload }).detail;
    return (detail === undefined ? ({} as TPayload) : detail) as TPayload;
  }
  return event as TPayload;
}

function isDomEvent(value: unknown): value is Event {
  return typeof Event !== "undefined" && value instanceof Event;
}

function isEventLike(value: unknown): value is { detail?: unknown; type?: unknown } {
  if (!value || typeof value !== "object") return false;
  const record = value as { detail?: unknown; type?: unknown; target?: unknown };
  if (!("detail" in record)) return false;
  return "type" in record || "target" in record || "currentTarget" in record;
}
