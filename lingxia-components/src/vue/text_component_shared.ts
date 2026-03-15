export function getCustomEventDetail<T>(event: Event): T {
  return ((event as CustomEvent).detail ?? {}) as T;
}

export function bindElementEvents(
  currentBoundElement: HTMLElement | null,
  nextElement: HTMLElement | null,
  listeners: Record<string, EventListenerObject>
): HTMLElement | null {
  if (currentBoundElement && currentBoundElement !== nextElement) {
    for (const [event, listener] of Object.entries(listeners)) {
      currentBoundElement.removeEventListener(event, listener);
    }
    currentBoundElement = null;
  }
  if (nextElement && currentBoundElement !== nextElement) {
    for (const [event, listener] of Object.entries(listeners)) {
      nextElement.addEventListener(event, listener);
    }
    currentBoundElement = nextElement;
  }
  return currentBoundElement;
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
