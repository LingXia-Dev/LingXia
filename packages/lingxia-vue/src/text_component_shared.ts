import { ref, watch, type Ref } from "vue";

export function getCustomEventDetail<T>(event: Event): T {
  return ((event as CustomEvent).detail ?? {}) as T;
}

type NativeAriaProps = {
  "aria-label"?: string;
  ariaLabel?: string;
  "aria-description"?: string;
  "aria-hidden"?: boolean;
  automationId?: string;
};

export function useNativeHostElement(props: NativeAriaProps): Ref<HTMLElement | null> {
  const elementRef = ref<HTMLElement | null>(null);
  const apply = () => applyNativeAriaAttributes(elementRef.value, props);
  watch(elementRef, apply);
  watch(
    () => [
      props["aria-label"],
      props.ariaLabel,
      props["aria-description"],
      props["aria-hidden"],
      props.automationId,
    ],
    apply,
  );
  return elementRef;
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

export function setOptionalDomAttribute(
  el: HTMLElement,
  name: string,
  value: string | number | boolean | undefined | null,
): void {
  if (value === undefined || value === null || value === false) {
    el.removeAttribute(name);
    return;
  }
  if (typeof value === "boolean") {
    el.setAttribute(name, value ? "true" : "false");
    return;
  }
  el.setAttribute(name, String(value));
}

/** Vue's custom-element patch may assign `el['aria-label']` as an expando.
 *  UI Automation and the island compiler only see the reflected attribute. */
export function applyNativeAriaAttributes(
  el: HTMLElement | null,
  props: {
    "aria-label"?: string;
    ariaLabel?: string;
    "aria-description"?: string;
    "aria-hidden"?: boolean;
    automationId?: string;
  },
): void {
  if (!el) return;
  setOptionalDomAttribute(el, "aria-label", props["aria-label"] ?? props.ariaLabel);
  setOptionalDomAttribute(el, "aria-description", props["aria-description"]);
  if (props["aria-hidden"] === undefined) {
    el.removeAttribute("aria-hidden");
  } else {
    setOptionalDomAttribute(el, "aria-hidden", props["aria-hidden"]);
  }
  setOptionalDomAttribute(el, "automation-id", props.automationId);
}
