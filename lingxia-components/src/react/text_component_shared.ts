export function normalizeBindingAttrName(key: string): string {
  return key.replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
}

export function getCustomEventDetail<T>(event: Event): T {
  return ((event as CustomEvent).detail ?? {}) as T;
}

export function bindEventListeners(
  el: HTMLElement,
  listeners: Record<string, EventListenerObject>
): void {
  for (const [event, listener] of Object.entries(listeners)) {
    el.addEventListener(event, listener);
  }
}

export function unbindEventListeners(
  el: HTMLElement,
  listeners: Record<string, EventListenerObject> | null
): void {
  if (!listeners) return;
  for (const [event, listener] of Object.entries(listeners)) {
    el.removeEventListener(event, listener);
  }
}

export function appendBindingAndDatasetAttrs(
  rest: Record<string, unknown>,
  target: Record<string, string>
): void {
  for (const [key, raw] of Object.entries(rest)) {
    if (raw === undefined || raw === null) continue;
    if (key.startsWith("data-")) {
      target[key] = String(raw);
      continue;
    }
    if ((key.startsWith("bind") || key.startsWith("catch")) && typeof raw === "string") {
      target[normalizeBindingAttrName(key)] = raw;
    }
  }
}
