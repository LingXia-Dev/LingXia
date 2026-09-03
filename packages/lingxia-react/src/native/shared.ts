import type { CSSProperties, ForwardedRef, ReactNode } from "react";
import {
  assignForwardedRef,
  bindElementEvents,
  unbindElementEvents,
} from "../text_component_shared.js";
import { unwrapNativeEventPayload } from "@lingxia/elements";

export type NativeStyle = CSSProperties;

export interface LxNativeNodeProps {
  id?: string;
  automationId?: string;
  className?: string;
  style?: NativeStyle;
  pointerEvents?: "auto" | "none" | "box-only" | "box-none";
  hidden?: boolean;
  hiddenTransition?: "none" | "fade";
  "aria-label"?: string;
  "aria-description"?: string;
  "aria-hidden"?: boolean;
  children?: ReactNode;
}

export function applyNativeAria(
  el: HTMLElement,
  aria: Pick<LxNativeNodeProps, "aria-label" | "aria-description" | "aria-hidden">
): void {
  setOptionalAttribute(el, "aria-label", aria["aria-label"]);
  setOptionalAttribute(el, "aria-description", aria["aria-description"]);
  setOptionalAttribute(el, "aria-hidden", aria["aria-hidden"]);
}

export function assignNativeRef<T>(ref: ForwardedRef<T>, value: T | null): void {
  assignForwardedRef(ref, value);
}

export function bindNativeEvents(
  bound: HTMLElement | null,
  next: HTMLElement | null,
  listeners: Record<string, EventListenerObject>
): HTMLElement | null {
  return bindElementEvents(bound, next, listeners);
}

export function unbindNativeEvents(
  bound: HTMLElement | null,
  listeners: Record<string, EventListenerObject>
): void {
  unbindElementEvents(bound, listeners);
}

export function payloadListener<T>(
  readHandler: () => ((payload: T) => void) | undefined
): EventListenerObject {
  return {
    handleEvent: (event: Event) => {
      const handler = readHandler();
      if (typeof handler === "function") {
        handler(unwrapNativeEventPayload<T>(event));
      }
    },
  };
}

export function setBooleanAttribute(el: HTMLElement, name: string, value: boolean | undefined): void {
  if (value === undefined) {
    el.removeAttribute(name);
    return;
  }
  el.setAttribute(name, value ? "true" : "false");
}

export function setOptionalAttribute(
  el: HTMLElement,
  name: string,
  value: string | number | boolean | undefined | null
): void {
  if (value === undefined || value === null) {
    el.removeAttribute(name);
    return;
  }
  if (typeof value === "boolean") {
    el.setAttribute(name, value ? "true" : "false");
    return;
  }
  el.setAttribute(name, String(value));
}
