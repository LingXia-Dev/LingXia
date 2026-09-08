import React, { forwardRef, useCallback, useEffect, useRef } from "react";
import {
  registerNativeButtonComponent,
  type NativeActionIcon,
  type NativeHandler,
  type PressPayload,
} from "@lingxia/elements";
import {
  assignNativeRef,
  applyNativeAria,
  bindNativeEvents,
  payloadListener,
  setOptionalAttribute,
  type LxNativeNodeProps,
} from "./shared.js";

if (typeof window !== "undefined") {
  registerNativeButtonComponent();
}

export interface LxNativeButtonProps extends LxNativeNodeProps {
  label?: string;
  icon?: NativeActionIcon;
  iconPosition?: "start" | "end";
  intent?: "neutral" | "accent" | "destructive";
  emphasis?: "primary" | "secondary" | "quiet";
  size?: "compact" | "regular";
  hitSlop?: number;
  disabled?: boolean;
  pressed?: boolean;
  expanded?: boolean;
  loading?: boolean;
  tabIndex?: 0 | -1;
  onPress?: NativeHandler<PressPayload>;
}

export const LxNativeButton = forwardRef<HTMLElement, LxNativeButtonProps>(
  (
    {
      id,
      automationId,
      className,
      style,
      pointerEvents,
      hidden,
      label,
      icon,
      iconPosition,
      intent,
      emphasis,
      size,
      hitSlop,
      disabled,
      pressed,
      expanded,
      loading,
      tabIndex,
      onPress,
      children,
      ...aria
    },
    ref
  ) => {
    const elementRef = useRef<HTMLElement | null>(null);
    const boundRef = useRef<HTMLElement | null>(null);
    const handlers = useRef({ onPress });
    handlers.current = { onPress };
    const listeners = useRef({
      press: payloadListener<PressPayload>(() => handlers.current.onPress),
    });

    const setRef = useCallback(
      (element: HTMLElement | null) => {
        boundRef.current = bindNativeEvents(boundRef.current, element, listeners.current);
        elementRef.current = element;
        assignNativeRef(ref, element);
      },
      [ref]
    );

    useEffect(
      () => () => {
        bindNativeEvents(boundRef.current, null, listeners.current);
        boundRef.current = null;
      },
      []
    );

    useEffect(() => {
      const el = elementRef.current;
      if (!el) return;
      setOptionalAttribute(el, "automation-id", automationId);
      setOptionalAttribute(el, "pointer-events", pointerEvents);
      setOptionalAttribute(el, "label", label);
      setOptionalAttribute(el, "icon-position", iconPosition);
      setOptionalAttribute(el, "intent", intent);
      setOptionalAttribute(el, "emphasis", emphasis);
      setOptionalAttribute(el, "size", size);
      setOptionalAttribute(el, "hit-slop", hitSlop);
      setOptionalAttribute(el, "disabled", disabled);
      setOptionalAttribute(el, "pressed", pressed);
      setOptionalAttribute(el, "expanded", expanded);
      setOptionalAttribute(el, "loading", loading);
      applyNativeAria(el, aria);
      if (typeof tabIndex === "number") {
        el.tabIndex = tabIndex;
      }
      setOptionalAttribute(el, "icon", icon);
    }, [
      automationId,
      pointerEvents,
      label,
      icon,
      iconPosition,
      intent,
      emphasis,
      size,
      hitSlop,
      disabled,
      pressed,
      expanded,
      loading,
      tabIndex,
      aria,
    ]);

    return React.createElement(
      "lx-native-button",
      { ref: setRef, id, className, style, hidden },
      children
    );
  }
);

LxNativeButton.displayName = "LxNativeButton";
