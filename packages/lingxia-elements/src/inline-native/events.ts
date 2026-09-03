const HOST_EVENTS = new Set(["press"]);

export type IslandHostMessage = {
  event?: string;
  action?: string;
  detail?: unknown;
};

export type IslandEventTarget = {
  dispatchEvent(event: Event): boolean;
};

/** Dispatch a host press onto an author element. */
export function applyIslandHostEvent(
  target: IslandEventTarget,
  message: IslandHostMessage
): boolean {
  const name = String(message.event ?? message.action ?? "").toLowerCase();
  if (!HOST_EVENTS.has(name)) {
    return false;
  }
  const detail = message.detail ?? {};
  target.dispatchEvent(
    new CustomEvent(name, {
      detail,
      bubbles: false,
      cancelable: false,
    })
  );
  return true;
}
