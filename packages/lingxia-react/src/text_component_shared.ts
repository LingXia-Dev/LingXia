import type { ForwardedRef } from "react";

export function getCustomEventDetail<T>(event: Event): T {
  return ((event as CustomEvent).detail ?? {}) as T;
}

export function assignForwardedRef<T>(ref: ForwardedRef<T>, value: T | null): void {
  if (typeof ref === "function") {
    ref(value);
    return;
  }
  if (ref) {
    ref.current = value;
  }
}

export function bindElementEvents(
  boundElement: HTMLElement | null,
  nextElement: HTMLElement | null,
  listeners: Record<string, EventListenerObject>
): HTMLElement | null {
  if (boundElement && boundElement !== nextElement) {
    unbindElementEvents(boundElement, listeners);
    boundElement = null;
  }
  if (nextElement && boundElement !== nextElement) {
    for (const [event, listener] of Object.entries(listeners)) {
      nextElement.addEventListener(event, listener);
    }
    boundElement = nextElement;
  }
  return boundElement;
}

export function unbindElementEvents(
  boundElement: HTMLElement | null,
  listeners: Record<string, EventListenerObject>
): void {
  if (!boundElement) return;
  for (const [event, listener] of Object.entries(listeners)) {
    boundElement.removeEventListener(event, listener);
  }
}
